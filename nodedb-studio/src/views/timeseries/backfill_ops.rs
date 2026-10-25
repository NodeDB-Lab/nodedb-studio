//! Backfill job rules: what a form edit does to the preview, which preview
//! a run may use, and the target-key choices. Plain functions, tested
//! without a renderer.

use crate::models::timeseries::{Bucket, TimeSpan, parse_utc};
use crate::models::ts_backfill::{BackfillPreview, BackfillSpec};
use crate::models::ts_lifecycle::Lifecycle;
use crate::state::timeseries::{BackfillUi, PreviewState};

/// `day`, `hour`, or `minute`: the word a bucket's key starts with.
pub fn bucket_word(b: Bucket) -> &'static str {
    match b {
        Bucket::Day1 => "day",
        Bucket::Hour6 | Bucket::Hour1 => "hour",
        Bucket::Minute15 | Bucket::Minute5 | Bucket::Minute1 => "minute",
    }
}

/// The keys a bucket of `target` can be identified by: the bucket alone, then
/// the bucket with each growing prefix of its dimensions.
pub fn target_key_options(l: &Lifecycle, target: &str) -> Vec<String> {
    let Some(r) = l.rollup(target) else {
        return Vec::new();
    };
    let word = bucket_word(r.bucket);
    let mut out = vec![word.to_string()];
    let mut key = word.to_string();
    for dim in &r.group_by {
        key = format!("{key} + {dim}");
        out.push(key.clone());
    }
    out
}

/// Point the job at another rollup: its bucket, outputs, and the widest key
/// it has. What was typed for the old target no longer applies.
pub fn set_target(ui: &mut BackfillUi, l: &Lifecycle, target: &str) {
    let Some(r) = l.rollup(target) else { return };
    ui.spec.target = r.name.clone();
    ui.spec.bucket = r.bucket;
    ui.spec.aggregates = r.outputs.clone();
    ui.spec.target_key = target_key_options(l, target)
        .last()
        .cloned()
        .unwrap_or_default();
    ui.preview = PreviewState::Draft;
}

/// Read the range boxes into the spec. Returns why they do not parse.
pub fn apply_range(ui: &mut BackfillUi) -> Result<(), &'static str> {
    match (parse_utc(&ui.start), parse_utc(&ui.end)) {
        (Some(a), Some(b)) if b > a => {
            ui.spec.range = TimeSpan::new(a, b);
            Ok(())
        }
        (Some(_), Some(_)) => Err("The end must be after the start."),
        _ => Err("Use YYYY-MM-DD HH:MM, in UTC."),
    }
}

/// The preview a run may use: one that answers this spec exactly.
pub fn valid_preview(ui: &BackfillUi) -> Option<&BackfillPreview> {
    match &ui.preview {
        PreviewState::Ready(p) if p.revision == ui.spec.preview_revision() => Some(p),
        PreviewState::Ready(_)
        | PreviewState::Draft
        | PreviewState::Running(_)
        | PreviewState::Failed(_) => None,
    }
}

/// Whether a preview exists but no longer matches the form.
pub fn preview_outdated(ui: &BackfillUi) -> bool {
    matches!(&ui.preview, PreviewState::Ready(p) if p.revision != ui.spec.preview_revision())
}

/// The title of the bucket table.
pub fn buckets_title(spec: &BackfillSpec) -> String {
    format!("Affected {} buckets", bucket_word(spec.bucket))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;

    async fn lifecycle() -> Lifecycle {
        MockConnectionService::ready()
            .lifecycle("sales")
            .await
            .expect("l")
    }

    #[tokio::test]
    async fn the_key_options_grow_by_one_dimension_each() {
        let l = lifecycle().await;
        assert_eq!(
            target_key_options(&l, "sales_daily"),
            ["day", "day + store_id", "day + store_id + currency"]
        );
        assert_eq!(target_key_options(&l, "sales_hourly")[0], "hour");
        assert!(target_key_options(&l, "gone").is_empty());
    }

    #[tokio::test]
    async fn a_new_target_takes_its_bucket_outputs_and_widest_key() {
        let l = lifecycle().await;
        let mut ui = BackfillUi::standard();
        set_target(&mut ui, &l, "sales_hourly");
        assert_eq!(ui.spec.bucket, Bucket::Hour1);
        assert_eq!(ui.spec.target_key, "hour + store_id + currency");
        assert_eq!(ui.spec.aggregates.len(), 2);
        assert_eq!(ui.preview, PreviewState::Draft);
    }

    #[test]
    fn the_range_boxes_parse_into_the_spec_or_say_why_not() {
        let mut ui = BackfillUi::standard();
        ui.start = "2026-06-02 00:00".into();
        ui.end = "2026-06-09 00:00".into();
        assert_eq!(apply_range(&mut ui), Ok(()));
        assert_eq!(ui.spec.range.end - ui.spec.range.start, 7 * 86_400);
        ui.end = "2026-06-01 00:00".into();
        assert!(apply_range(&mut ui).is_err());
        ui.end = "tomorrow".into();
        assert_eq!(apply_range(&mut ui), Err("Use YYYY-MM-DD HH:MM, in UTC."));
    }

    #[tokio::test]
    async fn only_a_preview_of_this_exact_spec_may_run() {
        let svc = MockConnectionService::ready();
        let mut ui = BackfillUi::standard();
        let p = svc.backfill_preview(&ui.spec).await.expect("preview");
        ui.preview = PreviewState::Ready(p);
        assert!(valid_preview(&ui).is_some());
        assert!(!preview_outdated(&ui));
        ui.spec.currency = None;
        assert!(valid_preview(&ui).is_none());
        assert!(preview_outdated(&ui));
        // A change that is not part of the result keeps it.
        let mut ui2 = BackfillUi::standard();
        ui2.preview = PreviewState::Ready(svc.backfill_preview(&ui2.spec).await.expect("preview"));
        ui2.spec.job_name = "other".into();
        ui2.spec.description = "x".into();
        assert!(valid_preview(&ui2).is_some());
    }

    #[test]
    fn the_table_is_titled_by_its_bucket() {
        let mut ui = BackfillUi::standard();
        assert_eq!(buckets_title(&ui.spec), "Affected day buckets");
        ui.spec.bucket = Bucket::Hour1;
        assert_eq!(buckets_title(&ui.spec), "Affected hour buckets");
    }
}
