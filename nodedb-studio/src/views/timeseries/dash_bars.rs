//! The bars above a dashboard's panels: the title row that persists the
//! document, and the range, refresh, and filters every panel shares.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::date_minute;
use crate::models::records::Filter;
use crate::models::timeseries::{TimeCatalog, TimeSpan};
use crate::models::ts_dashboard::{Dashboard, Refresh};
use crate::services::backend::Backend;
use crate::state::timeseries::{DashUi, SaveState, TimeseriesUi};
use crate::views::timeseries::dash_ops::new_panel;
use crate::views::timeseries::dash_tasks::save;
use crate::views::timeseries::toolbar::{FilterAdder, RangeButton};

/// What the title row says about the document's persistence. Loading data
/// is separate: a panel can still be loading in any of these.
pub fn state_text(ui: &DashUi) -> (&'static str, String) {
    let Some(doc) = &ui.open else {
        return ("idle", String::new());
    };
    match &ui.save {
        SaveState::Saving => ("info", "Saving…".to_string()),
        SaveState::Failed(_) => ("err", "Save failed. Retry or Save as.".to_string()),
        SaveState::Idle => {
            let saved = doc.saved_at.map(|t| {
                format!(
                    "saved {} UTC",
                    date_minute(t, 0).get(5..).unwrap_or_default()
                )
            });
            match (doc.id.is_some(), ui.modified()) {
                (false, _) => ("warn", "Not saved yet".to_string()),
                (true, true) => ("warn", format!("Modified · {}", saved.unwrap_or_default())),
                (true, false) => ("ok", format!("Saved · {}", saved.unwrap_or_default())),
            }
        }
    }
}

/// The document's title, its state, and the actions that persist it.
#[component]
pub(super) fn DocBar(
    doc: Dashboard,
    ui: DashUi,
    range: TimeSpan,
    catalog: Option<TimeCatalog>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    let (tone, text) = state_text(&ui);
    let saving = ui.save == SaveState::Saving;
    let failed = matches!(ui.save, SaveState::Failed(_));
    let can_save = doc.id.is_none() || ui.modified() || failed;

    rsx! {
        div { class: "ts-docbar",
            h2 { class: "ts-doc-title", "{doc.name}" }
            span { class: "status ts-doc-state", role: "status",
                span { class: "status-dot {tone}" }
                "{text}"
            }
            div { class: "toolbar-spacer" }
            if failed {
                button {
                    class: "btn",
                    title: "Try the save again",
                    onclick: {
                        let backend = backend.clone();
                        move |_| { spawn_forever(save(dash, backend.clone(), None, range)); }
                    },
                    Ico { icon: Icon::RefreshCw, size: 16 }
                    "Retry"
                }
            }
            if ui.modified() {
                button {
                    class: "icon-btn",
                    title: "Revert: discard the unsaved changes",
                    "aria-label": "Revert unsaved changes",
                    onclick: move |_| crate::views::timeseries::dash_ops::revert(&mut dash.write()),
                    Ico { icon: Icon::RotateCcw, size: 16 }
                }
            }
            button {
                class: "btn",
                title: "Save a copy under another name",
                onclick: move |_| {
                    dash.write().dialog = Some(crate::state::timeseries::DashDialog::SaveAs {
                        name: String::new(),
                        error: None,
                        then: None,
                    });
                },
                "Save as…"
            }
            button {
                class: "btn",
                title: "Add a panel: pick a collection, shape the query, preview, add",
                onclick: {
                    let catalog = catalog.clone();
                    move |_| {
                        if let Some(q) = catalog
                            .as_ref()
                            .and_then(|c| c.sources.iter().find(|s| s.eligible()).and_then(|s| s.default_query(c.observed_at)))
                        {
                            new_panel(&mut dash.write(), q);
                        }
                    }
                },
                disabled: catalog.is_none(),
                Ico { icon: Icon::Plus, size: 16 }
                "Panel"
            }
            button {
                class: "btn primary",
                disabled: !can_save || saving,
                title: if can_save { "Save the queries, layout, and settings" } else { "Nothing has changed since the last save" },
                onclick: {
                    let backend = backend.clone();
                    move |_| { spawn_forever(save(dash, backend.clone(), None, range)); }
                },
                Ico { icon: Icon::Save, size: 16 }
                "Save dashboard"
            }
        }
    }
}

/// The range, refresh, and filters every panel shares.
#[component]
pub(super) fn FilterBar(doc: Dashboard, now: i64, catalog: Option<TimeCatalog>) -> Element {
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    // A filter reaches every panel, so it must name a field every panel's
    // collection has.
    let choices: Vec<String> = match &catalog {
        Some(c) => {
            let mut common: Option<Vec<String>> = None;
            for name in doc.collections() {
                let fields: Vec<String> = c
                    .source(&name)
                    .map(|s| {
                        s.fields
                            .iter()
                            .filter(|f| f.type_label.starts_with("string"))
                            .map(|f| f.name.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                common = Some(match common {
                    None => fields,
                    Some(prev) => prev.into_iter().filter(|f| fields.contains(f)).collect(),
                });
            }
            common.unwrap_or_default()
        }
        None => Vec::new(),
    };

    rsx! {
        div { class: "ts-toolbar ts-filterbar",
            RangeButton {
                span: doc.range,
                now,
                invalid: false,
                on_apply: move |span: TimeSpan| {
                    if let Some(d) = dash.write().open.as_mut() {
                        d.range = span;
                    }
                },
            }
            span { class: "ts-utc", title: "Buckets align to UTC. Bounds are half-open: start included, end excluded.", "UTC" }
            div { class: "ts-ctl",
                label { r#for: "ts-dash-refresh", "Refresh" }
                select {
                    id: "ts-dash-refresh",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        let next = Refresh::ALL.into_iter().find(|r| r.label() == v).unwrap_or(Refresh::Off);
                        if let Some(d) = dash.write().open.as_mut() {
                            d.refresh = next;
                        }
                    },
                    for r in Refresh::ALL {
                        option { key: "{r.label()}", value: "{r.label()}", selected: r == doc.refresh, "{r.label()}" }
                    }
                }
            }
            for f in doc.filters.iter() {
                {
                    let field = f.field.clone();
                    let label = f.label();
                    rsx! {
                        span { key: "{field}", class: "chip", title: "Applies to every panel",
                            Ico { icon: Icon::Filter, size: 16 }
                            "{label}"
                            button {
                                class: "chip-remove",
                                "aria-label": "Remove filter {label}",
                                onclick: move |_| {
                                    if let Some(d) = dash.write().open.as_mut() {
                                        d.filters.retain(|x| x.field != field);
                                    }
                                },
                                Ico { icon: Icon::X, size: 16 }
                            }
                        }
                    }
                }
            }
            FilterAdder {
                choices,
                on_add: move |f: Filter| {
                    if let Some(d) = dash.write().open.as_mut() {
                        d.filters.retain(|x| x.field != f.field);
                        d.filters.push(f);
                    }
                },
            }
        }
    }
}
