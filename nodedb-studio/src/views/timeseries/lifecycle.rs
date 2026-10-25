//! Lifecycle: retention, rollups, late data, and backfill of one timeseries
//! collection. Edits only queue changes. The review is the one place a
//! change is read in full and applied.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::collection::Profile;
use crate::models::timeseries::{Aggregate, Bucket, Measure, TimeCatalog, TimeSource, parse_utc};
use crate::models::ts_lifecycle::{Lifecycle, RetentionDraft, RollupDraft, RollupFn};
use crate::routes::Route;
use crate::routes::tabs::TimeseriesTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::designer::{DesignerState, Target};
use crate::state::explorer::Selected;
use crate::state::timeseries::{BackfillUi, LifeSheet, PreviewState, TimeseriesUi};
use crate::state::workspace::Surface;
use crate::views::timeseries::explore_tasks::drill_into_explore;
use crate::views::timeseries::late_data::LateBody;
use crate::views::timeseries::life_sheets::LifeSheets;
use crate::views::timeseries::life_tasks::{open_review, refresh_now};
use crate::views::timeseries::notice::TsNotice;
use crate::views::timeseries::retention::{RetentionBody, selected_tier};
use crate::views::timeseries::rollup_side::RollupBody;

const SURFACE: Surface = Surface::Timeseries(TimeseriesTab::Lifecycle);
const LATE_SIZE: PanelSize = PanelSize::height(310.0, 250.0, 540.0);
const ROLLUP_SIZE: PanelSize = PanelSize::width(410.0, 340.0, 640.0);

