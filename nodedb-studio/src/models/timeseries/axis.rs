//! Axis ticks and number formats for the Timeseries charts. Plain Rust, so
//! the choices are tested: the chart only draws what these return.

use crate::models::format::{civil, thousands};
use crate::models::timeseries::Bucket;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The value axis for data that tops out at `max`: the ticks, from zero up,
/// at a round step. The last tick is the axis top.
pub fn value_ticks(max: f64, target: usize) -> Vec<f64> {
    if !max.is_finite() || max <= 0.0 {
        return vec![0.0, 1.0];
    }
    let raw = max / target.max(1) as f64;
    let magnitude = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * magnitude);
    let count = (max / step).ceil() as usize;
    (0..=count).map(|i| i as f64 * step).collect()
}

/// An axis tick: whole numbers with separators, short forms past a million.
pub fn axis_number(v: f64) -> String {
    let a = v.abs();
    if a >= 1_000_000.0 {
        let m = v / 1_000_000.0;
        return if (m - m.round()).abs() < 1e-9 {
            format!("{}M", m.round() as i64)
        } else {
            format!("{m:.1}M")
        };
    }
    if (v - v.round()).abs() < 1e-9 {
        return signed(v.round() as i64);
    }
    // A step below one: show the digits the step needs.
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn signed(n: i64) -> String {
    let body = thousands(n.unsigned_abs());
    if n < 0 { format!("-{body}") } else { body }
}

/// A table value with `decimals` places and thousands separators:
/// `1,800.00`.
pub fn fixed(v: f64, decimals: usize) -> String {
    let negative = v < 0.0;
    let text = format!("{:.*}", decimals, v.abs());
    let (whole, frac) = match text.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (text.as_str(), None),
    };
    let whole = whole.parse::<u64>().map(thousands).unwrap_or_default();
    let sign = if negative && text.chars().any(|c| c.is_ascii_digit() && c != '0') {
        "-"
    } else {
        ""
    };
    match frac {
        Some(f) => format!("{sign}{whole}.{f}"),
        None => format!("{sign}{whole}"),
    }
}

/// Which bucket indexes get an x label, so labels never collide:
/// every `k`th bucket, starting with the first.
pub fn label_stride(buckets: usize, max_labels: usize) -> usize {
    if buckets == 0 || max_labels == 0 {
        return 1;
    }
    buckets.div_ceil(max_labels).max(1)
}

/// The x label of a bucket start. Day buckets read `Jun 07`. Finer buckets
/// read the time, with the date when the axis spans more than one day.
pub fn x_label(start: i64, bucket: Bucket, spans_days: bool) -> String {
    let c = civil(start, 0);
    let month = MONTHS[(c.month as usize).saturating_sub(1).min(11)];
    if bucket.seconds() >= 86_400 {
        return format!("{month} {:02}", c.day);
    }
    if spans_days {
        format!("{month} {:02} {:02}:{:02}", c.day, c.hour, c.minute)
    } else {
        format!("{:02}:{:02}", c.hour, c.minute)
    }
}

/// Whether the bucket starts fall on more than one UTC day.
pub fn spans_days(starts: &[i64]) -> bool {
    match (starts.first(), starts.last()) {
        (Some(a), Some(b)) => a.div_euclid(86_400) != b.div_euclid(86_400),
        _ => false,
    }
}

/// `13 Jun 2026 10:30` for a tooltip or a table.
pub fn long_stamp(at: i64) -> String {
    let c = civil(at, 0);
    let month = MONTHS[(c.month as usize).saturating_sub(1).min(11)];
    format!(
        "{:02} {month} {} {:02}:{:02}",
        c.day, c.year, c.hour, c.minute
    )
}

