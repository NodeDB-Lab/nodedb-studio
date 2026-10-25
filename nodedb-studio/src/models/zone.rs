//! Time zones a scheduled job can run in.
//!
//! Studio carries no time-zone database. Each zone here is a fixed offset
//! plus, for the US and EU zones, their published daylight-saving rule. A
//! server that supports more zones needs its own entry here before a job can
//! name it.

use crate::models::format::epoch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    Utc,
    KualaLumpur,
    Tokyo,
    Kolkata,
    NewYork,
    London,
    Berlin,
}

/// How a zone changes its offset through the year.
enum Rule {
    None,
    /// Second Sunday of March to first Sunday of November, at 02:00 local.
    Us,
    /// Last Sunday of March to last Sunday of October, at 01:00 UTC.
    Eu,
}

impl Zone {
    pub const ALL: [Zone; 7] = [
        Zone::Utc,
        Zone::KualaLumpur,
        Zone::Tokyo,
        Zone::Kolkata,
        Zone::NewYork,
        Zone::London,
        Zone::Berlin,
    ];

    /// The IANA name, which is what a job stores.
    pub fn name(self) -> &'static str {
        match self {
            Zone::Utc => "UTC",
            Zone::KualaLumpur => "Asia/Kuala_Lumpur",
            Zone::Tokyo => "Asia/Tokyo",
            Zone::Kolkata => "Asia/Kolkata",
            Zone::NewYork => "America/New_York",
            Zone::London => "Europe/London",
            Zone::Berlin => "Europe/Berlin",
        }
    }

    pub fn from_name(name: &str) -> Option<Zone> {
        Zone::ALL.into_iter().find(|z| z.name() == name)
    }

    /// Offset from UTC in seconds, outside daylight time.
    fn standard_secs(self) -> i64 {
        match self {
            Zone::Utc | Zone::London => 0,
            Zone::KualaLumpur => 8 * 3600,
            Zone::Tokyo => 9 * 3600,
            Zone::Kolkata => 5 * 3600 + 1800,
            Zone::NewYork => -5 * 3600,
            Zone::Berlin => 3600,
        }
    }

    fn rule(self) -> Rule {
        match self {
            Zone::Utc | Zone::KualaLumpur | Zone::Tokyo | Zone::Kolkata => Rule::None,
            Zone::NewYork => Rule::Us,
            Zone::London | Zone::Berlin => Rule::Eu,
        }
    }

    /// Offset from UTC in seconds at the instant `utc`.
    pub fn offset_secs(self, utc: i64) -> i64 {
        let standard = self.standard_secs();
        let year = year_of(utc);
        let daylight = match self.rule() {
            Rule::None => false,
            Rule::Us => {
                // Both changes happen at 02:00 on the wall clock.
                let start = nth_sunday(year, 3, 2) + 2 * 3600 - standard;
                let end = nth_sunday(year, 11, 1) + 2 * 3600 - (standard + 3600);
                (start..end).contains(&utc)
            }
            Rule::Eu => {
                let start = last_sunday(year, 3) + 3600;
                let end = last_sunday(year, 10) + 3600;
                (start..end).contains(&utc)
            }
        };
        standard + if daylight { 3600 } else { 0 }
    }
}

/// The calendar year of `utc`.
fn year_of(utc: i64) -> i64 {
    crate::models::format::civil(utc, 0).year
}

/// Seconds at 00:00 UTC of the `n`th Sunday (1-based) of a month.
fn nth_sunday(year: i64, month: u32, n: i64) -> i64 {
    let first = epoch(year, month, 1, 0, 0, 0);
    let to_sunday = (7 - weekday(first)) % 7;
    first + (to_sunday + 7 * (n - 1)) * 86_400
}

/// Seconds at 00:00 UTC of the last Sunday of a month.
fn last_sunday(year: i64, month: u32) -> i64 {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let next_first = epoch(y, m, 1, 0, 0, 0);
    let back = weekday(next_first - 86_400);
    next_first - 86_400 - back * 86_400
}

/// Day of the week of the UTC day holding `secs`: Sunday is 0.
pub fn weekday(secs: i64) -> i64 {
    // 1970-01-01 was a Thursday.
    (secs.div_euclid(86_400) + 4).rem_euclid(7)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for z in Zone::ALL {
            assert_eq!(Zone::from_name(z.name()), Some(z));
        }
        assert_eq!(Zone::from_name("Mars/Olympus"), None);
    }

    #[test]
    fn weekdays_match_known_dates() {
        assert_eq!(weekday(epoch(1970, 1, 1, 0, 0, 0)), 4);
        // 2026-06-14 is a Sunday.
        assert_eq!(weekday(epoch(2026, 6, 14, 12, 0, 0)), 0);
        assert_eq!(weekday(epoch(2026, 6, 13, 12, 0, 0)), 6);
    }

    #[test]
    fn fixed_zones_never_change() {
        for month in [1, 7] {
            let at = epoch(2026, month, 15, 12, 0, 0);
            assert_eq!(Zone::Utc.offset_secs(at), 0);
            assert_eq!(Zone::KualaLumpur.offset_secs(at), 8 * 3600);
            assert_eq!(Zone::Kolkata.offset_secs(at), 19_800);
        }
    }

    #[test]
    fn us_daylight_time_runs_from_second_sunday_of_march() {
        // 2026: starts Sunday March 8 at 02:00 EST (07:00 UTC).
        assert_eq!(
            Zone::NewYork.offset_secs(epoch(2026, 3, 8, 6, 59, 0)),
            -5 * 3600
        );
        assert_eq!(
            Zone::NewYork.offset_secs(epoch(2026, 3, 8, 7, 0, 0)),
            -4 * 3600
        );
        // Ends Sunday November 1 at 02:00 EDT (06:00 UTC).
        assert_eq!(
            Zone::NewYork.offset_secs(epoch(2026, 11, 1, 5, 59, 0)),
            -4 * 3600
        );
        assert_eq!(
            Zone::NewYork.offset_secs(epoch(2026, 11, 1, 6, 0, 0)),
            -5 * 3600
        );
    }

    #[test]
    fn eu_daylight_time_runs_between_the_last_sundays() {
        // 2026: starts Sunday March 29 at 01:00 UTC, ends Sunday October 25.
        assert_eq!(Zone::Berlin.offset_secs(epoch(2026, 3, 29, 0, 59, 0)), 3600);
        assert_eq!(Zone::Berlin.offset_secs(epoch(2026, 3, 29, 1, 0, 0)), 7200);
        assert_eq!(Zone::London.offset_secs(epoch(2026, 7, 1, 0, 0, 0)), 3600);
        assert_eq!(Zone::London.offset_secs(epoch(2026, 10, 25, 1, 0, 0)), 0);
        assert_eq!(Zone::London.offset_secs(epoch(2026, 12, 25, 0, 0, 0)), 0);
    }
}
