//! XML Schema date and time values.
//!
//! `xs:date`, `xs:dateTime` and `xs:time`, in the lexical forms XML Schema
//! defines, with the arithmetic needed to order them.
//!
//! Written rather than taken from a calendar crate because the requirement is
//! narrow — parse a lexical form, compare two values, read a component — and
//! a general date library would bring a great deal more than that. The
//! civil-date conversion below is the standard days-from-civil algorithm.
//!
//! A value with no timezone is treated as UTC. XPath 2.0 uses an implicit
//! timezone from the evaluation context; fixing it at UTC makes a validation
//! run reproducible on any machine, which this crate values more. See
//! `spec/xpath2/`.

use std::fmt;

/// Which of the two duration subtypes a [`Duration`] holds.
///
/// XPath 2.0 splits `xs:duration` in two because the general type is not
/// totally ordered: whether one month exceeds thirty days depends on the
/// month. Implementing the subtypes, and not the general type, means every
/// duration this crate produces can be compared with every other of its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DurationKind {
    /// `xs:yearMonthDuration`, counted in months.
    YearMonth,
    /// `xs:dayTimeDuration`, counted in seconds.
    DayTime,
}

impl DurationKind {
    /// The type name, as a schema author writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            DurationKind::YearMonth => "xs:yearMonthDuration",
            DurationKind::DayTime => "xs:dayTimeDuration",
        }
    }
}

/// A duration: a number of months, or a number of seconds.
///
/// # Examples
///
/// ```
/// use schematron::xpath::{Duration, DurationKind};
///
/// let ninety_days = Duration::parse("P90D", DurationKind::DayTime).unwrap();
/// assert_eq!(ninety_days.to_seconds(), 90.0 * 86_400.0);
///
/// let quarter = Duration::parse("P3M", DurationKind::YearMonth).unwrap();
/// assert_eq!(quarter.to_months(), 3);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Duration {
    kind: DurationKind,
    /// Months, for a yearMonthDuration.
    months: i64,
    /// Seconds, for a dayTimeDuration.
    seconds: f64,
}

impl Duration {
    /// The subtype this value has.
    #[must_use]
    pub const fn kind(&self) -> DurationKind {
        self.kind
    }

    /// The number of months, for a yearMonthDuration.
    #[must_use]
    pub const fn to_months(&self) -> i64 {
        self.months
    }

    /// The number of seconds, for a dayTimeDuration.
    #[must_use]
    pub const fn to_seconds(&self) -> f64 {
        self.seconds
    }

    /// Builds a duration of a number of months.
    #[must_use]
    pub const fn from_months(months: i64) -> Self {
        Self {
            kind: DurationKind::YearMonth,
            months,
            seconds: 0.0,
        }
    }

    /// Builds a duration of a number of seconds.
    #[must_use]
    pub const fn from_seconds(seconds: f64) -> Self {
        Self {
            kind: DurationKind::DayTime,
            months: 0,
            seconds,
        }
    }

