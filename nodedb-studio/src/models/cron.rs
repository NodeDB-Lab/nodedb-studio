//! Five-field cron expressions and their next run times.
//!
//! `minute hour day-of-month month day-of-week`, with `*`, lists, ranges,
//! steps, month and weekday names, and the `@hourly`-style macros. When both
//! day fields are restricted, a day matches when either does, as in Vixie
//! cron. Seconds, years, and the `L`, `W`, `#`, and `?` extensions are not
//! accepted: the server schedules at whole-minute resolution.
//!
//! Run times are found in the job's zone. A local time that does not exist
//! (clocks go forward) is skipped, and one that happens twice (clocks go
//! back) runs once, at its first occurrence.

use crate::models::format::civil;
use crate::models::zone::{Zone, weekday};

/// Which field of the expression an error is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Minute,
    Hour,
    DayOfMonth,
    Month,
    DayOfWeek,
}

impl Field {
    const ALL: [Field; 5] = [
        Field::Minute,
        Field::Hour,
        Field::DayOfMonth,
        Field::Month,
        Field::DayOfWeek,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Field::Minute => "minute",
            Field::Hour => "hour",
            Field::DayOfMonth => "day of month",
            Field::Month => "month",
            Field::DayOfWeek => "day of week",
        }
    }

    fn range(self) -> (u32, u32) {
        match self {
            Field::Minute => (0, 59),
            Field::Hour => (0, 23),
            Field::DayOfMonth => (1, 31),
            Field::Month => (1, 12),
            // 7 is a second spelling of Sunday.
            Field::DayOfWeek => (0, 7),
        }
    }
}

/// Why one part of a field was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Reason {
    #[error("has an empty entry")]
    Empty,
    #[error("`{0}` is not a number or a name")]
    NotAValue(String),
    #[error("`{value}` is outside {min}-{max}")]
    OutOfRange { value: u32, min: u32, max: u32 },
    #[error("the range `{0}` ends before it starts")]
    BackwardsRange(String),
    #[error("the step must be at least 1")]
    ZeroStep,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CronError {
    #[error("the schedule is empty")]
    Empty,
    #[error("a schedule has 5 fields (minute hour day month weekday), this has {0}")]
    FieldCount(usize),
    #[error("unknown macro `{0}`: use @hourly, @daily, @weekly, @monthly, or @yearly")]
    UnknownMacro(String),
    #[error("{}: {reason}", field.label())]
    Field { field: Field, reason: Reason },
}

/// A parsed schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    minutes: u64,
    hours: u32,
    days: u32,
    months: u16,
    weekdays: u8,
    /// The day-of-month field starts with `*`.
    any_day: bool,
    /// The day-of-week field starts with `*`.
    any_weekday: bool,
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const WEEKDAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

fn macro_expansion(name: &str) -> Option<&'static str> {
    match name {
        "@yearly" | "@annually" => Some("0 0 1 1 *"),
        "@monthly" => Some("0 0 1 * *"),
        "@weekly" => Some("0 0 * * 0"),
        "@daily" | "@midnight" => Some("0 0 * * *"),
        "@hourly" => Some("0 * * * *"),
        _ => None,
    }
}

fn value(field: Field, text: &str) -> Result<u32, Reason> {
    let named = match field {
        Field::Month => MONTHS.iter().position(|m| text.eq_ignore_ascii_case(m)),
        Field::DayOfWeek => WEEKDAYS.iter().position(|d| text.eq_ignore_ascii_case(d)),
        Field::Minute | Field::Hour | Field::DayOfMonth => None,
    };
    let n = match named {
        // Month names start at 1, weekday names at 0.
        Some(i) if field == Field::Month => i as u32 + 1,
        Some(i) => i as u32,
        None => text
            .parse::<u32>()
            .map_err(|_| Reason::NotAValue(text.to_string()))?,
    };
    let (min, max) = field.range();
    if (min..=max).contains(&n) {
        Ok(n)
    } else {
        Err(Reason::OutOfRange { value: n, min, max })
    }
}

