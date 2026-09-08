//! Parsing one repeating record at a time, in bounded memory.
//!
//! [`StreamingReader`] parses the "skeleton" — the document element and
//! nothing else — once, then parses each of its direct children as a
//! complete subtree, one at a time, into the *same* arena: each call to
//! [`StreamingReader::next_record`] first truncates the arena back to the
//! skeleton's own length, so a `NodeId` from the previous record is never
//! observed again once the next one starts. Peak memory is the skeleton
//! plus one record, not the whole document — see `spec/streaming/` for what
//! a schema and document have to look like for this to be sound at all.
//!
//! This does not replace or duplicate [`super::parser`]'s event handling:
//! [`super::parser::handle_event`] is shared verbatim, so entity, CDATA,
//! comment, PI, and depth-limit behavior can never drift between whole-
//! document and streaming parsing.

use std::cell::Cell;
use std::io::{BufRead, BufReader, Read};
use std::rc::Rc;

use quick_xml::Reader;

use super::document::Document;
use super::node::NodeId;
use super::parser::{handle_event, Builder, EventOutcome, PositionSource};
use crate::error::Result;

/// Counts (line, column) as bytes are consumed, so a streaming parse error
/// can still report a real position without ever buffering the whole input.
///
/// Whole-document parsing scans the complete source for this
/// (`super::parser::line_column`); a streaming source is never fully
/// buffered, so this tracks the same information incrementally through the
/// standard `Read`/`BufRead` methods quick-xml already calls, rather than
/// re-deriving it from a byte offset after the fact.
struct LineTrackingReader<R> {
    inner: BufReader<R>,
    position: Rc<Cell<(usize, usize)>>,
}

impl<R: Read> LineTrackingReader<R> {
    fn new(inner: R) -> (Self, Rc<Cell<(usize, usize)>>) {
        let position = Rc::new(Cell::new((1, 1)));
        (
            Self {
                inner: BufReader::new(inner),
                position: Rc::clone(&position),
            },
            position,
        )
    }

    fn advance(position: &Cell<(usize, usize)>, bytes: &[u8]) {
        let (mut line, mut column) = position.get();
        for &b in bytes {
            if b == b'\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        position.set((line, column));
    }
}

impl<R: Read> Read for LineTrackingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        Self::advance(&self.position, &buf[..n]);
        Ok(n)
    }
}

impl<R: Read> BufRead for LineTrackingReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        // `fill_buf` here just returns the buffer already filled above, at
        // no extra read cost, so the bytes being consumed can be scanned
        // for newlines before they are gone.
        if let Ok(buf) = self.inner.fill_buf() {
            let n = amt.min(buf.len());
            Self::advance(&self.position, &buf[..n]);
        }
        self.inner.consume(amt);
    }
}

/// Parses one record's worth of a document at a time. See the module
/// documentation.
///
/// Requires UTF-8 (or ASCII-compatible) input: unlike [`Document::from_bytes`],
/// there is no UTF-16 byte-order-mark transcoding here, because that would
/// mean decoding the whole input up front — exactly the cost streaming
/// exists to avoid. A UTF-16 document needs whole-document validation.
pub(crate) struct StreamingReader<R: Read> {
    reader: Reader<LineTrackingReader<R>>,
    /// Reused across every `read_event_into` call: unlike `Reader::from_str`,
    /// a `BufRead`-backed reader hands back events borrowed from a buffer
    /// the caller owns, not from the original source.
    buf: Vec<u8>,
    builder: Builder<'static>,
    open: Vec<NodeId>,
    saw_element: bool,
    /// `builder.doc.nodes.len()` once the document element itself is open
    /// and nothing else has been parsed yet — the length every record is
    /// truncated back to before the next one is parsed.
    skeleton_len: usize,
    /// The document element: every record is one of its direct children.
    record_parent: NodeId,
    /// One running count per (kind, expanded name), matching how
    /// `Document::finalize_subtree` groups siblings — the true cumulative
    /// count `Document::set_sibling_position` needs, since a record's
    /// parent only ever has one child attached at a time.
    sibling_counters: std::collections::HashMap<(super::NodeKind, Option<String>, String), usize>,
}