    /// Whether the duration runs backwards.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        match self.kind {
            DurationKind::YearMonth => self.months < 0,
            DurationKind::DayTime => self.seconds < 0.0,
        }
    }

    /// Parses a lexical form, `PnYnM` or `PnDTnHnMnS`, with an optional
    /// leading `-`.
    ///
    /// # Errors
    ///
    /// Returns a message naming the value and the type it failed to parse as.
    pub fn parse(text: &str, kind: DurationKind) -> Result<Duration, String> {
        let trimmed = text.trim();
        let reject = || format!("{trimmed:?} is not a valid {} value", kind.as_str());

        let (negative, rest) = match trimmed.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, trimmed),
        };
        let rest = rest.strip_prefix('P').ok_or_else(reject)?;
        if rest.is_empty() {
            return Err(reject());
        }

        let (date_part, time_part) = match rest.split_once('T') {
            Some((date, time)) => {
                // A `T` must be followed by at least one component.
                if time.is_empty() {
                    return Err(reject());
                }
                (date, Some(time))
            }
            None => (rest, None),
        };

        let date_fields = parse_duration_fields(date_part, "YMD").ok_or_else(reject)?;
        let time_fields = match time_part {
            Some(time) => parse_duration_fields(time, "HMS").ok_or_else(reject)?,
            None => Vec::new(),
        };
        if date_fields.is_empty() && time_fields.is_empty() {
            return Err(reject());
        }

        // `M` means months before the `T` and minutes after it, so the two
        // parts are summed separately rather than by unit letter.
        let mut months = 0_i64;
        let mut seconds = 0.0_f64;
        #[allow(clippy::cast_possible_truncation)] // Year and month counts are small.
        for (value, unit) in &date_fields {
            match unit {
                'Y' => months += *value as i64 * 12,
                'M' => months += *value as i64,
                'D' => seconds += value * 86_400.0,
                _ => return Err(reject()),
            }
        }
        for (value, unit) in &time_fields {
            match unit {
                'H' => seconds += value * 3_600.0,
                'M' => seconds += value * 60.0,
                'S' => seconds += value,
                _ => return Err(reject()),
            }
        }

        // Each subtype accepts only its own fields.
        match kind {
            DurationKind::YearMonth if seconds != 0.0 => return Err(reject()),
            DurationKind::DayTime if months != 0 => return Err(reject()),
            _ => {}
        }

        let sign = if negative { -1.0 } else { 1.0 };
        Ok(match kind {
            DurationKind::YearMonth => {
                Duration::from_months(if negative { -months } else { months })
            }
            DurationKind::DayTime => Duration::from_seconds(seconds * sign),
        })
    }

    /// The lexical form, as XPath's `string()` would render it.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn to_lexical(&self) -> String {
        let mut out = String::new();
        if self.is_negative() {
            out.push('-');
        }
        out.push('P');

        match self.kind {
            DurationKind::YearMonth => {
                let months = self.months.abs();
                let (years, months) = (months / 12, months % 12);
                if years != 0 {
                    out.push_str(&format!("{years}Y"));
                }
                if months != 0 || years == 0 {
                    out.push_str(&format!("{months}M"));
                }
            }
            DurationKind::DayTime => {
                let total = self.seconds.abs();
                let days = (total / 86_400.0).floor();
                let rest = total - days * 86_400.0;
                let hours = (rest / 3_600.0).floor();
                let minutes = ((rest % 3_600.0) / 60.0).floor();
                let seconds = rest % 60.0;

                if days != 0.0 {
                    out.push_str(&format!("{}D", days as i64));
                }
                if hours != 0.0 || minutes != 0.0 || seconds != 0.0 {
                    out.push('T');
                    if hours != 0.0 {
                        out.push_str(&format!("{}H", hours as i64));
                    }
                    if minutes != 0.0 {
                        out.push_str(&format!("{}M", minutes as i64));
                    }
                    if seconds != 0.0 {
                        if seconds.fract() == 0.0 {
                            out.push_str(&format!("{}S", seconds as i64));
                        } else {
                            out.push_str(&format!("{seconds}S"));
                        }
                    }
                } else if days == 0.0 {
                    out.push_str("T0S");
                }
            }
        }
        out
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_lexical())
    }
}

impl PartialEq for Duration {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.months == other.months && self.seconds == other.seconds
    }
}

impl PartialOrd for Duration {
    /// Orders two durations of the same subtype.
    ///
    /// `None` for two of different subtypes: whether a month exceeds thirty
    /// days has no answer, which is why the subtypes are separate.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.kind != other.kind {
            return None;
        }
        match self.kind {
            DurationKind::YearMonth => Some(self.months.cmp(&other.months)),
            DurationKind::DayTime => self.seconds.partial_cmp(&other.seconds),
        }
    }
}

/// Splits `1Y2M3D` into its `(value, unit)` fields, checking the unit order.
///
/// `units` gives the permitted letters in the order they must appear, which
/// is what rejects `P1M1Y`.
fn parse_duration_fields(text: &str, units: &str) -> Option<Vec<(f64, char)>> {
    if text.is_empty() {
        return Some(Vec::new());
    }
    let mut fields = Vec::new();
    let mut digits = String::new();
    let mut next_unit = 0;

    for c in text.chars() {
        if c.is_ascii_digit() || c == '.' {
            digits.push(c);
            continue;
        }
        let position = units.find(c)?;
        if position < next_unit || digits.is_empty() {
            return None;
        }
        next_unit = position + 1;
        fields.push((digits.parse().ok()?, c));
        digits.clear();
    }
    // Trailing digits with no unit.
    if !digits.is_empty() {
        return None;
    }
    Some(fields)
}

/// Adds a number of months to a date, clamping the day.
///
/// XML Schema requires the clamp: 31 January plus one month is 28 February,
/// or 29 February in a leap year, rather than overflowing into March.
#[must_use]
pub fn add_months(temporal: &Temporal, months: i64) -> Temporal {
    let total = temporal.year() * 12 + i64::from(temporal.month()) - 1 + months;
    let year = total.div_euclid(12);
    let month = u32::try_from(total.rem_euclid(12) + 1).unwrap_or(1);
    let day = temporal.day().min(days_in_month(year, month));

    Temporal {
        kind: temporal.kind(),
        year,
        month,
        day,
        hour: temporal.hour(),
        minute: temporal.minute(),
        second: temporal.second(),
        offset_minutes: temporal.offset_minutes(),
    }
}

