//! Display formatting shared by every screen.

/// `1488201` → `1,488,201`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Seconds since an observation, as `4s ago`, `4m ago`, `4h ago`, `4d ago`.
pub fn ago(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s ago"),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

/// A span of seconds as `45s`, `1m 22s`, `2h 5m`, or `3d 4h`. Zero units drop.
pub fn duration(secs: u64) -> String {
    let (d, h, m, s) = (
        secs / 86_400,
        secs % 86_400 / 3600,
        secs % 3600 / 60,
        secs % 60,
    );
    let parts: [(u64, &str); 4] = [(d, "d"), (h, "h"), (m, "m"), (s, "s")];
    let shown: Vec<String> = parts
        .iter()
        .filter(|(n, _)| *n > 0)
        .take(2)
        .map(|(n, u)| format!("{n}{u}"))
        .collect();
    if shown.is_empty() {
        "0s".to_string()
    } else {
        shown.join(" ")
    }
}

/// A large count in short form: `18.4M`, `3.2M`, `482K`, `912`.
pub fn compact(n: u64) -> String {
    let short = |value: f64, unit: &str| {
        let text = format!("{value:.1}");
        format!("{}{unit}", text.strip_suffix(".0").unwrap_or(&text))
    };
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => short(n as f64 / 1_000.0, "K"),
        _ => short(n as f64 / 1_000_000.0, "M"),
    }
}

/// Seconds since 1970-01-01 UTC for a civil date and time. Fixtures write
/// their timestamps with it, so the values read as dates.
pub const fn epoch(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64
}

/// A calendar date and clock time, in the zone it was converted to.
pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// `at` (seconds since the epoch, UTC) shifted by `offset_min` minutes.
pub fn civil(at: i64, offset_min: i16) -> Civil {
    let local = at + i64::from(offset_min) * 60;
    let days = local.div_euclid(86_400);
    let secs = local.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    Civil {
        year,
        month,
        day,
        hour: (secs / 3600) as u32,
        minute: (secs % 3600 / 60) as u32,
        second: (secs % 60) as u32,
    }
}

/// `2026-06-13`.
pub fn date(at: i64, offset_min: i16) -> String {
    let c = civil(at, offset_min);
    format!("{:04}-{:02}-{:02}", c.year, c.month, c.day)
}

/// `2026-06-13 13:21`.
pub fn date_minute(at: i64, offset_min: i16) -> String {
    let c = civil(at, offset_min);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute
    )
}

/// `2026-06-13 13:21:00`.
pub fn date_second(at: i64, offset_min: i16) -> String {
    let c = civil(at, offset_min);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// `13:21`.
pub fn clock(at: i64, offset_min: i16) -> String {
    let c = civil(at, offset_min);
    format!("{:02}:{:02}", c.hour, c.minute)
}

/// Whether two instants fall on the same calendar day in the zone.
pub fn same_day(a: i64, b: i64, offset_min: i16) -> bool {
    let (a, b) = (civil(a, offset_min), civil(b, offset_min));
    (a.year, a.month, a.day) == (b.year, b.month, b.day)
}

/// `13:21 UTC` on the reference day, else `06-12 21:40 UTC`.
pub fn stamp(at: i64, reference: i64, offset_min: i16) -> String {
    let zone = offset_label(offset_min);
    if same_day(at, reference, offset_min) {
        format!("{} {zone}", clock(at, offset_min))
    } else {
        let c = civil(at, offset_min);
        format!(
            "{:02}-{:02} {} {zone}",
            c.month,
            c.day,
            clock(at, offset_min)
        )
    }
}

/// `UTC` or `UTC+08:00`.
pub fn offset_label(offset_min: i16) -> String {
    if offset_min == 0 {
        return "UTC".to_string();
    }
    let sign = if offset_min < 0 { '-' } else { '+' };
    let abs = offset_min.unsigned_abs();
    format!("UTC{sign}{:02}:{:02}", abs / 60, abs % 60)
}

/// Whole days from `from` to `to`, rounded down. Negative when `to` is earlier.
pub fn days_between(from: i64, to: i64) -> i64 {
    (to - from).div_euclid(86_400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1_488_201), "1,488,201");
    }

    #[test]
    fn ago_picks_the_largest_whole_unit() {
        assert_eq!(ago(1), "1s ago");
        assert_eq!(ago(240), "4m ago");
        assert_eq!(ago(7200), "2h ago");
        assert_eq!(ago(172_800), "2d ago");
    }

    #[test]
    fn durations_keep_the_two_largest_units() {
        assert_eq!(duration(0), "0s");
        assert_eq!(duration(14), "14s");
        assert_eq!(duration(82), "1m 22s");
        assert_eq!(duration(300), "5m");
        assert_eq!(duration(7500), "2h 5m");
        assert_eq!(duration(90_000), "1d 1h");
    }

    #[test]
    fn compact_counts_read_short() {
        assert_eq!(compact(912), "912");
        assert_eq!(compact(482_613), "482.6K");
        assert_eq!(compact(3_200_000), "3.2M");
        assert_eq!(compact(18_400_000), "18.4M");
        assert_eq!(compact(2_000_000), "2M");
    }

    #[test]
    fn epoch_matches_known_instants() {
        assert_eq!(epoch(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(epoch(2000, 3, 1, 0, 0, 0), 951_868_800);
        assert_eq!(epoch(2026, 6, 13, 13, 21, 0), 1_781_356_860);
    }

    #[test]
    fn dates_round_trip_through_epoch() {
        let at = epoch(2026, 6, 13, 13, 21, 5);
        assert_eq!(date(at, 0), "2026-06-13");
        assert_eq!(date_minute(at, 0), "2026-06-13 13:21");
        assert_eq!(date_second(at, 0), "2026-06-13 13:21:05");
        assert_eq!(clock(at, 0), "13:21");
        // Leap day and a year boundary.
        assert_eq!(date(epoch(2024, 2, 29, 23, 59, 59), 0), "2024-02-29");
        assert_eq!(date(epoch(2025, 12, 31, 23, 59, 59) + 1, 0), "2026-01-01");
    }

    #[test]
    fn offset_shifts_the_clock_and_the_day() {
        let at = epoch(2026, 6, 13, 20, 0, 0);
        assert_eq!(date_minute(at, 480), "2026-06-14 04:00");
        assert_eq!(date_minute(at, -300), "2026-06-13 15:00");
        assert!(!same_day(at, epoch(2026, 6, 13, 12, 0, 0), 480));
    }

    #[test]
    fn offset_labels() {
        assert_eq!(offset_label(0), "UTC");
        assert_eq!(offset_label(480), "UTC+08:00");
        assert_eq!(offset_label(-330), "UTC-05:30");
    }

    #[test]
    fn stamp_drops_the_date_on_the_reference_day() {
        let now = epoch(2026, 6, 13, 13, 21, 0);
        assert_eq!(stamp(now, now, 0), "13:21 UTC");
        assert_eq!(
            stamp(epoch(2026, 6, 12, 21, 40, 0), now, 0),
            "06-12 21:40 UTC"
        );
    }

    #[test]
    fn days_between_rounds_down() {
        let a = epoch(2026, 6, 13, 13, 21, 0);
        assert_eq!(days_between(a, epoch(2026, 12, 10, 0, 0, 0)), 179);
        assert_eq!(days_between(a, a), 0);
        assert_eq!(days_between(a, a - 1), -1);
    }
}