/// A range for a toolbar: `13 Jun 2026 · 10:00–11:00` within one day,
/// else both ends in full.
pub fn range_label(start: i64, end: i64) -> String {
    let (a, b) = (civil(start, 0), civil(end, 0));
    let same_day = start.div_euclid(86_400) == (end - 1).div_euclid(86_400);
    if same_day {
        let month = MONTHS[(a.month as usize).saturating_sub(1).min(11)];
        let close = if end.rem_euclid(86_400) == 0 {
            "24:00".to_string()
        } else {
            format!("{:02}:{:02}", b.hour, b.minute)
        };
        format!(
            "{:02} {month} {} · {:02}:{:02}–{close}",
            a.day, a.year, a.hour, a.minute
        )
    } else {
        format!("{} – {}", long_stamp(start), long_stamp(end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;

    #[test]
    fn value_ticks_start_at_zero_and_cover_the_maximum() {
        let t = value_ticks(2_950.0, 4);
        assert_eq!(t.first(), Some(&0.0));
        assert!(t.last().copied().unwrap_or(0.0) >= 2_950.0);
        assert!(t.len() >= 3 && t.len() <= 8, "{t:?}");
    }

    #[test]
    fn value_ticks_use_round_steps() {
        assert_eq!(value_ticks(3_000.0, 3), [0.0, 1_000.0, 2_000.0, 3_000.0]);
        assert_eq!(value_ticks(9.0, 3), [0.0, 5.0, 10.0]);
        assert_eq!(value_ticks(21.0, 3), [0.0, 10.0, 20.0, 30.0]);
    }

    #[test]
    fn an_empty_or_zero_range_still_has_an_axis() {
        assert_eq!(value_ticks(0.0, 4), [0.0, 1.0]);
        assert_eq!(value_ticks(f64::NAN, 4), [0.0, 1.0]);
    }

    #[test]
    fn axis_numbers_group_digits() {
        assert_eq!(axis_number(3_000.0), "3,000");
        assert_eq!(axis_number(0.0), "0");
        assert_eq!(axis_number(2_500_000.0), "2.5M");
        assert_eq!(axis_number(0.5), "0.5");
    }

    #[test]
    fn fixed_numbers_keep_their_places() {
        assert_eq!(fixed(1800.0, 2), "1,800.00");
        assert_eq!(fixed(59.97, 2), "59.97");
        assert_eq!(fixed(12.0, 0), "12");
        assert_eq!(fixed(-3.5, 1), "-3.5");
    }

    #[test]
    fn labels_thin_out_to_fit() {
        assert_eq!(label_stride(12, 12), 1);
        assert_eq!(label_stride(24, 12), 2);
        assert_eq!(label_stride(25, 12), 3);
        assert_eq!(label_stride(0, 12), 1);
    }

    #[test]
    fn x_labels_follow_the_bucket() {
        let t = epoch(2026, 6, 7, 10, 30, 0);
        assert_eq!(x_label(t, Bucket::Day1, false), "Jun 07");
        assert_eq!(x_label(t, Bucket::Minute5, false), "10:30");
        assert_eq!(x_label(t, Bucket::Hour1, true), "Jun 07 10:30");
    }

    #[test]
    fn a_range_within_one_day_does_not_span_days() {
        let a = epoch(2026, 6, 7, 0, 0, 0);
        assert!(!spans_days(&[a, a + 3_600]));
        assert!(spans_days(&[a, a + 86_400]));
    }

    #[test]
    fn a_range_within_a_day_reads_as_one_date_and_two_times() {
        let a = epoch(2026, 6, 13, 10, 0, 0);
        assert_eq!(range_label(a, a + 3_600), "13 Jun 2026 · 10:00–11:00");
        let day = epoch(2026, 6, 13, 0, 0, 0);
        assert_eq!(range_label(day, day + 86_400), "13 Jun 2026 · 00:00–24:00");
        assert_eq!(
            range_label(day, day + 3 * 86_400),
            "13 Jun 2026 00:00 – 16 Jun 2026 00:00"
        );
    }

    #[test]
    fn long_stamps_read_as_dates() {
        assert_eq!(
            long_stamp(epoch(2026, 6, 13, 10, 30, 0)),
            "13 Jun 2026 10:30"
        );
    }
}
