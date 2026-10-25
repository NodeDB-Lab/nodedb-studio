//! The Saved dashboards panel: the named documents of this workspace, a
//! preview of the selected one, and the actions that open or manage it.
//! Selecting a row only previews it. Opening is its own step.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::date_minute;
use crate::models::ts_dashboard::{DashboardSummary, WORKSPACE_LOCATION};
use crate::state::timeseries::{DashDialog, DashUi};
use crate::views::timeseries::dash_ops::request_open;

/// Rows whose name or collection matches `needle`, case-insensitively.
pub fn matching<'a>(rows: &'a [DashboardSummary], needle: &str) -> Vec<&'a DashboardSummary> {
    let n = needle.trim().to_lowercase();
    rows.iter()
        .filter(|d| {
            n.is_empty()
                || d.name.to_lowercase().contains(&n)
                || d.collections.iter().any(|c| c.to_lowercase().contains(&n))
        })
        .collect()
}

#[component]
pub fn LibraryBody(rows: Vec<DashboardSummary>, ui: DashUi) -> Element {
    let tsui = use_context::<crate::state::timeseries::TimeseriesUi>();
    let mut dash = tsui.dash;
    let found = matching(&rows, &ui.filter);
    let selected = ui
        .selected
        .as_ref()
        .and_then(|id| rows.iter().find(|d| d.id == *id))
        .cloned();
    let manage_open = ui.menu.as_deref() == Some("manage");

    rsx! {
        div { class: "ts-library",
            div { class: "search-field ts-lib-search",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    r#type: "search",
                    placeholder: "Search dashboards…",
                    "aria-label": "Search dashboards",
                    value: "{ui.filter}",
                    oninput: move |e| dash.write().filter = e.value(),
                }
            }
            div { class: "list ts-lib-list", role: "listbox", "aria-label": "Saved dashboards",
                for d in found.iter() {
                    {
                        let id = d.id.clone();
                        let collections = d.collections.join(", ");
                        let is_sel = ui.selected.as_deref() == Some(d.id.as_str());
                        let open = ui.open_id.as_deref() == Some(d.id.as_str());
                        rsx! {
                            button {
                                key: "{id}",
                                class: if is_sel { "list-item ts-lib-item selected" } else { "list-item ts-lib-item" },
                                role: "option",
                                "aria-selected": "{is_sel}",
                                onclick: {
                                    let id = id.clone();
                                    move |_| dash.write().selected = Some(id.clone())
                                },
                                ondoubleclick: {
                                    let id = id.clone();
                                    move |_| request_open(&mut dash.write(), &id)
                                },
                                Ico { icon: Icon::ChartColumn, size: 20 }
                                span { class: "ts-pick-text",
                                    span { class: "ts-pick-name", "{d.name}" }
                                    span { class: "ts-pick-sub", "{collections}" }
                                }
                                if open {
                                    span { class: "badge outline", "Open" }
                                }
                            }
                        }
                    }
                }
                if found.is_empty() {
                    div { class: "ts-pick-none",
                        if rows.is_empty() { "No saved dashboard yet. Start one with New." } else { "No dashboard matches." }
                    }
                }
            }
            if let Some(d) = selected.clone() {
                dl { class: "ts-lib-meta",
                    dt { "Panels" }
                    dd { "{d.panels}" }
                    dt { "Saved" }
                    dd { "{date_minute(d.saved_at, 0)} UTC · revision {d.revision}" }
                    dt { "Location" }
                    dd { "{WORKSPACE_LOCATION}" }
                }
            }
            div { class: "ts-lib-actions",
                button {
                    class: "btn",
                    disabled: selected.is_none(),
                    title: "Open the selected dashboard and run fresh queries",
                    onclick: {
                        let id = selected.as_ref().map(|d| d.id.clone());
                        move |_| {
                            if let Some(id) = &id {
                                request_open(&mut dash.write(), id);
                            }
                        }
                    },
                    Ico { icon: Icon::FolderOpen, size: 16 }
                    "Open dashboard"
                }
                div { class: "anchor",
                    button {
                        class: "btn",
                        "aria-haspopup": "menu",
                        "aria-expanded": "{manage_open}",
                        disabled: selected.is_none(),
                        onclick: move |_| {
                            let mut w = dash.write();
                            w.menu = if w.menu.as_deref() == Some("manage") { None } else { Some("manage".to_string()) };
                        },
                        "Manage…"
                    }
                    if manage_open {
                        div { class: "ts-scrim", onclick: move |_| dash.write().menu = None }
                        div { class: "popover align-left ts-manage-pop", role: "menu", "aria-label": "Manage dashboard",
                            button {
                                class: "menu-item",
                                role: "menuitem",
                                onclick: {
                                    let name = selected.as_ref().map(|d| d.name.clone()).unwrap_or_default();
                                    move |_| {
                                        let mut w = dash.write();
                                        w.menu = None;
                                        w.dialog = Some(DashDialog::Rename { name: name.clone(), error: None });
                                    }
                                },
                                Ico { icon: Icon::Pencil, size: 16 }
                                span { "Rename…" }
                            }
                            button {
                                class: "menu-item",
                                role: "menuitem",
                                onclick: {
                                    let id = selected.as_ref().map(|d| d.id.clone()).unwrap_or_default();
                                    move |_| {
                                        let mut w = dash.write();
                                        w.menu = None;
                                        w.pending_duplicate = Some(id.clone());
                                    }
                                },
                                Ico { icon: Icon::Copy, size: 16 }
                                span { "Duplicate" }
                            }
                            button {
                                class: "menu-item",
                                role: "menuitem",
                                disabled: true,
                                title: "Export needs a file destination, which no design names yet.",
                                Ico { icon: Icon::Download, size: 16 }
                                span { "Export…" }
                            }
                            button {
                                class: "menu-item",
                                role: "menuitem",
                                disabled: true,
                                title: "Import needs a file source, which no design names yet.",
                                Ico { icon: Icon::Upload, size: 16 }
                                span { "Import…" }
                            }
                            div { class: "menu-sep" }
                            button {
                                class: "menu-item danger",
                                role: "menuitem",
                                onclick: move |_| {
                                    let mut w = dash.write();
                                    w.menu = None;
                                    w.dialog = Some(DashDialog::Delete);
                                },
                                Ico { icon: Icon::Trash, size: 16 }
                                span { "Delete…" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;

    #[tokio::test]
    async fn search_matches_a_name_or_a_collection() {
        let rows = MockConnectionService::ready()
            .dashboards()
            .await
            .expect("rows");
        assert_eq!(matching(&rows, "").len(), 3);
        assert_eq!(matching(&rows, "ORDER").len(), 1);
        let by_collection: Vec<&str> = matching(&rows, "shipments")
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(by_collection, ["Delivery trends"]);
        assert!(matching(&rows, "zzz").is_empty());
    }
}