/// Adds a number of seconds to a date or time.
#[must_use]
pub fn add_seconds(temporal: &Temporal, seconds: f64) -> Temporal {
    let offset = f64::from(temporal.offset_minutes().unwrap_or(0)) * 60.0;
    let shifted = from_unix_seconds(temporal.kind(), temporal.to_seconds() + seconds + offset);
    Temporal {
        offset_minutes: temporal.offset_minutes(),
        ..shifted
    }
}

/// Adjusts a temporal value to a new timezone, per XPath 2.0's
/// `adjust-date-to-timezone()`, `adjust-dateTime-to-timezone()`, and
/// `adjust-time-to-timezone()`.
///
/// One function serves all three because this crate's representation
/// already puts every kind in the shape the spec's own algorithm needs: a
/// `Date`'s hour/minute/second are already fixed at midnight, and a
/// `Time`'s year/month/day are already fixed at [`TIME_REFERENCE`] — the
/// exact "combine with 00:00:00" and "combine with 1972-12-31" recipes F&O
/// describes for those two forms, applied once here instead of three times.
///
/// `new_offset`: `None` removes the timezone. `Some(minutes)` sets it —
/// converting the instant `temporal` denotes, when it already had a
/// timezone (the local fields shift so the same instant is now expressed
/// at the new offset, which is how a date can roll to an adjacent day);
/// simply attaching the new offset with no conversion, when it had none,
/// since there is no instant yet to preserve.
#[must_use]
pub fn adjust_to_timezone(temporal: &Temporal, new_offset: Option<i32>) -> Temporal {
    match (temporal.offset_minutes, new_offset) {
        (None, target) => Temporal {
            offset_minutes: target,
            ..*temporal
        },
        (Some(_), None) => Temporal {
            offset_minutes: None,
            ..*temporal
        },
        (Some(_), Some(new)) => {
            let instant = temporal.to_seconds();
            Temporal {
                offset_minutes: Some(new),
                ..from_unix_seconds(temporal.kind, instant + f64::from(new) * 60.0)
            }
        }
    }
}

/// The date an `xs:time` is placed on, so that two times compare by their
/// time of day alone.
///
/// XML Schema uses 1972-12-31 for this, being a date that precedes a leap
/// second, so the choice is the standard's rather than arbitrary.
const TIME_REFERENCE: (i64, u32, u32) = (1972, 12, 31);

/// Which of the three types a [`Temporal`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemporalKind {
    /// `xs:date`
    Date,
    /// `xs:dateTime`
    DateTime,
    /// `xs:time`
    Time,
}

impl TemporalKind {
    /// The type name, as a schema author writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            TemporalKind::Date => "xs:date",
            TemporalKind::DateTime => "xs:dateTime",
            TemporalKind::Time => "xs:time",
        }
    }
}

/// A date, dateTime, or time.
///
/// Stored as its calendar components plus an optional timezone offset in
/// minutes, so that a component can be read back without recomputing it, and
/// separately as an instant for ordering.
///
/// # Examples
///
/// ```
/// use schematron::xpath::{Temporal, TemporalKind};
///
/// let date = Temporal::parse("2026-08-21", TemporalKind::Date).unwrap();
/// assert_eq!(date.year(), 2026);
/// assert_eq!(date.month(), 8);
/// assert_eq!(date.day(), 21);
///
/// let earlier = Temporal::parse("2020-01-01", TemporalKind::Date).unwrap();
/// assert!(earlier < date);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Temporal {
    kind: TemporalKind,
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    /// Seconds, including any fractional part.
    second: f64,
    /// Offset from UTC in minutes; `None` means none was written.
    offset_minutes: Option<i32>,
}

impl Temporal {
    /// The type this value has.
    #[must_use]
    pub const fn kind(&self) -> TemporalKind {
        self.kind
    }

    /// The year. Negative for years before the common era.
    #[must_use]
    pub const fn year(&self) -> i64 {
        self.year
    }

    /// The month, 1 to 12.
    #[must_use]
    pub const fn month(&self) -> u32 {
        self.month
    }

    /// The day of the month, 1 to 31.
    #[must_use]
    pub const fn day(&self) -> u32 {
        self.day
    }

    /// The hour, 0 to 23.
    #[must_use]
    pub const fn hour(&self) -> u32 {
        self.hour
    }

    /// The minute, 0 to 59.
    #[must_use]
    pub const fn minute(&self) -> u32 {
        self.minute
    }

    /// The seconds, including any fractional part.
    #[must_use]
    pub const fn second(&self) -> f64 {
        self.second
    }

