//! The job review: what Save or Create will change, what the server will
//! run, and when. Nothing is written until the button in the footer.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::format::date_second;
use crate::models::jobs::{ScheduledJob, secs_label};
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::jobs::{Editing, JobEditor};
use crate::views::events::common::SqlBlock;
use crate::views::events::job_tasks;

#[component]
pub fn ReviewSheet(
    editor: JobEditor,
    job: Option<ScheduledJob>,
    jobs: Vec<ScheduledJob>,
    now: i64,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<EventsUi>().jobs;
    let form = editor.form.clone();
    let name = form.name.trim().to_string();
    let creating = editor.target == Editing::New;
    let changes = job
        .as_ref()
        .map(|j| form.changes_from(j))
        .unwrap_or_default();
    let issues = form.issues(&jobs, job.as_ref().map(|j| j.id.as_str()));
    let next = form.next_runs(now, 3).unwrap_or_default();
    let save_label = if creating {
        format!("Create job {name}")
    } else {
        format!("Save changes to {name}")
    };
    let mut close = move || {
        if let Some(e) = ui.write().editor.as_mut() {
            e.review = false;
        }
    };
    rsx! {
        Sheet {
            title: if creating { format!("Review new job {name}") } else { format!("Review changes to {name}") },
            width: SheetWidth::Medium,
            on_close: move |_| close(),
            footer: rsx! {
                SheetFooter {
                    error: editor.error.clone(),
                    hint: Some(if creating {
                        "The job is created enabled and runs on its schedule.".to_string()
                    } else {
                        "A saved schedule takes effect at once. It does not run the job.".to_string()
                    }),
                    button { class: "btn", disabled: editor.busy, onclick: move |_| close(), "Back to edit" }
                    button {
                        class: "btn primary",
                        disabled: editor.busy || !issues.is_empty(),
                        onclick: move |_| {
                            let backend = backend.clone();
                            spawn_forever(async move { job_tasks::save(ui, backend).await });
                        },
                        "{save_label}"
                    }
                }
            },
            div { class: "review-feed",
                if creating {
                    div { class: "review-block",
                        h3 { class: "section-title", "{name} will run" }
                        div { class: "kv",
                            div { class: "kv-row", span { "Schedule" } span { class: "mono", "{form.cron} · {form.timezone}" } }
                            div { class: "kv-row", span { "Source" } span { class: "mono", "{form.source}" } }
                            div { class: "kv-row", span { "Owner" } span { "{form.owner}" } }
                            div { class: "kv-row", span { "Timeout" } span { "{secs_label(form.timeout_secs)}" } }
                            div { class: "kv-row", span { "Overlap policy" } span { "{form.overlap.label()}" } }
                            div { class: "kv-row", span { "Retries" } span { "{form.retries}, {secs_label(form.backoff_secs)} apart" } }
                        }
                    }
                } else {
                    div { class: "review-block",
                        h3 { class: "section-title", "Changes" }
                        if changes.is_empty() {
                            div { class: "insp-sub", "No field differs from the saved job." }
                        }
                        div { class: "kv",
                            for (label, was, now_value) in changes {
                                div { key: "{label}", class: "kv-row",
                                    span { "{label}" }
                                    span { class: "change-values",
                                        span { class: "was", "{was}" }
                                        Ico { icon: Icon::ChevronRight, size: 16 }
                                        span { class: "now", "{now_value}" }
                                    }
                                }
                            }
                        }
                        if let Editing::Job { revision, .. } = &editor.target {
                            div { class: "insp-sub",
                                "Saved against revision {revision}. If someone else changed {name} since, the save is refused and nothing is overwritten."
                            }
                        }
                    }
                }
                div { class: "review-block",
                    h3 { class: "section-title", "Next executions (UTC)" }
                    if next.is_empty() {
                        div { class: "insp-sub", "This schedule never runs." }
                    }
                    ul { class: "next-list",
                        for t in next {
                            li { key: "{t}", class: "mono", "{date_second(t, 0)}" }
                        }
                    }
                }
                div { class: "review-block",
                    h3 { class: "section-title", "SQL" }
                    SqlBlock { sql: form.sql.clone(), class: "review-sql" }
                }
                div { class: "callout info",
                    Ico { icon: Icon::Info, size: 16 }
                    span {
                        "The SQL runs as {form.owner} against {form.source}, with that role's permissions. A schedule is not a stream: each run ends and leaves a run in the history."
                    }
                }
            }
        }
    }
}
