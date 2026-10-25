//! Lifecycle drafts: the rules for building a rollup and queueing changes
//! for review. Plain functions, so each rule is tested without a renderer.

use crate::models::ts_lifecycle::{
    Lifecycle, LifecycleChange, RollupDraft, RollupFn, RollupOutput,
};
use crate::state::timeseries::LifeUi;

/// The name a stored output gets when the person ticks it.
pub fn default_alias(func: RollupFn, column: Option<&str>) -> String {
    match (func, column) {
        (RollupFn::Count, _) | (_, None) => "row_count".to_string(),
        (RollupFn::Sum, Some(c)) => c.to_string(),
        (RollupFn::Min, Some(c)) => format!("min_{c}"),
        (RollupFn::Max, Some(c)) => format!("max_{c}"),
    }
}

/// The outputs a rollup over `input` can store. Over the collection: any
/// aggregate of a numeric column, and a row count. Over a rollup: only what
/// that rollup stored, because a rollup merges stored state.
pub fn candidate_outputs(l: &Lifecycle, input: &str) -> Vec<RollupOutput> {
    if let Some(r) = l.rollup(input) {
        return r
            .outputs
            .iter()
            .map(|o| RollupOutput {
                func: o.func,
                column: o.column.clone(),
                alias: o.alias.clone(),
            })
            .collect();
    }
    let mut out = vec![RollupOutput {
        func: RollupFn::Count,
        column: None,
        alias: default_alias(RollupFn::Count, None),
    }];
    for c in &l.numeric_fields {
        for f in RollupFn::ALL {
            if f != RollupFn::Count {
                out.push(RollupOutput {
                    func: f,
                    column: Some(c.clone()),
                    alias: default_alias(f, Some(c)),
                });
            }
        }
    }
    out
}

/// The dimensions a rollup over `input` can keep.
pub fn candidate_dimensions(l: &Lifecycle, input: &str) -> Vec<String> {
    match l.rollup(input) {
        Some(r) => r.group_by.clone(),
        None => l.group_fields.clone(),
    }
}

/// Tick or untick an output.
pub fn toggle_output(d: &mut RollupDraft, o: &RollupOutput) {
    match d
        .outputs
        .iter()
        .position(|x| x.func == o.func && x.column == o.column)
    {
        Some(at) => {
            d.outputs.remove(at);
        }
        None => d.outputs.push(o.clone()),
    }
}

/// Tick or untick a dimension.
pub fn toggle_dimension(d: &mut RollupDraft, dim: &str) {
    match d.group_by.iter().position(|g| g == dim) {
        Some(at) => {
            d.group_by.remove(at);
        }
        None => d.group_by.push(dim.to_string()),
    }
}

/// Point the draft at another input. What it ticked belonged to the old
/// input and may not exist on the new one, so outputs and dimensions reset
/// to the new input's.
pub fn set_input(d: &mut RollupDraft, l: &Lifecycle, input: &str) {
    d.input = input.to_string();
    d.outputs.clear();
    d.group_by = candidate_dimensions(l, input);
    d.allow_lossy = false;
    if let Some(r) = l.rollup(input) {
        // The next coarser bucket than the input's, so the draft starts valid.
        if let Some(next) = crate::models::timeseries::Bucket::ALL
            .into_iter()
            .find(|b| b.seconds() > r.bucket.seconds() && b.seconds() % r.bucket.seconds() == 0)
        {
            d.bucket = next;
        }
    }
}