    /// The timezone offset in minutes, if the value carried one.
    #[must_use]
    pub const fn offset_minutes(&self) -> Option<i32> {
        self.offset_minutes
    }

    /// Builds a value from UTC components.
    #[must_use]
    pub fn from_utc(
        kind: TemporalKind,
        year: i64,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: f64,
    ) -> Self {
        Self {
            kind,
            year,
            month,
            day,
            hour,
            minute,
            second,
            offset_minutes: Some(0),
        }
    }

    /// Parses a lexical form.
    ///
    /// # Errors
    ///
    /// Returns a message naming the value and the type it failed to parse as.
    /// A date typo must fail loudly; a silently false test would defeat the
    /// point of checking it.
    pub fn parse(text: &str, kind: TemporalKind) -> Result<Temporal, String> {
        let trimmed = text.trim();
        let reject = || {
            format!(
                "{trimmed:?} is not a valid {} value",
                kind.as_str()
            )
        };

        let (body, offset) = split_timezone(trimmed).ok_or_else(reject)?;

        let parsed = match kind {
            TemporalKind::Date => parse_date(body).map(|(y, m, d)| (y, m, d, 0, 0, 0.0)),
            TemporalKind::Time => parse_time(body)
                .map(|(h, mi, s)| (TIME_REFERENCE.0, TIME_REFERENCE.1, TIME_REFERENCE.2, h, mi, s)),
            TemporalKind::DateTime => {
                let (date, time) = body.split_once('T').ok_or_else(reject)?;
                let (year, month, day) = parse_date(date).ok_or_else(reject)?;
                let (hour, minute, second) = parse_time(time).ok_or_else(reject)?;
                Some((year, month, day, hour, minute, second))
            }
        };

        let (year, month, day, hour, minute, second) = parsed.ok_or_else(reject)?;
        if !is_valid_civil(year, month, day) {
            return Err(reject());
        }

        Ok(Temporal {
            kind,
            year,
            month,
            day,
            hour,
            minute,
            second,
            offset_minutes: offset,
        })
    }

    /// Parses a lexical form without being told which type it is.
    ///
    /// Used when casting an untyped value to match the other operand of a
    /// comparison, where the target type is known — and when a schema writes
    /// a bare literal, where it is not.
    #[must_use]
    pub fn parse_any(text: &str) -> Option<Temporal> {
        for kind in [
            TemporalKind::DateTime,
            TemporalKind::Date,
            TemporalKind::Time,
        ] {
            if let Ok(value) = Temporal::parse(text, kind) {
                return Some(value);
            }
        }
        None
    }

    /// The instant this value denotes, in seconds from 1970-01-01T00:00:00Z,
    /// reading a value with no timezone as UTC.
    ///
    /// Use [`Temporal::to_seconds_in`] where the implicit timezone matters.
    #[must_use]
    pub fn to_seconds(&self) -> f64 {
        self.to_seconds_in(0)
    }

    /// The instant this value denotes, reading a value with no timezone as
    /// being in `implicit_minutes`.
    ///
    /// XPath 2.0 takes the implicit timezone from the evaluation context. See
    /// `spec/xpath2/` for why this crate defaults it to UTC.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Day counts are far below 2^53.
    pub fn to_seconds_in(&self, implicit_minutes: i32) -> f64 {
        let days = days_from_civil(self.year, self.month, self.day);
        let offset = f64::from(self.offset_minutes.unwrap_or(implicit_minutes)) * 60.0;
        (days as f64) * 86_400.0
            + f64::from(self.hour) * 3_600.0
            + f64::from(self.minute) * 60.0
            + self.second
            - offset
    }

    /// Orders two values, reading a value with no timezone as being in
    /// `implicit_minutes`.
    #[must_use]
    pub fn compare_in(&self, other: &Temporal, implicit_minutes: i32) -> Option<std::cmp::Ordering> {
        self.to_seconds_in(implicit_minutes)
            .partial_cmp(&other.to_seconds_in(implicit_minutes))
    }

    /// The lexical form, as XPath's `string()` would render it.
    #[must_use]
    pub fn to_lexical(&self) -> String {
        let mut out = String::new();
        if matches!(self.kind, TemporalKind::Date | TemporalKind::DateTime) {
            out.push_str(&format!(
                "{:04}-{:02}-{:02}",
                self.year, self.month, self.day
            ));
        }
        if self.kind == TemporalKind::DateTime {
            out.push('T');
        }
        if matches!(self.kind, TemporalKind::Time | TemporalKind::DateTime) {
            out.push_str(&format!("{:02}:{:02}:", self.hour, self.minute));
            if self.second.fract() == 0.0 {
                // Whole seconds, and always below 60, so this cannot lose
                // anything.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let whole = self.second as u32;
                out.push_str(&format!("{whole:02}"));
            } else {
                out.push_str(&format!("{:09.6}", self.second));
            }
        }
        match self.offset_minutes {
            None => {}
            Some(0) => out.push('Z'),
            Some(minutes) => {
                let sign = if minutes < 0 { '-' } else { '+' };
                let minutes = minutes.abs();
                out.push_str(&format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60));
            }
        }
        out
    }
}

