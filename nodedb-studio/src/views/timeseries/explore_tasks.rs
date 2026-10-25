//! Explore: the draft rules and the writes that outlive a click. Each async
//! task runs under `spawn_forever` and touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;
use dioxus::router::Navigator;

use crate::models::saved_query::SaveQuery;
use crate::models::timeseries::ChartKind;
use crate::models::timeseries::{ExploreQuery, TimeCatalog, TimeSource, TimeSpan};
use crate::models::ts_dashboard::PanelDef;
use crate::routes::Route;
use crate::routes::tabs::TimeseriesTab;
use crate::services::backend::Backend;
use crate::state::timeseries::{DashUi, ExploreUi, Notice};

/// Point the draft at a collection, as a fresh query over it. The previous
/// collection's field choices do not carry over: they may not exist.
pub fn choose_collection(
    ui: &mut ExploreUi,
    source: &TimeSource,
    now: i64,
    time_field: Option<&str>,
) {
    let Some(mut q) = source.default_query(now) else {
        return;
    };
    if let Some(t) = time_field
        && source.time_fields.iter().any(|f| f.name == t)
    {
        q.time_field = t.to_string();
    }
    ui.draft = Some(q);
    ui.hidden.clear();
    ui.previous_ranges.clear();
}

/// Start the Explore tab from what the catalog offers: another page's
/// request, else the first collection that can be explored, a timeseries
/// one before the rest.
pub fn init(ui: &mut ExploreUi, catalog: &TimeCatalog) {
    let (sources, now) = (&catalog.sources, catalog.observed_at);
    if let Some((collection, field)) = ui.requested.take() {
        match sources.iter().find(|s| s.collection == collection) {
            Some(s) if s.eligible() => {
                choose_collection(ui, s, now, field.as_deref());
                ui.shown = None;
                ui.error = None;
            }
            Some(s) => {
                let why = s.unsupported.map_or("", |u| u.reason());
                ui.notice = Some(Notice::err(format!(
                    "{collection} cannot be explored. {why}"
                )));
            }
            None => {
                ui.notice = Some(Notice::err(format!(
                    "The database has no collection named {collection}."
                )));
            }
        }
    }
    // A database change can take the drafted collection away. The draft and
    // its result belong to it, so both go.
    let gone = ui.draft.as_ref().is_some_and(|q| {
        !sources
            .iter()
            .any(|s| s.collection == q.collection && s.eligible())
    });
    if gone {
        let name = ui.draft.take().map(|q| q.collection).unwrap_or_default();
        ui.shown = None;
        ui.error = None;
        ui.running = None;
        ui.hidden.clear();
        ui.previous_ranges.clear();
        ui.notice = Some(Notice::err(format!(
            "{name} is not in this database. Pick another collection."
        )));
    }
    if ui.draft.is_some() {
        return;
    }
    let first = sources
        .iter()
        .filter(|s| s.eligible())
        .min_by_key(|s| s.profile != crate::models::collection::Profile::Timeseries);
    if let Some(s) = first {
        choose_collection(ui, s, now, None);
    }
}

/// Run the draft. A reply that arrives after a newer run or a cancel is
/// dropped, so a slow answer never replaces a newer one.
pub async fn run(mut ex: Signal<ExploreUi>, backend: Rc<dyn Backend>) {
    let Some(q) = ex.peek().draft.clone() else {
        return;
    };
    let seq = {
        let mut w = ex.write();
        w.seq += 1;
        w.running = Some(w.seq);
        w.cancelled = false;
        w.seq
    };
    let result = backend.explore(&q).await;
    let mut w = ex.write();
    if w.running != Some(seq) {
        return;
    }
    w.running = None;
    match result {
        Ok(r) => {
            let keys: Vec<String> = r.series.iter().map(|s| s.key.clone()).collect();
            w.hidden.retain(|k| keys.contains(k));
            w.shown = Some(r);
            w.error = None;
        }
        // The last result stays, marked stale.
        Err(e) => w.error = Some(e.to_string()),
    }
}

/// Stop waiting for the running query. The last result stays on screen.
pub fn cancel(mut ex: Signal<ExploreUi>) {
    let mut w = ex.write();
    if w.running.take().is_some() {
        w.cancelled = true;
    }
}

/// Open `query` in the Explore tab as the draft, to be run there. The old
/// result goes: it answered another question.
pub fn drill_into_explore(mut ex: Signal<ExploreUi>, query: &ExploreQuery) {
    let mut w = ex.write();
    w.draft = Some(query.clone());
    w.shown = None;
    w.error = None;
    w.cancelled = false;
    w.running = None;
    w.seq = 0;
    w.hidden.clear();
    w.previous_ranges.clear();
    w.picker_open = false;
}

/// Narrow the range to `span`, keeping the one it replaces so the user can
/// go back. Returns whether the draft changed.
pub fn set_range(ui: &mut ExploreUi, span: TimeSpan) -> bool {
    let Some(q) = ui.draft.as_mut() else {
        return false;
    };
    if q.span == span {
        return false;
    }
    ui.previous_ranges.push(q.span);
    q.span = span;
    true
}

/// Go back to the range before the last change.
pub fn previous_range(ui: &mut ExploreUi) -> bool {
    let Some(span) = ui.previous_ranges.pop() else {
        return false;
    };
    if let Some(q) = ui.draft.as_mut() {
        q.span = span;
    }
    true
}

