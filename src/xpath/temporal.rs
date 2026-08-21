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
//! `spec/xpath2.md`.

use std::fmt;

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

    /// The instant this value denotes, in seconds from 1970-01-01T00:00:00Z.
    ///
    /// A value with no timezone is read as UTC.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Day counts are far below 2^53.
    pub fn to_seconds(&self) -> f64 {
        let days = days_from_civil(self.year, self.month, self.day);
        let offset = f64::from(self.offset_minutes.unwrap_or(0)) * 60.0;
        (days as f64) * 86_400.0
            + f64::from(self.hour) * 3_600.0
            + f64::from(self.minute) * 60.0
            + self.second
            - offset
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