#[component]
pub fn LifecycleTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let ts = use_context::<TimeseriesUi>();
    let mut life = ts.life;
    let mut designer = use_context::<Signal<DesignerState>>();
    let mut explorer_selection = use_context::<Signal<Option<Selected>>>();
    let nav = use_navigator();

    let catalog_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _scope = (
                conn(),
                active.read().as_ref().map(|c| c.current_database.clone()),
            );
            async move { backend.time_sources().await }
        })
    };
    let catalog: Option<TimeCatalog> = catalog_read
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    // Lifecycle belongs to native timeseries collections.
    let timeseries: Vec<String> = catalog
        .as_ref()
        .map(|c| {
            c.sources
                .iter()
                .filter(|s| s.profile == Profile::Timeseries && s.eligible())
                .map(|s| s.collection.clone())
                .collect()
        })
        .unwrap_or_default();
    let chosen = life.read().collection.clone();
    let collection = chosen
        .filter(|c| timeseries.contains(c))
        .or_else(|| timeseries.first().cloned());

    let mut lifecycle_read = {
        let backend = backend.clone();
        use_resource(use_reactive!(|collection| {
            let backend = backend.clone();
            let _scope = (
                conn(),
                active.read().as_ref().map(|c| c.current_database.clone()),
            );
            let _epoch = life.read().epoch;
            async move {
                match collection {
                    Some(c) => Some(backend.lifecycle(&c).await),
                    None => None,
                }
            }
        }))
    };
    let state = {
        let raw = lifecycle_read.read().clone();
        match raw {
            Some(Some(r)) => crate::services::async_state::AsyncState::from_value(Some(r)),
            _ => crate::services::async_state::AsyncState::Loading,
        }
    };
    let l: Option<Lifecycle> = state.loaded().cloned();
    let selected = life.read().selected.clone();
    let tier_name = l
        .as_ref()
        .map(|l| selected_tier(&l.tiers(), selected.as_deref()).name.clone());

    // The selected rollup's queued refreshes.
    let runs_read = {
        let backend = backend.clone();
        use_resource(use_reactive!(|collection, tier_name| {
            let backend = backend.clone();
            let _epoch = life.read().epoch;
            async move {
                match (collection, tier_name) {
                    (Some(c), Some(t)) => backend.rollup_runs(&c, &t).await.unwrap_or_default(),
                    _ => Vec::new(),
                }
            }
        }))
    };
    let runs = runs_read.read().clone().unwrap_or_default();
    let history_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _epoch = life.read().epoch;
            async move { backend.backfill_runs(None).await.unwrap_or_default() }
        })
    };
    let latest = history_read
        .read()
        .clone()
        .and_then(|r| r.into_iter().next());

    let ui = life.read().clone();

    rsx! {
        div { class: "page-body ts-page",
            TsNotice { notice: ui.notice.clone(), on_dismiss: move |_| life.write().notice = None }
            if timeseries.is_empty() && catalog.is_some() {
                div { class: "empty-state",
                    Ico { icon: Icon::ChartColumn, size: 24 }
                    h2 { "No timeseries collection" }
                    p { "Lifecycle manages retention and rollups of a native timeseries collection. This database has none. A collection without a TIME_KEY has no lifecycle: explore it in Explore, or create a timeseries collection in the Designer." }
                    div { class: "actions",
                        button { class: "btn", onclick: move |_| { nav.push(Route::Designer {}); },
                            Ico { icon: Icon::Pencil, size: 16 }
                            "Open Designer"
                        }
                    }
                }
            }
            if let (Some(_), Some(coll)) = (&catalog, &collection) {
                div { class: "ts-toolbar ts-lifebar",
                    div { class: "ts-ctl",
                        label { r#for: "ts-life-coll", "Collection" }
                        select {
                            id: "ts-life-coll",
                            class: "select native",
                            onchange: move |e| {
                                let mut w = life.write();
                                w.collection = Some(e.value());
                                w.selected = None;
                                w.pending.clear();
                                w.sheet = None;
                            },
                            for name in timeseries.iter() {
                                option { key: "{name}", value: "{name}", selected: name == coll, "{name}" }
                            }
                        }
                    }
                    if let Some(l) = &l {
                        span { class: "ts-meta", "{l.time_field} · TIME_KEY" }
                        span { class: "ts-meta-sep" }
                        span { class: "ts-meta", "Partition {l.partition}" }
                    }
                    div { class: "toolbar-spacer" }
                    button {
                        class: "btn",
                        title: "Open {coll} in the Designer, where its schema and storage are defined",
                        onclick: {
                            let coll = coll.clone();
                            move |_| {
                                designer.write().select(Target::Collection(coll.clone()));
                                nav.push(Route::Designer {});
                            }
                        },
                        Ico { icon: Icon::ExternalLink, size: 16 }
                        "Open in Designer"
                    }
                    button {
                        class: "btn primary",
                        title: "Read every pending change in full, then apply",
                        onclick: {
                            let (backend, coll) = (backend.clone(), coll.clone());
                            move |_| { spawn_forever(open_review(life, backend.clone(), coll.clone())); }
                        },
                        "Review changes"
                        if !ui.pending.is_empty() {
                            span { class: "ts-pending", "{ui.pending.len()}" }
                        }
                    }
                }
                AsyncView {
                    loading: state.is_loading(),
                    empty: false,
                    error: state.error_message(),
                    retriable: state.is_retriable(),
                    on_retry: move |_| lifecycle_read.restart(),
                }
                if let Some(l) = l.clone() {
                    div { class: "workbench events-wb ts-life",
                        div { class: "workbench col ts-life-left",
                            Panel {
                                surface: SURFACE,
                                id: "retention",
                                title: "Retention & rollups",
                                class: "fill",
                                actions: rsx! {
                                    button {
                                        class: "btn",
                                        title: "Change how long raw rows and rollups are kept. Queued for review.",
                                        onclick: {
                                            let l = l.clone();
                                            move |_| life.write().sheet = Some(LifeSheet::Policy(RetentionDraft::from(&l)))
                                        },
                                        "Edit policy"
                                    }
                                    button {
                                        class: "btn",
                                        title: "Define a rollup. Queued for review: creating one previews its dependencies.",
                                        onclick: {
                                            let l = l.clone();
                                            move |_| life.write().sheet = Some(LifeSheet::Rollup(RollupDraft::blank(&l)))
                                        },
                                        Ico { icon: Icon::Plus, size: 16 }
                                        "Rollup"
                                    }
                                },
                                RetentionBody {
                                    l: l.clone(),
                                    selected: ui.selected.clone(),
                                    on_select: move |name: String| life.write().selected = Some(name),
                                }
                            }
                            ResizeHandle { surface: SURFACE, target: "late", size: LATE_SIZE, side: Side::After }
                            Panel {
                                surface: SURFACE,
                                id: "late",
                                title: "Late data & backfill",
                                size: LATE_SIZE,
                                actions: rsx! {
                                    button {
                                        class: "link-btn",
                                        title: "Open the import and write entry point of {coll}",
                                        onclick: {
                                            let coll = coll.clone();
                                            move |_| {
                                                explorer_selection.set(Some(Selected { name: coll.clone() }));
                                                nav.push(Route::DataTransfer {});
                                            }
                                        },
                                        "Manage ingestion"
                                        Ico { icon: Icon::ExternalLink, size: 16 }
                                    }
                                    button {
                                        class: "btn",
                                        title: "Change how late writes correct closed buckets. Queued for review.",
                                        onclick: {
                                            let l = l.clone();
                                            move |_| life.write().sheet = Some(LifeSheet::Late(l.late))
                                        },
                                        "Edit late-data policy"
                                    }
                                },
                                LateBody {
                                    l: l.clone(),
                                    quick: ui.quick.clone(),
                                    latest: latest.clone(),
                                    on_quick: move |q| life.write().quick = q,
                                    on_preview: {
                                        let l = l.clone();
                                        move |_| start_backfill_review(ts, &l, nav)
                                    },
                                    on_runs: {
                                        let l = l.clone();
                                        move |_| open_runs(ts, &l, nav)
                                    },
                                }
                            }
                        }
                        ResizeHandle { surface: SURFACE, target: "rollup", size: ROLLUP_SIZE, side: Side::After }
                        Panel {
                            surface: SURFACE,
                            id: "rollup",
                            title: "Rollup",
                            size: ROLLUP_SIZE,
                            RollupBody {
                                l: l.clone(),
                                tier: selected_tier(&l.tiers(), ui.selected.as_deref()).clone(),
                                tab: ui.tab,
                                runs,
                                on_tab: move |t| life.write().tab = t,
                                on_refresh: {
                                    let (backend, coll, name) = (backend.clone(), coll.clone(), tier_name.clone());
                                    move |_| {
                                        if let Some(n) = &name {
                                            spawn_forever(refresh_now(life, backend.clone(), coll.clone(), n.clone()));
                                        }
                                    }
                                },
                                on_explore: {
                                    let l = l.clone();
                                    let name = tier_name.clone();
                                    let source = catalog.as_ref().and_then(|c| c.source(coll).cloned());
                                    let now = catalog.as_ref().map_or(0, |c| c.observed_at);
                                    move |_| explore_rollup(ts, &l, source.as_ref(), name.as_deref(), now, nav)
                                },
                            }
                        }
                        if let Some(sheet) = ui.sheet.clone() {
                            LifeSheets { sheet, l: l.clone(), collection: coll.clone(), pending: ui.pending.len() }
                        }
                    }
                }
            }
        }
    }
}