impl<R: Read> StreamingReader<R> {
    /// Parses down to and including the document element — the skeleton —
    /// leaving the reader positioned to parse its children one at a time.
    ///
    /// # Errors
    ///
    /// As [`Document::from_str`], for anything malformed before or at the
    /// document element itself.
    pub(crate) fn open(source: R) -> Result<Self> {
        let (counting, position) = LineTrackingReader::new(source);
        let mut reader = Reader::from_reader(counting);
        let config = reader.config_mut();
        config.trim_text(false);
        config.expand_empty_elements = false;
        config.check_end_names = true;
        config.check_comments = true;

        let mut builder = Builder::new(PositionSource::Streaming(position));
        let mut open: Vec<NodeId> = vec![builder.doc.root];
        let mut saw_element = false;
        let mut buf = Vec::new();

        loop {
            buf.clear();
            let event = reader
                .read_event_into(&mut buf)
                .map_err(|e| builder.error(0, e.to_string()))?;
            match handle_event(&mut builder, &mut open, &mut saw_element, 0, event)? {
                EventOutcome::Eof => {
                    return Err(builder.error(0, "document has no element"));
                }
                EventOutcome::Continue => {}
            }
            if saw_element {
                break;
            }
        }

        let record_parent = *open.last().expect("the root is never popped");
        let skeleton_len = builder.doc.nodes.len();

        Ok(Self {
            reader,
            buf,
            builder,
            open,
            saw_element,
            skeleton_len,
            record_parent,
            sibling_counters: std::collections::HashMap::new(),
        })
    }

    /// The document as it stands right now: the skeleton, plus the current
    /// record if [`StreamingReader::next_record`] has returned one.
    pub(crate) fn document(&self) -> &Document {
        &self.builder.doc
    }

    /// Parses the next direct child of the document element as a complete
    /// subtree, or `None` once the document element itself has closed.
    ///
    /// Resets the arena back to the skeleton first, so a `NodeId` from the
    /// previous record is never valid to use again after this returns.
    pub(crate) fn next_record(&mut self) -> Result<Option<NodeId>> {
        self.builder.doc.nodes.truncate(self.skeleton_len);
        self.builder.doc.nodes[self.record_parent.0].children.clear();
        // The skeleton's own nodes never change, but a fresh `Vec` element
        // slot after `truncate` still needs its stale index forgotten from
        // any side table — there is none besides `children`, cleared above.

        let mut record: Option<NodeId> = None;
        loop {
            self.buf.clear();
            let event = self
                .reader
                .read_event_into(&mut self.buf)
                .map_err(|e| self.builder.error(0, e.to_string()))?;

            // The very next `Start`/`Empty` seen while resting at the
            // document element's own depth is the next record — captured
            // before `handle_event` runs, so it is unambiguous even if the
            // event turns out to self-close (`Empty` never changes `open`).
            let opens_next_record = record.is_none()
                && self.open.len() == 2
                && matches!(
                    event,
                    quick_xml::events::Event::Start(_) | quick_xml::events::Event::Empty(_)
                );

            match handle_event(
                &mut self.builder,
                &mut self.open,
                &mut self.saw_element,
                0,
                event,
            )? {
                EventOutcome::Eof => {
                    return Err(self.builder.error(0, "unclosed element at end of input"));
                }
                EventOutcome::Continue => {}
            }

            if opens_next_record {
                record = Some(
                    *self.builder.doc.nodes[self.record_parent.0]
                        .children
                        .last()
                        .expect("handle_event just pushed this record onto its parent"),
                );
            }

            if self.open.len() == 1 {
                // The document element itself just closed: no more records.
                return Ok(None);
            }
            if self.open.len() == 2 && record.is_some() {
                // Back down to [root, document element]: this record's
                // whole subtree — `Start`...`End`, or a self-closing
                // `Empty` that never left this depth to begin with — is
                // fully parsed.
                break;
            }
        }

        let record = record.expect("open.len() == 2 with a record is only reached after one opened");
        self.builder.doc.finalize_subtree(record);

        let key = self.sibling_key(record);
        let counter = self.sibling_counters.entry(key).or_insert(0);
        *counter += 1;
        self.builder.doc.set_sibling_position(record, *counter);

        Ok(Some(record))
    }

    /// Reads whatever remains after the document element closes — trailing
    /// whitespace, comments, and processing instructions are legal; other
    /// character data is not — mirroring the tail of whole-document
    /// parsing exactly, via the same shared [`handle_event`].
    ///
    /// Call once [`StreamingReader::next_record`] has returned `None`.
    pub(crate) fn finish(&mut self) -> Result<()> {
        loop {
            self.buf.clear();
            let event = self
                .reader
                .read_event_into(&mut self.buf)
                .map_err(|e| self.builder.error(0, e.to_string()))?;
            if let EventOutcome::Eof = handle_event(
                &mut self.builder,
                &mut self.open,
                &mut self.saw_element,
                0,
                event,
            )? {
                break;
            }
        }
        if self.open.len() != 1 {
            return Err(self.builder.error(0, "unclosed element at end of input"));
        }
        Ok(())
    }

    fn sibling_key(&self, id: NodeId) -> (super::NodeKind, Option<String>, String) {
        let document = &self.builder.doc;
        let (uri, local) = document
            .name(id)
            .map_or((None, String::new()), |n| (n.uri.clone(), n.local.clone()));
        (document.kind(id), uri, local)
    }
}