/// The values one comma-separated part selects.
fn part(field: Field, text: &str) -> Result<Vec<u32>, Reason> {
    if text.is_empty() {
        return Err(Reason::Empty);
    }
    let (base, step) = match text.split_once('/') {
        Some((base, step)) => {
            let step = step
                .parse::<u32>()
                .map_err(|_| Reason::NotAValue(step.to_string()))?;
            if step == 0 {
                return Err(Reason::ZeroStep);
            }
            (base, Some(step))
        }
        None => (text, None),
    };
    let (min, max) = field.range();
    let (from, to) = if base == "*" {
        // A weekday star covers 0-6: 7 would repeat Sunday.
        (min, if field == Field::DayOfWeek { 6 } else { max })
    } else if let Some((a, b)) = base.split_once('-') {
        let (a, b) = (value(field, a)?, value(field, b)?);
        if b < a {
            return Err(Reason::BackwardsRange(base.to_string()));
        }
        (a, b)
    } else {
        let a = value(field, base)?;
        // `5/10` runs from 5 to the end of the field, `5` is only 5.
        (a, if step.is_some() { max } else { a })
    };
    Ok((from..=to).step_by(step.unwrap_or(1) as usize).collect())
}

/// The bit set a field selects.
fn field_bits(field: Field, text: &str) -> Result<u64, CronError> {
    let mut bits = 0u64;
    for p in text.split(',') {
        for v in part(field, p).map_err(|reason| CronError::Field { field, reason })? {
            // Sunday is 0 and 7.
            let v = if field == Field::DayOfWeek && v == 7 {
                0
            } else {
                v
            };
            bits |= 1 << v;
        }
    }
    Ok(bits)
}