impl fmt::Display for Temporal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_lexical())
    }
}

impl PartialEq for Temporal {
    fn eq(&self, other: &Self) -> bool {
        self.to_seconds() == other.to_seconds()
    }
}

impl PartialOrd for Temporal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.to_seconds().partial_cmp(&other.to_seconds())
    }
}

/// Splits a trailing timezone designator from a lexical form.
///
/// Returns the body and the offset in minutes, or `None` if the designator is
/// malformed.
fn split_timezone(text: &str) -> Option<(&str, Option<i32>)> {
    if let Some(body) = text.strip_suffix('Z') {
        return Some((body, Some(0)));
    }
    // An offset is the last `+hh:mm` or `-hh:mm`. A leading `-` on a negative
    // year is not one, so only look at the tail.
    if text.len() >= 6 {
        let (body, tail) = text.split_at(text.len() - 6);
        let sign = match tail.as_bytes().first() {
            Some(b'+') => 1,
            Some(b'-') => -1,
            _ => return Some((text, None)),
        };
        if !body.is_empty() {
            let hours: i32 = tail.get(1..3)?.parse().ok()?;
            if tail.as_bytes().get(3) != Some(&b':') {
                return Some((text, None));
            }
            let minutes: i32 = tail.get(4..6)?.parse().ok()?;
            if hours > 14 || minutes > 59 {
                return None;
            }
            return Some((body, Some(sign * (hours * 60 + minutes))));
        }
    }
    Some((text, None))
}

/// Parses `YYYY-MM-DD`, allowing a negative year and more than four digits.
fn parse_date(text: &str) -> Option<(i64, u32, u32)> {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let parts: Vec<&str> = rest.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    if parts[0].len() < 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return None;
    }
    let year: i64 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    Some((if negative { -year } else { year }, month, day))
}

/// Parses `hh:mm:ss` with optional fractional seconds.
fn parse_time(text: &str) -> Option<(u32, u32, f64)> {
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() != 3 || parts[0].len() != 2 || parts[1].len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    // Seconds may carry a fraction, but must have two integral digits.
    let seconds_text = parts[2];
    let integral = seconds_text.split('.').next()?;
    if integral.len() != 2 || !seconds_text.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let second: f64 = seconds_text.parse().ok()?;

    // 24:00:00 is the one hour value above 23, and means midnight.
    if hour > 24 || (hour == 24 && (minute != 0 || second != 0.0)) {
        return None;
    }
    if minute > 59 || second >= 60.0 {
        return None;
    }
    Some((hour, minute, second))
}

/// Whether a year, month and day form a real calendar date.
fn is_valid_civil(year: i64, month: u32, day: u32) -> bool {
    if !(1..=12).contains(&month) || day < 1 {
        return false;
    }
    day <= days_in_month(year, month)
}

/// The number of days in a month, honouring the Gregorian leap rule.
fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// The proleptic Gregorian leap year rule.
fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Days from 1970-01-01 to the given civil date.
///
/// Howard Hinnant's `days_from_civil`, which is exact for the proleptic
/// Gregorian calendar over the whole range of `i64` years.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era =
        year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The civil date a day count denotes, inverting [`days_from_civil`].
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = if month <= 2 { year + 1 } else { year };
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// Builds a value from seconds since the Unix epoch, in UTC.
///
/// Each kind keeps only the components it has, and zeroes the rest. A date
/// with the time of day still attached would not equal the same date written
/// out — the very comparison the type exists for.
#[must_use]
pub fn from_unix_seconds(kind: TemporalKind, seconds: f64) -> Temporal {
    let days = (seconds / 86_400.0).floor();
    let rest = seconds - days * 86_400.0;
    #[allow(clippy::cast_possible_truncation)] // Bounded by the day count.
    let (year, month, day) = civil_from_days(days as i64);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let hour = (rest / 3_600.0).floor() as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let minute = ((rest % 3_600.0) / 60.0).floor() as u32;
    let second = rest % 60.0;

    match kind {
        // A date has no time of day.
        TemporalKind::Date => Temporal::from_utc(kind, year, month, day, 0, 0, 0.0),
        TemporalKind::DateTime => {
            Temporal::from_utc(kind, year, month, day, hour, minute, second)
        }
        // A time has no date. The reference day is the one `parse_time` uses,
        // so a parsed time and a generated one are on the same footing.
        TemporalKind::Time => {
            Temporal::from_utc(kind, TIME_REFERENCE.0, TIME_REFERENCE.1, TIME_REFERENCE.2, hour, minute, second)
        }
    }
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss)] // Day counts in tests are tiny.
mod tests {
    use super::*;