/// Queue a change for review. A retention or late-data change replaces the
/// one already waiting: only the last wish counts. A rollup of the same
/// name replaces its earlier draft.
pub fn queue(ui: &mut LifeUi, change: LifecycleChange) {
    match &change {
        LifecycleChange::Retention(_) => ui
            .pending
            .retain(|c| !matches!(c, LifecycleChange::Retention(_))),
        LifecycleChange::LatePolicy(_) => ui
            .pending
            .retain(|c| !matches!(c, LifecycleChange::LatePolicy(_))),
        LifecycleChange::CreateRollup(d) => ui.pending.retain(
            |c| !matches!(c, LifecycleChange::CreateRollup(x) if x.name.trim() == d.name.trim()),
        ),
    }
    ui.pending.push(change);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ts_lifecycle::{Retention, RetentionDraft};
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;

    async fn lifecycle() -> Lifecycle {
        MockConnectionService::ready()
            .lifecycle("sales")
            .await
            .expect("lifecycle")
    }

    #[tokio::test]
    async fn the_collection_offers_every_aggregate_of_its_numbers_and_a_row_count() {
        let l = lifecycle().await;
        let c = candidate_outputs(&l, "sales");
        assert!(c.iter().any(|o| o.logical() == "COUNT(*)"));
        assert!(c.iter().any(|o| o.logical() == "SUM(revenue)"));
        assert!(c.iter().any(|o| o.logical() == "MAX(quantity)"));
        let mut aliases: Vec<&str> = c.iter().map(|o| o.alias.as_str()).collect();
        aliases.sort_unstable();
        aliases.dedup();
        assert_eq!(aliases.len(), c.len(), "aliases are unique");
    }

    #[tokio::test]
    async fn a_rollup_offers_only_what_it_stored() {
        let l = lifecycle().await;
        let c = candidate_outputs(&l, "sales_hourly");
        let logical: Vec<String> = c.iter().map(|o| o.logical()).collect();
        assert_eq!(logical, ["SUM(revenue)", "COUNT(*)"]);
        assert_eq!(
            candidate_dimensions(&l, "sales_hourly"),
            ["store_id", "currency"]
        );
    }

    #[tokio::test]
    async fn changing_the_input_resets_what_was_ticked_and_picks_a_valid_bucket() {
        let l = lifecycle().await;
        let mut d = RollupDraft::blank(&l);
        toggle_output(&mut d, &candidate_outputs(&l, "sales")[0]);
        set_input(&mut d, &l, "sales_hourly");
        assert!(d.outputs.is_empty());
        assert_eq!(d.group_by, ["store_id", "currency"]);
        assert!(d.bucket.seconds() > 3_600 && d.bucket.seconds() % 3_600 == 0);
    }

    #[tokio::test]
    async fn ticking_twice_unticks() {
        let l = lifecycle().await;
        let mut d = RollupDraft::blank(&l);
        let o = candidate_outputs(&l, "sales")[1].clone();
        toggle_output(&mut d, &o);
        assert_eq!(d.outputs.len(), 1);
        toggle_output(&mut d, &o);
        assert!(d.outputs.is_empty());
        let before = d.group_by.clone();
        toggle_dimension(&mut d, "store_id");
        assert_eq!(d.group_by.len(), before.len() - 1);
        toggle_dimension(&mut d, "store_id");
        assert_eq!(d.group_by.len(), before.len());
    }

    #[tokio::test]
    async fn a_later_change_of_the_same_kind_replaces_the_earlier_one() {
        let l = lifecycle().await;
        let mut ui = LifeUi::default();
        let mut a = RetentionDraft::from(&l);
        a.raw_keep = Retention::Days(30);
        let mut b = RetentionDraft::from(&l);
        b.raw_keep = Retention::Days(60);
        queue(&mut ui, LifecycleChange::Retention(a));
        queue(&mut ui, LifecycleChange::LatePolicy(l.late));
        queue(&mut ui, LifecycleChange::Retention(b.clone()));
        assert_eq!(ui.pending.len(), 2);
        assert!(ui.pending.contains(&LifecycleChange::Retention(b)));
    }

    #[tokio::test]
    async fn two_rollups_with_different_names_both_wait() {
        let l = lifecycle().await;
        let mut ui = LifeUi::default();
        let mut d = RollupDraft::blank(&l);
        d.name = "a".into();
        queue(&mut ui, LifecycleChange::CreateRollup(d.clone()));
        d.name = "b".into();
        queue(&mut ui, LifecycleChange::CreateRollup(d.clone()));
        queue(&mut ui, LifecycleChange::CreateRollup(d));
        assert_eq!(ui.pending.len(), 2);
    }
}