impl Schedule {
    pub fn parse(expression: &str) -> Result<Schedule, CronError> {
        let trimmed = expression.trim();
        if trimmed.is_empty() {
            return Err(CronError::Empty);
        }
        let expanded;
        let text = if trimmed.starts_with('@') {
            expanded = macro_expansion(&trimmed.to_ascii_lowercase())
                .ok_or_else(|| CronError::UnknownMacro(trimmed.to_string()))?;
            expanded
        } else {
            trimmed
        };
        let fields: Vec<&str> = text.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(CronError::FieldCount(fields.len()));
        }
        let mut bits = [0u64; 5];
        for (i, field) in Field::ALL.into_iter().enumerate() {
            bits[i] = field_bits(field, fields[i])?;
        }
        Ok(Schedule {
            minutes: bits[0],
            hours: bits[1] as u32,
            days: bits[2] as u32,
            months: bits[3] as u16,
            weekdays: bits[4] as u8,
            any_day: fields[2].starts_with('*'),
            any_weekday: fields[4].starts_with('*'),
        })
    }

    fn day_matches(&self, day: u32, weekday: u32) -> bool {
        let by_date = self.days & (1 << day) != 0;
        let by_weekday = self.weekdays & (1 << weekday) != 0;
        if self.any_day || self.any_weekday {
            by_date && by_weekday
        } else {
            by_date || by_weekday
        }
    }

    /// The first `count` run times after `from` (seconds since the epoch,
    /// UTC) in `zone`, as UTC instants. The search stops five years ahead,
    /// so a schedule that never fires, like `0 0 31 2 *`, returns fewer.
    pub fn next_runs(&self, from: i64, zone: Zone, count: usize) -> Vec<i64> {
        let horizon = from + 5 * 366 * 86_400;
        let mut out = Vec::with_capacity(count);
        // Local wall-clock seconds of the last run, so a repeated hour does
        // not run twice.
        let mut last_local = None;
        // The next whole minute after `from`.
        let mut t = from - from.rem_euclid(60) + 60;
        while out.len() < count && t <= horizon {
            let offset = zone.offset_secs(t);
            let c = civil(t, (offset / 60) as i16);
            let local = t + offset;
            let weekday = weekday(local) as u32;
            if self.months & (1 << c.month) == 0
                || !self.day_matches(c.day, weekday)
                || self.hours & (1 << c.hour) == 0
            {
                // Skip to the next local hour. Zone offsets are whole
                // minutes, so this lands on :00 local.
                t += i64::from(60 - c.minute) * 60;
                continue;
            }
            if self.minutes & (1 << c.minute) == 0 {
                t += 60;
                continue;
            }
            if last_local != Some(local) {
                out.push(t);
                last_local = Some(local);
            }
            t += 60;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::{date_minute, epoch};

    fn runs(expr: &str, from: i64, zone: Zone, n: usize) -> Vec<String> {
        Schedule::parse(expr)
            .expect("parses")
            .next_runs(from, zone, n)
            .into_iter()
            .map(|t| date_minute(t, 0))
            .collect()
    }

    fn now() -> i64 {
        epoch(2026, 6, 13, 13, 42, 30)
    }

    #[test]
    fn the_design_schedule_runs_at_two_every_night() {
        assert_eq!(
            runs("0 2 * * *", now(), Zone::Utc, 3),
            ["2026-06-14 02:00", "2026-06-15 02:00", "2026-06-16 02:00"]
        );
    }

    #[test]
    fn a_run_exactly_now_is_not_next() {
        let at = epoch(2026, 6, 14, 2, 0, 0);
        assert_eq!(runs("0 2 * * *", at, Zone::Utc, 1), ["2026-06-15 02:00"]);
        assert_eq!(
            runs("0 2 * * *", at - 1, Zone::Utc, 1),
            ["2026-06-14 02:00"]
        );
    }

    #[test]
    fn steps_lists_and_ranges() {
        assert_eq!(
            runs("*/20 9-10 * * *", now(), Zone::Utc, 4),
            [
                "2026-06-14 09:00",
                "2026-06-14 09:20",
                "2026-06-14 09:40",
                "2026-06-14 10:00"
            ]
        );
        assert_eq!(
            runs("5,35 14 * * *", now(), Zone::Utc, 3),
            ["2026-06-13 14:05", "2026-06-13 14:35", "2026-06-14 14:05"]
        );
        // `10/20` runs from 10 to the end of the hour.
        assert_eq!(
            runs("10/20 14 * * *", now(), Zone::Utc, 3),
            ["2026-06-13 14:10", "2026-06-13 14:30", "2026-06-13 14:50"]
        );
    }

    #[test]
    fn weekday_names_and_ranges_skip_the_weekend() {
        // 2026-06-13 is a Saturday.
        assert_eq!(
            runs("0 9 * * mon-fri", now(), Zone::Utc, 3),
            ["2026-06-15 09:00", "2026-06-16 09:00", "2026-06-17 09:00"]
        );
        assert_eq!(runs("0 0 * * 7", now(), Zone::Utc, 1), ["2026-06-14 00:00"]);
        assert_eq!(runs("0 0 * * 0", now(), Zone::Utc, 1), ["2026-06-14 00:00"]);
    }

    #[test]
    fn month_names_work() {
        assert_eq!(
            runs("0 0 1 JAN,jul *", now(), Zone::Utc, 2),
            ["2026-07-01 00:00", "2027-01-01 00:00"]
        );
    }

    #[test]
    fn restricted_day_fields_match_either() {
        // The 15th, or any Monday.
        let got = runs("0 0 15 * mon", now(), Zone::Utc, 3);
        assert_eq!(
            got,
            ["2026-06-15 00:00", "2026-06-22 00:00", "2026-06-29 00:00"]
        );
        // A star on one side makes the other the only filter.
        assert_eq!(
            runs("0 0 15 * *", now(), Zone::Utc, 2),
            ["2026-06-15 00:00", "2026-07-15 00:00"]
        );
    }

    #[test]
    fn leap_day_waits_for_a_leap_year() {
        assert_eq!(
            runs("0 0 29 2 *", now(), Zone::Utc, 1),
            ["2028-02-29 00:00"]
        );
    }

    #[test]
    fn an_impossible_date_never_runs() {
        assert!(runs("0 0 31 2 *", now(), Zone::Utc, 1).is_empty());
    }

    #[test]
    fn macros_expand() {
        assert_eq!(runs("@hourly", now(), Zone::Utc, 1), ["2026-06-13 14:00"]);
        assert_eq!(runs("@daily", now(), Zone::Utc, 1), ["2026-06-14 00:00"]);
        assert_eq!(runs("@weekly", now(), Zone::Utc, 1), ["2026-06-14 00:00"]);
        assert_eq!(runs("@monthly", now(), Zone::Utc, 1), ["2026-07-01 00:00"]);
        assert_eq!(runs("@yearly", now(), Zone::Utc, 1), ["2027-01-01 00:00"]);
    }

    #[test]
    fn a_zone_shifts_the_run_in_utc() {
        // 02:00 in Kuala Lumpur (+08:00) is 18:00 UTC the day before.
        assert_eq!(
            runs("0 2 * * *", now(), Zone::KualaLumpur, 2),
            ["2026-06-13 18:00", "2026-06-14 18:00"]
        );
        // Kolkata is +05:30.
        assert_eq!(
            runs("0 2 * * *", now(), Zone::Kolkata, 1),
            ["2026-06-13 20:30"]
        );
    }

    #[test]
    fn a_skipped_local_time_does_not_run() {
        // New York jumps from 02:00 to 03:00 on 2026-03-08, so 02:30 does
        // not exist that day. The runs are the 7th and the 9th.
        let from = epoch(2026, 3, 6, 12, 0, 0);
        assert_eq!(
            runs("30 2 * * *", from, Zone::NewYork, 3),
            ["2026-03-07 07:30", "2026-03-09 06:30", "2026-03-10 06:30"]
        );
    }

    #[test]
    fn a_repeated_local_time_runs_once() {
        // New York repeats 01:00-02:00 on 2026-11-01. 01:30 runs at its
        // first occurrence, 05:30 UTC, and not again at 06:30 UTC.
        let from = epoch(2026, 10, 31, 12, 0, 0);
        assert_eq!(
            runs("30 1 * * *", from, Zone::NewYork, 3),
            ["2026-11-01 05:30", "2026-11-02 06:30", "2026-11-03 06:30"]
        );
    }

    #[test]
    fn an_empty_schedule_is_refused() {
        assert_eq!(Schedule::parse("  "), Err(CronError::Empty));
    }

    #[test]
    fn the_field_count_is_checked() {
        assert_eq!(Schedule::parse("0 2 * *"), Err(CronError::FieldCount(4)));
        assert_eq!(
            Schedule::parse("0 0 2 * * *"),
            Err(CronError::FieldCount(6))
        );
    }

    #[test]
    fn errors_name_the_field_and_the_value() {
        let e = Schedule::parse("75 2 * * *").expect_err("out of range");
        assert_eq!(e.to_string(), "minute: `75` is outside 0-59");
        let e = Schedule::parse("0 24 * * *").expect_err("out of range");
        assert_eq!(e.to_string(), "hour: `24` is outside 0-23");
        let e = Schedule::parse("0 2 0 * *").expect_err("out of range");
        assert_eq!(e.to_string(), "day of month: `0` is outside 1-31");
        let e = Schedule::parse("0 2 * foo *").expect_err("bad name");
        assert_eq!(e.to_string(), "month: `foo` is not a number or a name");
        let e = Schedule::parse("0 2 * * 8").expect_err("out of range");
        assert_eq!(e.to_string(), "day of week: `8` is outside 0-7");
        let e = Schedule::parse("*/0 * * * *").expect_err("zero step");
        assert_eq!(e.to_string(), "minute: the step must be at least 1");
        let e = Schedule::parse("30-10 * * * *").expect_err("backwards");
        assert_eq!(
            e.to_string(),
            "minute: the range `30-10` ends before it starts"
        );
        let e = Schedule::parse("1,,2 * * * *").expect_err("empty part");
        assert_eq!(e.to_string(), "minute: has an empty entry");
    }

    #[test]
    fn an_unknown_macro_is_refused() {
        assert_eq!(
            Schedule::parse("@fortnightly"),
            Err(CronError::UnknownMacro("@fortnightly".into()))
        );
    }

    #[test]
    fn extensions_the_server_does_not_schedule_are_refused() {
        assert!(Schedule::parse("0 0 L * *").is_err());
        assert!(Schedule::parse("0 0 ? * *").is_err());
        assert!(Schedule::parse("0 0 * * 1#2").is_err());
    }
}
