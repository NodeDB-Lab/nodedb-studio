//! The dialogs over the Dashboards tab, each a narrow sheet: save under a
//! name, rename, delete, leave with unsaved changes, inspect a panel's
//! query, and resolve a save conflict. A sheet holds work in progress, so it
//! closes only when the person says so or the action finishes.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::sheet::{Sheet, SheetFooter};
use crate::models::format::date_minute;
use crate::models::timeseries::TimeSpan;
use crate::models::ts_dashboard::{Dashboard, DashboardSummary, WORKSPACE_LOCATION};
use crate::services::backend::Backend;
use crate::state::timeseries::{After, DashDialog, DashUi, SaveState, TimeseriesUi};
use crate::views::events::common::{FormField, SqlBlock};
use crate::views::timeseries::dash_ops::{proceed, revert};
use crate::views::timeseries::dash_tasks::{delete, reload, rename, save, save_as};

#[component]
pub fn DashDialogs(
    dialog: DashDialog,
    ui: DashUi,
    summaries: Vec<DashboardSummary>,
    range: TimeSpan,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    let close = use_callback(move |()| {
        dash.write().dialog = None;
    });
    let selected = ui
        .selected
        .as_ref()
        .and_then(|id| summaries.iter().find(|d| d.id == *id))
        .cloned();

    match dialog {
        DashDialog::SaveAs { name, error, then } => {
            let issue = Dashboard::name_issue(&name);
            let busy = ui.save == SaveState::Saving;
            rsx! {
                Sheet {
                    title: "Save dashboard as".to_string(),
                    on_close: close,
                    footer: rsx! {
                        SheetFooter {
                            error: error.clone(),
                            hint: Some("Saving never overwrites another dashboard.".to_string()),
                            button { class: "btn", onclick: move |_| close.call(()), "Cancel" }
                            button {
                                class: "btn primary",
                                disabled: issue.is_some() || busy,
                                onclick: {
                                    let (backend, name, then) = (backend.clone(), name.clone(), then.clone());
                                    move |_| {
                                        spawn_forever(save_as(dash, backend.clone(), name.clone(), then.clone(), range));
                                    }
                                },
                                "Save"
                            }
                        }
                    },
                    div { class: "sheet-form",
                        FormField {
                            label: "Name".to_string(),
                            id: "dd-name".to_string(),
                            error: issue.filter(|_| !name.is_empty()).map(str::to_string),
                            input {
                                id: "dd-name",
                                class: "input",
                                value: "{name}",
                                autofocus: true,
                                oninput: {
                                    let (error, then) = (error.clone(), then.clone());
                                    move |e| {
                                        let _ = &error;
                                        dash.write().dialog = Some(DashDialog::SaveAs { name: e.value(), error: None, then: then.clone() });
                                    }
                                },
                            }
                        }
                        dl { class: "ts-lib-meta",
                            dt { "Location" }
                            dd { "{WORKSPACE_LOCATION}" }
                        }
                    }
                }
            }
        }
        DashDialog::Rename { name, error } => {
            let issue = Dashboard::name_issue(&name);
            let target = selected.clone();
            rsx! {
                Sheet {
                    title: "Rename dashboard".to_string(),
                    on_close: close,
                    footer: rsx! {
                        SheetFooter {
                            error: error.clone(),
                            hint: target.as_ref().map(|t| format!("Renames {} in the library.", t.name)),
                            button { class: "btn", onclick: move |_| close.call(()), "Cancel" }
                            button {
                                class: "btn primary",
                                disabled: issue.is_some() || target.is_none(),
                                onclick: {
                                    let (backend, name, target) = (backend.clone(), name.clone(), target.clone());
                                    move |_| {
                                        if let Some(t) = &target {
                                            spawn_forever(rename(dash, backend.clone(), t.id.clone(), t.revision, name.clone()));
                                        }
                                    }
                                },
                                "Rename"
                            }
                        }
                    },
                    div { class: "sheet-form",
                        FormField { label: "Name".to_string(), id: "dd-rename".to_string(),
                            error: issue.filter(|_| !name.is_empty()).map(str::to_string),
                            input {
                                id: "dd-rename",
                                class: "input",
                                value: "{name}",
                                autofocus: true,
                                oninput: move |e| dash.write().dialog = Some(DashDialog::Rename { name: e.value(), error: None }),
                            }
                        }
                    }
                }
            }
        }
        DashDialog::Delete => {
            let target = selected.clone();
            let title = target.as_ref().map_or_else(
                || "Delete dashboard".to_string(),
                |t| format!("Delete {}", t.name),
            );
            rsx! {
                Sheet {
                    title,
                    on_close: close,
                    footer: rsx! {
                        SheetFooter {
                            error: None,
                            hint: None,
                            button { class: "btn", onclick: move |_| close.call(()), "Cancel" }
                            button {
                                class: "btn danger",
                                disabled: target.is_none(),
                                onclick: {
                                    let (backend, target) = (backend.clone(), target.clone());
                                    move |_| {
                                        if let Some(t) = &target {
                                            spawn_forever(delete(dash, backend.clone(), t.id.clone(), t.name.clone()));
                                        }
                                    }
                                },
                                "Delete dashboard"
                            }
                        }
                    },
                    div { class: "sheet-form",
                        if let Some(t) = target {
                            p { "Delete {t.name}? This removes the document and its {t.panels} panel definitions from the workspace." }
                            p { class: "hint", "The collections its panels read are not touched. Deleting cannot be undone." }
                        } else {
                            p { "No dashboard is selected." }
                        }
                    }
                }
            }
        }
        DashDialog::Unsaved(after) => {
            let name = ui
                .open
                .as_ref()
                .map_or_else(String::new, |d| d.name.clone());
            rsx! {
                Sheet {
                    title: "Unsaved changes".to_string(),
                    on_close: close,
                    footer: rsx! {
                        SheetFooter {
                            error: None,
                            hint: None,
                            button { class: "btn", onclick: move |_| close.call(()), "Cancel" }
                            button {
                                class: "btn",
                                onclick: {
                                    let after = after.clone();
                                    move |_| {
                                        let mut w = dash.write();
                                        revert(&mut w);
                                        w.dialog = None;
                                        proceed(&mut w, &after, range);
                                    }
                                },
                                "Discard"
                            }
                            button {
                                class: "btn primary",
                                onclick: {
                                    let (backend, after) = (backend.clone(), after.clone());
                                    move |_| {
                                        spawn_forever(save(dash, backend.clone(), Some(after.clone()), range));
                                    }
                                },
                                "Save dashboard"
                            }
                        }
                    },
                    div { class: "sheet-form",
                        p { "{name} has changes that are not saved. " { match after { After::Open(_) => "Opening another dashboard", After::New => "Starting a new dashboard" } } " would lose them." }
                    }
                }
            }
        }
        DashDialog::Inspect(panel_id) => {
            let found = ui
                .open
                .as_ref()
                .and_then(|d| d.panel(&panel_id).map(|p| (d.clone(), p.clone())));
            rsx! {
                Sheet {
                    title: "Panel query".to_string(),
                    on_close: close,
                    footer: rsx! {
                        SheetFooter { error: None, hint: None,
                            button { class: "btn", onclick: move |_| close.call(()), "Close" }
                        }
                    },
                    div { class: "sheet-form",
                        if let Some((doc, panel)) = found {
                            {
                                let q = doc.run_query(&panel);
                                rsx! {
                                    dl { class: "ts-lib-meta",
                                        dt { "Panel" }
                                        dd { "{panel.title}" }
                                        dt { "Collection" }
                                        dd { "{q.collection}" }
                                        dt { "Range" }
                                        dd { "{date_minute(q.span.start, 0)} to {date_minute(q.span.end, 0)} UTC, end excluded" }
                                    }
                                    SqlBlock { sql: q.sql(), class: "ts-sql" }
                                }
                            }
                        } else {
                            p { "The panel is no longer in the dashboard." }
                        }
                    }
                }
            }
        }
        DashDialog::Conflict => {
            let message = match &ui.save {
                SaveState::Failed(m) => m.clone(),
                SaveState::Idle | SaveState::Saving => String::new(),
            };
            let id = ui.open_id.clone();
            let name = ui
                .open
                .as_ref()
                .map_or_else(String::new, |d| d.name.clone());
            rsx! {
                Sheet {
                    title: "Another session saved first".to_string(),
                    on_close: close,
                    footer: rsx! {
                        SheetFooter { error: None, hint: None,
                            button { class: "btn", onclick: move |_| close.call(()), "Keep editing" }
                            button {
                                class: "btn",
                                onclick: move |_| {
                                    dash.write().dialog = Some(DashDialog::SaveAs { name: String::new(), error: None, then: None });
                                },
                                "Save as a new dashboard"
                            }
                            button {
                                class: "btn danger",
                                title: "Discard your changes and load what the other session saved",
                                onclick: {
                                    let (backend, id) = (backend.clone(), id.clone());
                                    move |_| {
                                        if let Some(id) = &id {
                                            spawn_forever(reload(dash, backend.clone(), id.clone()));
                                        }
                                    }
                                },
                                "Reload stored version"
                            }
                        }
                    },
                    div { class: "sheet-form",
                        p { "{name} changed since you opened it. Nothing was overwritten." }
                        p { class: "hint", "{message}" }
                    }
                }
            }
        }
    }
}
