//! The sheets over Backups: the schedule form, and the review that applies
//! a snapshot, a schedule change, or a restore.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::admin::AdminChange;
use crate::models::admin_backup::{BackupChange, BackupOverview, RETENTION_CHOICES, ScheduleDraft};
use crate::services::backend::Backend;
use crate::state::admin::{AdminUi, BackupsUi};
use crate::views::admin::common::ReviewSheet;
use crate::views::admin::tasks;
use crate::views::events::common::FormField;

fn edit(mut ui: Signal<BackupsUi>, f: impl FnOnce(&mut ScheduleDraft)) {
    if let Some(d) = ui.write().schedule.as_mut() {
        f(d);
    }
}

#[component]
pub fn BackupSheets(overview: BackupOverview) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<AdminUi>().backups;
    let u = ui.read().clone();
    if let Some(state) = u.review.clone() {
        return rsx! {
            ReviewSheet {
                state,
                on_back: move |_| ui.write().review = None,
                on_apply: move |_| {
                    let backend = backend.clone();
                    spawn_forever(async move { tasks::apply_review(ui, backend).await });
                },
            }
        };
    }
    let Some(d) = u.schedule.clone() else {
        return rsx! {};
    };
    let base = overview.schedule.clone();
    let changed = d != ScheduleDraft::of(&base);
    let review = {
        let (d, base) = (d.clone(), base.clone());
        move |_| {
            let (backend, change) = (
                backend.clone(),
                AdminChange::Backup(BackupChange::SetSchedule(d.schedule(&base))),
            );
            spawn_forever(async move { tasks::open_review(ui, backend, change).await });
        }
    };
    rsx! {
        Sheet {
            title: format!("Manage schedule {}", base.name),
            width: SheetWidth::Narrow,
            on_close: move |_| ui.write().schedule = None,
            footer: rsx! {
                SheetFooter {
                    error: None,
                    hint: Some("Review changes reads what the schedule would do. Nothing changes yet.".to_string()),
                    button { class: "btn", onclick: move |_| ui.write().schedule = None, "Cancel" }
                    button { class: "btn primary", disabled: !changed, onclick: review, "Review changes" }
                }
            },
            div { class: "sheet-form ad-form",
                FormField { label: "Database", id: "sc-db",
                    input { id: "sc-db", class: "input", value: "{base.database}", readonly: true }
                }
                div { class: "field-row two",
                    FormField { label: "Hour (UTC)", id: "sc-hour",
                        select {
                            id: "sc-hour",
                            class: "select native",
                            onchange: move |e| {
                                if let Ok(h) = e.value().parse::<u32>() {
                                    edit(ui, |x| x.hour = h);
                                }
                            },
                            for h in 0..24u32 {
                                option { key: "{h}", value: "{h}", selected: h == d.hour, "{h:02}" }
                            }
                        }
                    }
                    FormField { label: "Minute", id: "sc-minute",
                        select {
                            id: "sc-minute",
                            class: "select native",
                            onchange: move |e| {
                                if let Ok(m) = e.value().parse::<u32>() {
                                    edit(ui, |x| x.minute = m);
                                }
                            },
                            for m in [0u32, 15, 30, 45] {
                                option { key: "{m}", value: "{m}", selected: m == d.minute, "{m:02}" }
                            }
                        }
                    }
                }
                FormField { label: "Retention", id: "sc-retention", hint: "Snapshots older than this are removed.".to_string(),
                    select {
                        id: "sc-retention",
                        class: "select native",
                        onchange: move |e| {
                            if let Ok(n) = e.value().parse::<u32>() {
                                edit(ui, |x| x.retention_days = n);
                            }
                        },
                        for n in RETENTION_CHOICES {
                            option { key: "{n}", value: "{n}", selected: n == d.retention_days, "{n} days" }
                        }
                    }
                }
                label { class: "cl-switch",
                    input {
                        r#type: "checkbox",
                        class: "switch",
                        checked: d.enabled,
                        onchange: move |e| edit(ui, |x| x.enabled = e.checked()),
                    }
                    "Schedule enabled"
                }
            }
        }
    }
}