/// Hand the overview's range and target to the Backfill page and run the
/// dry run there.
fn start_backfill_review(ts: TimeseriesUi, l: &Lifecycle, nav: dioxus::router::Navigator) {
    let quick = ts.life.peek().quick.clone();
    let (Some(start), Some(end), Some(r)) = (
        parse_utc(&quick.start),
        parse_utc(&quick.end),
        l.rollup(&quick.target),
    ) else {
        return;
    };
    let mut ui = BackfillUi::standard();
    ui.spec.target = r.name.clone();
    ui.spec.bucket = r.bucket;
    ui.spec.range = crate::models::timeseries::TimeSpan::new(start, end);
    ui.spec.aggregates = r.outputs.clone();
    ui.spec.target_key = format!(
        "{} + {}",
        bucket_word(r.bucket),
        r.group_by.first().map_or("series", String::as_str)
    );
    ui.start = quick.start;
    ui.end = quick.end;
    ui.auto_preview = true;
    ui.preview = PreviewState::Draft;
    let mut backfill = ts.backfill;
    backfill.set(ui);
    nav.push(Route::Timeseries {
        tab: TimeseriesTab::Backfill,
    });
}

fn bucket_word(b: Bucket) -> &'static str {
    match b {
        Bucket::Day1 => "day",
        Bucket::Hour6 | Bucket::Hour1 => "hour",
        Bucket::Minute15 | Bucket::Minute5 | Bucket::Minute1 => "minute",
    }
}

/// Show the job history without starting a dry run.
fn open_runs(ts: TimeseriesUi, l: &Lifecycle, nav: dioxus::router::Navigator) {
    let quick = ts.life.peek().quick.clone();
    let mut ui = BackfillUi::standard();
    if let Some(r) = l.rollup(&quick.target) {
        ui.spec.target = r.name.clone();
        ui.spec.bucket = r.bucket;
        ui.spec.aggregates = r.outputs.clone();
    }
    ui.auto_preview = false;
    let mut backfill = ts.backfill;
    backfill.set(ui);
    nav.push(Route::Timeseries {
        tab: TimeseriesTab::Backfill,
    });
}

/// Explore the question a rollup answers, over the raw rows it is cut from:
/// its bucket, its first dimension, and its first summed column, in the
/// currency the collection declares for it.
fn explore_rollup(
    ts: TimeseriesUi,
    l: &Lifecycle,
    source: Option<&TimeSource>,
    tier: Option<&str>,
    now: i64,
    nav: dioxus::router::Navigator,
) {
    let Some(mut query) = source.and_then(|s| s.default_query(now)) else {
        return;
    };
    if let Some(r) = tier.and_then(|t| l.rollup(t)) {
        query.bucket = r.bucket;
        let currency = source.and_then(|s| s.money.first().map(|m| m.currency_field.clone()));
        query.group_by = r
            .group_by
            .iter()
            .find(|g| Some(g.as_str()) != currency.as_deref())
            .cloned();
        let summed = r
            .outputs
            .iter()
            .find(|o| o.func == RollupFn::Sum)
            .and_then(|o| o.column.clone());
        query.measure = match summed {
            Some(col) => Measure::of(Aggregate::Sum, &col),
            None => Measure::count_rows(),
        };
    }
    drill_into_explore(ts.explore, &query);
    nav.push(Route::Timeseries {
        tab: TimeseriesTab::Explore,
    });
}