/// Store the draft's SQL in the shared query library under `name`.
pub async fn save_query(
    mut ex: Signal<ExploreUi>,
    backend: Rc<dyn Backend>,
    database: String,
    name: String,
    q: ExploreQuery,
) {
    let save = SaveQuery {
        id: None,
        name: name.clone(),
        folder: None,
        database,
        sql: q.sql(),
        params: Vec::new(),
        tags: vec!["timeseries".to_string(), q.collection.clone()],
    };
    let result = backend.save_query(&save).await;
    ex.write().notice = Some(match result {
        Ok(saved) => Notice::ok(format!("Saved {} to the query library.", saved.name)),
        Err(e) => Notice::err(e.to_string()),
    });
}

/// Add the draft to a dashboard as an embedded definition: a snapshot that
/// stays as it is if a saved query later changes. `target` is a stored
/// document, or `None` for a new one.
pub fn add_to_dashboard(
    mut dash: Signal<DashUi>,
    nav: Navigator,
    target: Option<String>,
    q: &ExploreQuery,
) {
    let panel = PanelDef {
        id: String::new(),
        title: q.title(),
        query: q.clone(),
        kind: ChartKind::Line,
        height: 300,
    };
    {
        let mut w = dash.write();
        w.pending_add = Some(panel);
        match target {
            Some(id) => {
                if w.open_id.as_deref() != Some(id.as_str()) {
                    w.selected = Some(id);
                    w.dialog = None;
                    w.pending_open = true;
                }
            }
            None => w.pending_new = true,
        }
    }
    nav.push(Route::Timeseries {
        tab: TimeseriesTab::Dashboards,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;

    const NOW: i64 = epoch(2026, 6, 13, 13, 40, 0);

    async fn catalog() -> TimeCatalog {
        MockConnectionService::ready()
            .time_sources()
            .await
            .expect("catalog")
    }

    #[tokio::test]
    async fn a_fresh_explore_tab_opens_the_timeseries_collection() {
        let mut ui = ExploreUi::default();
        init(&mut ui, &catalog().await);
        let q = ui.draft.expect("draft");
        assert_eq!(q.collection, "sales");
        assert_eq!(q.time_field, "sold_at");
        assert_eq!(q.span.end, epoch(2026, 6, 14, 0, 0, 0));
    }

    #[tokio::test]
    async fn a_request_from_another_page_wins_and_carries_the_time_field() {
        let mut ui = ExploreUi {
            requested: Some(("shipments".into(), Some("updated_at".into()))),
            ..ExploreUi::default()
        };
        init(&mut ui, &catalog().await);
        let q = ui.draft.expect("draft");
        assert_eq!(
            (q.collection.as_str(), q.time_field.as_str()),
            ("shipments", "updated_at")
        );
    }

    #[tokio::test]
    async fn a_request_for_a_collection_that_cannot_be_explored_says_why_and_keeps_the_draft() {
        let c = catalog().await;
        let mut ui = ExploreUi::default();
        init(&mut ui, &c);
        ui.requested = Some(("sessions".into(), None));
        init(&mut ui, &c);
        assert_eq!(
            ui.draft.as_ref().map(|q| q.collection.as_str()),
            Some("sales")
        );
        let n = ui.notice.expect("notice");
        assert!(n.error && n.text.contains("key-value"), "{}", n.text);
    }

    #[tokio::test]
    async fn a_request_for_an_unknown_collection_is_an_error_notice() {
        let c = catalog().await;
        let mut ui = ExploreUi {
            requested: Some(("nope".into(), None)),
            ..ExploreUi::default()
        };
        init(&mut ui, &c);
        assert!(ui.notice.is_some_and(|n| n.error));
    }

    #[tokio::test]
    async fn choosing_another_collection_clears_what_belonged_to_the_old_one() {
        let c = catalog().await;
        let mut ui = ExploreUi::default();
        init(&mut ui, &c);
        ui.hidden.insert("store_104".into());
        ui.previous_ranges.push(TimeSpan::new(0, 1));
        let shipments = c.source("shipments").expect("source");
        choose_collection(&mut ui, shipments, NOW, None);
        assert_eq!(
            ui.draft.as_ref().map(|q| q.collection.as_str()),
            Some("shipments")
        );
        assert!(ui.hidden.is_empty() && ui.previous_ranges.is_empty());
        assert!(ui.draft.as_ref().is_some_and(|q| q.filters.is_empty()));
    }

    #[tokio::test]
    async fn a_database_without_the_drafted_collection_clears_the_draft_and_result() {
        let c = catalog().await;
        let mut ui = ExploreUi::default();
        init(&mut ui, &c);
        let mut other = c.clone();
        other.sources.retain(|s| s.collection != "sales");
        init(&mut ui, &other);
        // The draft moves to a collection the database does hold.
        assert_ne!(
            ui.draft.as_ref().map(|q| q.collection.as_str()),
            Some("sales")
        );
        assert!(ui.shown.is_none());
        assert!(
            ui.notice
                .is_some_and(|n| n.text.contains("sales is not in this database"))
        );
    }

    #[tokio::test]
    async fn a_range_change_can_be_undone_in_order() {
        let mut ui = ExploreUi::default();
        init(&mut ui, &catalog().await);
        let original = ui.draft.as_ref().map(|q| q.span).expect("span");
        let a = TimeSpan::new(original.start, original.start + 86_400 * 3);
        let b = TimeSpan::new(original.start, original.start + 86_400);
        assert!(set_range(&mut ui, a));
        assert!(set_range(&mut ui, b));
        assert!(!set_range(&mut ui, b), "the same range is not a change");
        assert!(previous_range(&mut ui));
        assert_eq!(ui.draft.as_ref().map(|q| q.span), Some(a));
        assert!(previous_range(&mut ui));
        assert_eq!(ui.draft.as_ref().map(|q| q.span), Some(original));
        assert!(!previous_range(&mut ui));
    }
}