    #[test]
    fn parses_a_date() {
        let d = Temporal::parse("2026-08-21", TemporalKind::Date).unwrap();
        assert_eq!((d.year(), d.month(), d.day()), (2026, 8, 21));
        assert_eq!(d.offset_minutes(), None);
    }

    #[test]
    fn parses_a_date_time_with_fractional_seconds() {
        let d = Temporal::parse("2026-08-21T10:30:05.25Z", TemporalKind::DateTime).unwrap();
        assert_eq!((d.hour(), d.minute()), (10, 30));
        assert!((d.second() - 5.25).abs() < f64::EPSILON);
        assert_eq!(d.offset_minutes(), Some(0));
    }

    #[test]
    fn parses_a_time() {
        let t = Temporal::parse("23:59:59", TemporalKind::Time).unwrap();
        assert_eq!((t.hour(), t.minute()), (23, 59));
    }

    #[test]
    fn parses_timezone_offsets() {
        let plus = Temporal::parse("2026-08-21+01:30", TemporalKind::Date).unwrap();
        assert_eq!(plus.offset_minutes(), Some(90));
        let minus = Temporal::parse("2026-08-21-05:00", TemporalKind::Date).unwrap();
        assert_eq!(minus.offset_minutes(), Some(-300));
    }

    #[test]
    fn an_offset_shifts_the_instant() {
        // The same wall time at two offsets is two different instants.
        let utc = Temporal::parse("2026-08-21T00:00:00Z", TemporalKind::DateTime).unwrap();
        let ahead = Temporal::parse("2026-08-21T00:00:00+01:00", TemporalKind::DateTime).unwrap();
        assert!(ahead < utc);
        assert!((utc.to_seconds() - ahead.to_seconds() - 3600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn adjusting_a_date_can_roll_it_to_an_adjacent_day() {
        // The worked example from the F&O spec's own reference material:
        // 2002-03-07-07:00, adjusted to -PT10H, rolls back to 2002-03-06.
        let date = Temporal::parse("2002-03-07-07:00", TemporalKind::Date).unwrap();
        let adjusted = adjust_to_timezone(&date, Some(-600));
        assert_eq!((adjusted.year(), adjusted.month(), adjusted.day()), (2002, 3, 6));
        assert_eq!(adjusted.offset_minutes(), Some(-600));
    }

    #[test]
    fn adjusting_a_time_wraps_within_its_reference_day() {
        // 10:00:00-07:00, adjusted to -PT10H, becomes 07:00:00-10:00.
        let time = Temporal::parse("10:00:00-07:00", TemporalKind::Time).unwrap();
        let adjusted = adjust_to_timezone(&time, Some(-600));
        assert_eq!((adjusted.hour(), adjusted.minute()), (7, 0));
        assert_eq!(adjusted.offset_minutes(), Some(-600));
    }

    #[test]
    fn adjusting_preserves_the_instant() {
        let dt = Temporal::parse("2026-08-21T09:00:00+02:00", TemporalKind::DateTime).unwrap();
        let adjusted = adjust_to_timezone(&dt, Some(-300));
        assert!((dt.to_seconds() - adjusted.to_seconds()).abs() < f64::EPSILON);
        assert_eq!(adjusted.offset_minutes(), Some(-300));
    }

    #[test]
    fn adjusting_a_timezone_less_value_attaches_without_converting() {
        let dt = Temporal::parse("2026-08-21T09:00:00", TemporalKind::DateTime).unwrap();
        let adjusted = adjust_to_timezone(&dt, Some(-300));
        assert_eq!((adjusted.hour(), adjusted.minute()), (9, 0));
        assert_eq!(adjusted.offset_minutes(), Some(-300));
    }

    #[test]
    fn adjusting_to_no_timezone_strips_it_without_converting() {
        let dt = Temporal::parse("2026-08-21T09:00:00+02:00", TemporalKind::DateTime).unwrap();
        let adjusted = adjust_to_timezone(&dt, None);
        assert_eq!((adjusted.hour(), adjusted.minute()), (9, 0));
        assert_eq!(adjusted.offset_minutes(), None);
    }

    #[test]
    fn rejects_impossible_dates() {
        for bad in [
            "2026-02-30",
            "2026-13-01",
            "2026-00-01",
            "2026-01-00",
            "2025-02-29",
            "not-a-date",
            "2026-8-21",
            "",
        ] {
            assert!(
                Temporal::parse(bad, TemporalKind::Date).is_err(),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn accepts_leap_days_only_in_leap_years() {
        assert!(Temporal::parse("2024-02-29", TemporalKind::Date).is_ok());
        assert!(Temporal::parse("2000-02-29", TemporalKind::Date).is_ok());
        assert!(Temporal::parse("1900-02-29", TemporalKind::Date).is_err());
        assert!(Temporal::parse("2023-02-29", TemporalKind::Date).is_err());
    }

    #[test]
    fn rejects_impossible_times() {
        for bad in ["25:00:00", "10:60:00", "10:00:60", "24:00:01", "1:00:00"] {
            assert!(
                Temporal::parse(bad, TemporalKind::Time).is_err(),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn orders_dates() {
        let a = Temporal::parse("2020-01-01", TemporalKind::Date).unwrap();
        let b = Temporal::parse("2026-08-21", TemporalKind::Date).unwrap();
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, Temporal::parse("2020-01-01", TemporalKind::Date).unwrap());
    }

    #[test]
    fn civil_conversion_round_trips() {
        for (year, month, day) in [
            (1970, 1, 1),
            (2026, 8, 21),
            (1900, 2, 28),
            (2000, 2, 29),
            (1, 1, 1),
            (-44, 3, 15),
        ] {
            let days = days_from_civil(year, month, day);
            assert_eq!(civil_from_days(days), (year, month, day), "{year}-{month}-{day}");
        }
    }

    #[test]
    fn the_epoch_is_day_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn builds_from_unix_seconds() {
        let d = from_unix_seconds(TemporalKind::DateTime, 0.0);
        assert_eq!((d.year(), d.month(), d.day()), (1970, 1, 1));
        assert_eq!((d.hour(), d.minute()), (0, 0));

        // 2026-08-21T12:00:00Z
        let seconds = days_from_civil(2026, 8, 21) as f64 * 86_400.0 + 43_200.0;
        let d = from_unix_seconds(TemporalKind::DateTime, seconds);
        assert_eq!((d.year(), d.month(), d.day()), (2026, 8, 21));
        assert_eq!(d.hour(), 12);
    }

    #[test]
    fn a_generated_date_has_no_time_of_day() {
        // Otherwise `current-date()` would not equal the same date written
        // out, which is the comparison the type exists for.
        let noon = days_from_civil(2026, 8, 21) as f64 * 86_400.0 + 43_200.0;
        let generated = from_unix_seconds(TemporalKind::Date, noon);
        let written = Temporal::parse("2026-08-21Z", TemporalKind::Date).unwrap();
        assert_eq!(generated, written);
        assert_eq!(generated.hour(), 0);
    }

    #[test]
    fn a_generated_time_sits_on_the_reference_day() {
        let noon = days_from_civil(2026, 8, 21) as f64 * 86_400.0 + 43_200.0;
        let generated = from_unix_seconds(TemporalKind::Time, noon);
        let written = Temporal::parse("12:00:00Z", TemporalKind::Time).unwrap();
        assert_eq!(generated, written);
    }

    #[test]
    fn lexical_form_round_trips() {
        for (text, kind) in [
            ("2026-08-21Z", TemporalKind::Date),
            ("2026-08-21T10:30:05Z", TemporalKind::DateTime),
            ("10:30:05Z", TemporalKind::Time),
            ("2026-08-21+01:30", TemporalKind::Date),
        ] {
            let parsed = Temporal::parse(text, kind).unwrap();
            assert_eq!(parsed.to_lexical(), text);
        }
    }

    #[test]
    fn parse_any_picks_the_right_type() {
        assert_eq!(
            Temporal::parse_any("2026-08-21").unwrap().kind(),
            TemporalKind::Date
        );
        assert_eq!(
            Temporal::parse_any("2026-08-21T10:00:00").unwrap().kind(),
            TemporalKind::DateTime
        );
        assert_eq!(
            Temporal::parse_any("10:00:00").unwrap().kind(),
            TemporalKind::Time
        );
        assert!(Temporal::parse_any("nonsense").is_none());
    }
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss)]
mod duration_tests {
    use super::*;

    #[test]
    fn parses_day_time_durations() {
        let d = Duration::parse("P90D", DurationKind::DayTime).unwrap();
        assert_eq!(d.to_seconds(), 90.0 * 86_400.0);

        let d = Duration::parse("P1DT2H30M15S", DurationKind::DayTime).unwrap();
        assert_eq!(d.to_seconds(), 86_400.0 + 7_200.0 + 1_800.0 + 15.0);

        let d = Duration::parse("PT90M", DurationKind::DayTime).unwrap();
        assert_eq!(d.to_seconds(), 5_400.0);
    }

    #[test]
    fn parses_year_month_durations() {
        assert_eq!(
            Duration::parse("P1Y6M", DurationKind::YearMonth).unwrap().to_months(),
            18
        );
        assert_eq!(
            Duration::parse("P3M", DurationKind::YearMonth).unwrap().to_months(),
            3
        );
        assert_eq!(
            Duration::parse("P2Y", DurationKind::YearMonth).unwrap().to_months(),
            24
        );
    }

    #[test]
    fn m_means_months_before_the_t_and_minutes_after_it() {
        // The one genuinely ambiguous letter in the lexical form.
        assert_eq!(
            Duration::parse("P1M", DurationKind::YearMonth).unwrap().to_months(),
            1
        );
        assert_eq!(
            Duration::parse("PT1M", DurationKind::DayTime).unwrap().to_seconds(),
            60.0
        );
    }

    #[test]
    fn parses_negative_durations() {
        assert_eq!(
            Duration::parse("-P1D", DurationKind::DayTime).unwrap().to_seconds(),
            -86_400.0
        );
        assert!(Duration::parse("-P1D", DurationKind::DayTime).unwrap().is_negative());
        assert_eq!(
            Duration::parse("-P6M", DurationKind::YearMonth).unwrap().to_months(),
            -6
        );
    }

    #[test]
    fn each_subtype_rejects_the_others_fields() {
        // A dayTimeDuration has no months, and a yearMonthDuration no days.
        assert!(Duration::parse("P1M", DurationKind::DayTime).is_err());
        assert!(Duration::parse("P1D", DurationKind::YearMonth).is_err());
        assert!(Duration::parse("P1Y", DurationKind::DayTime).is_err());
    }

    #[test]
    fn rejects_malformed_durations() {
        for bad in ["", "P", "1D", "PD", "P1X", "PT", "P1DT", "P1M1Y", "PT1S1H", "P1"] {
            assert!(
                Duration::parse(bad, DurationKind::DayTime).is_err()
                    && Duration::parse(bad, DurationKind::YearMonth).is_err(),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn orders_durations_of_the_same_subtype() {
        let short = Duration::parse("P1D", DurationKind::DayTime).unwrap();
        let long = Duration::parse("P90D", DurationKind::DayTime).unwrap();
        assert!(short < long);
        assert_eq!(short, Duration::parse("PT24H", DurationKind::DayTime).unwrap());
    }

    #[test]
    fn durations_of_different_subtypes_are_incomparable() {
        // Whether a month exceeds thirty days has no answer.
        let months = Duration::from_months(1);
        let days = Duration::from_seconds(30.0 * 86_400.0);
        assert!(months.partial_cmp(&days).is_none());
        assert_ne!(months, days);
    }

    #[test]
    fn lexical_form_round_trips() {
        for (text, kind) in [
            ("P90D", DurationKind::DayTime),
            ("P1DT2H30M15S", DurationKind::DayTime),
            ("-P1D", DurationKind::DayTime),
            ("P1Y6M", DurationKind::YearMonth),
            ("P3M", DurationKind::YearMonth),
        ] {
            let parsed = Duration::parse(text, kind).unwrap();
            assert_eq!(parsed.to_lexical(), text, "{text}");
        }
    }

    #[test]
    fn adding_months_clamps_the_day() {
        // XML Schema requires the clamp rather than an overflow into March.
        let january = Temporal::parse("2026-01-31", TemporalKind::Date).unwrap();
        assert_eq!(add_months(&january, 1).to_lexical(), "2026-02-28");

        let leap = Temporal::parse("2024-01-31", TemporalKind::Date).unwrap();
        assert_eq!(add_months(&leap, 1).to_lexical(), "2024-02-29");
    }

    #[test]
    fn adding_months_crosses_years_in_both_directions() {
        let date = Temporal::parse("2026-08-21", TemporalKind::Date).unwrap();
        assert_eq!(add_months(&date, 12).to_lexical(), "2027-08-21");
        assert_eq!(add_months(&date, -12).to_lexical(), "2025-08-21");
        assert_eq!(add_months(&date, 5).to_lexical(), "2027-01-21");
        assert_eq!(add_months(&date, -8).to_lexical(), "2025-12-21");
    }

    #[test]
    fn adding_seconds_moves_a_date() {
        let date = Temporal::parse("2026-08-21", TemporalKind::Date).unwrap();
        assert_eq!(add_seconds(&date, 86_400.0).to_lexical(), "2026-08-22");
        assert_eq!(add_seconds(&date, -86_400.0).to_lexical(), "2026-08-20");
    }
}
