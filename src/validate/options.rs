//! Options controlling a validation run.

/// Which phase of the schema to run.
///
/// # Examples
///
/// ```
/// use schematron::validate::PhaseSelection;
///
/// assert_eq!(PhaseSelection::from("#ALL"), PhaseSelection::All);
/// assert_eq!(PhaseSelection::from("#DEFAULT"), PhaseSelection::Default);
/// assert_eq!(PhaseSelection::from("strict"), PhaseSelection::Named("strict".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PhaseSelection {
    /// Use the schema's `@defaultPhase`, or every pattern when it has none.
    #[default]
    Default,
    /// Every pattern, including those no phase mentions.
    All,
    /// A phase by identifier. `#ALL` and `#DEFAULT` are recognised here too.
    Named(String),
}

impl From<&str> for PhaseSelection {
    fn from(value: &str) -> Self {
        match value {
            "#ALL" => PhaseSelection::All,
            "#DEFAULT" => PhaseSelection::Default,
            other => PhaseSelection::Named(other.to_string()),
        }
    }
}

impl From<String> for PhaseSelection {
    fn from(value: String) -> Self {
        PhaseSelection::from(value.as_str())
    }
}

/// How to run a validation.
///
/// # Examples
///
/// ```
/// use schematron::validate::{PhaseSelection, ValidateOptions};
///
/// let options = ValidateOptions::new()
///     .with_phase(PhaseSelection::Named("strict".into()))
///     .with_max_failures(10)
///     .with_record_fired_rules(false);
///
/// assert_eq!(options.max_failures, Some(10));
/// ```
// Fields will be added: a configurable implicit timezone is on the roadmap.
// Marking it non-exhaustive now means that will not be a breaking change.
// Every field has a `with_*` builder, so construction needs no literal.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct ValidateOptions {
    /// Which phase to run.
    pub phase: PhaseSelection,
    /// Stop after this many failed assertions.
    ///
    /// Useful for a fast "is this document broken at all" check over a large
    /// input, where the first failure is enough.
    pub max_failures: Option<usize>,
    /// Record every rule that fired, not only the ones that found something.
    ///
    /// On by default, because SVRL's `fired-rule` events are part of the
    /// standard output and because "which rule claimed this node" is the
    /// answer to most Schematron puzzles. Turn it off for large documents
    /// where the bookkeeping outweighs its value.
    pub record_fired_rules: bool,
    /// Evaluate the active patterns on separate threads.
    ///
    /// Patterns are independent by definition, so this changes nothing about
    /// the result: the same findings come back in the same order. Off by
    /// default because a library that spawns threads unasked is a surprise,
    /// and because many callers already parallelise across *documents* —
    /// [`Schema`](crate::Schema) is `Send + Sync` — where nesting a second
    /// layer would only oversubscribe the machine.
    ///
    /// The ceiling is the number of active patterns, so a single-pattern
    /// schema gains nothing. Setting [`ValidateOptions::max_failures`] keeps
    /// evaluation sequential; see `spec/validation.md`.
    pub parallel_patterns: bool,
    /// The instant `current-date()` and its companions report, in seconds
    /// since the Unix epoch.
    ///
    /// `None` reads the system clock **once**, at the start of the run.
    /// Supplying a value makes the run reproducible, which is how a test for
    /// a date rule should be written — a validator whose result depends on
    /// the wall clock cannot be tested, and its failures cannot be
    /// reproduced. See `spec/xpath2.md`.
    pub current_time: Option<f64>,
    /// The timezone a date or time with no offset is read as being in, in
    /// minutes from UTC.
    ///
    /// `None` means UTC, which keeps a validation run reproducible on any
    /// machine — XPath 2.0 would take the machine's local offset. Set it when
    /// local semantics are what the documents mean. See `spec/xpath2.md`.
    pub implicit_timezone: Option<i32>,
}

impl ValidateOptions {
    /// Default options: default phase, no failure limit, record fired rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: PhaseSelection::Default,
            max_failures: None,
            record_fired_rules: true,
            parallel_patterns: false,
            current_time: None,
            implicit_timezone: None,
        }
    }

    /// Sets the phase.
    #[must_use]
    pub fn with_phase(mut self, phase: PhaseSelection) -> Self {
        self.phase = phase;
        self
    }

    /// Stops after this many failures.
    #[must_use]
    pub fn with_max_failures(mut self, max: usize) -> Self {
        self.max_failures = Some(max);
        self
    }

    /// Sets whether to record rules that fired without finding anything.
    #[must_use]
    pub fn with_record_fired_rules(mut self, record: bool) -> Self {
        self.record_fired_rules = record;
        self
    }

    /// Evaluates the active patterns on separate threads.
    ///
    /// The report is unchanged; only the wall-clock time differs. Has no
    /// effect when the schema has one active pattern, or when
    /// [`ValidateOptions::max_failures`] is set.
    #[must_use]
    pub fn with_parallel_patterns(mut self, parallel: bool) -> Self {
        self.parallel_patterns = parallel;
        self
    }

    /// Fixes the instant the clock functions report, in seconds since the
    /// Unix epoch, making a run with date rules reproducible.
    #[must_use]
    pub fn with_current_time(mut self, seconds: f64) -> Self {
        self.current_time = Some(seconds);
        self
    }

    /// Sets the timezone a date or time with no offset is read as being in,
    /// in minutes from UTC.
    #[must_use]
    pub fn with_implicit_timezone(mut self, minutes: i32) -> Self {
        self.implicit_timezone = Some(minutes);
        self
    }

    /// The instant for this run, reading the system clock if none was given.
    ///
    /// Called once per run, never per expression.
    pub(crate) fn resolve_current_time(&self) -> f64 {
        self.current_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |elapsed| elapsed.as_secs_f64())
        })
    }

    /// Whether this run should actually use threads.
    ///
    /// `max_failures` forces sequential evaluation: "the first N failures" is
    /// not well defined while patterns are still running, and the crate
    /// values a reproducible report above the time threading would save.
    pub(crate) fn is_parallel(&self) -> bool {
        self.parallel_patterns && self.max_failures.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_keywords_are_recognised() {
        assert_eq!(PhaseSelection::from("#ALL"), PhaseSelection::All);
        assert_eq!(PhaseSelection::from("#DEFAULT"), PhaseSelection::Default);
        assert_eq!(
            PhaseSelection::from("x"),
            PhaseSelection::Named("x".to_string())
        );
    }

    #[test]
    fn defaults_record_fired_rules() {
        assert!(ValidateOptions::new().record_fired_rules);
        assert_eq!(ValidateOptions::new().max_failures, None);
    }

    #[test]
    fn parallelism_is_off_by_default() {
        assert!(!ValidateOptions::new().parallel_patterns);
        assert!(!ValidateOptions::new().is_parallel());
    }

    #[test]
    fn max_failures_forces_sequential_evaluation() {
        // Otherwise "the first N failures" would depend on thread timing.
        let options = ValidateOptions::new().with_parallel_patterns(true);
        assert!(options.is_parallel());
        assert!(!options.with_max_failures(5).is_parallel());
    }

    #[test]
    fn builders_chain() {
        let options = ValidateOptions::new()
            .with_phase(PhaseSelection::All)
            .with_max_failures(3)
            .with_record_fired_rules(false)
            .with_parallel_patterns(true);
        assert_eq!(options.phase, PhaseSelection::All);
        assert_eq!(options.max_failures, Some(3));
        assert!(!options.record_fired_rules);
        assert!(options.parallel_patterns);
    }
}
