//! The backfill review and the job details, as sheets. Review is read-only
//! until the person chooses Run: it names the range the run expanded to, the
//! source snapshot, what is replaced, what is recomputed after, and the
//! keys that make a repeat safe.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::format::{date_minute, thousands};
use crate::models::ts_backfill::{BackfillPreview, BackfillRun};
use crate::services::backend::Backend;
use crate::state::timeseries::{BackfillUi, ReviewKind, TimeseriesUi};
use crate::views::timeseries::backfill_ops::valid_preview;
use crate::views::timeseries::backfill_tasks::run;

#[component]
pub fn ReviewSheet(ui: BackfillUi, kind: ReviewKind) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ts = use_context::<TimeseriesUi>();
    let mut signal = ts.backfill;
    let close = use_callback(move |()| {
        signal.write().review = None;
    });
    let Some(p) = valid_preview(&ui).cloned() else {
        // The settings changed under an open review. It has nothing to show.
        return rsx! {
            Sheet {
                title: "Review backfill".to_string(),
                on_close: close,
                footer: rsx! {
                    SheetFooter { error: None, hint: None, button { class: "btn", onclick: move |_| close.call(()), "Close" } }
                },
                div { class: "sheet-form", p { "The settings changed after the dry run. Run the dry run again, then review." } }
            }
        };
    };
    let spec = ui.spec.clone();
    let key = spec.idempotency_key(&p.revision);
    let changed = p.changed();
    let gaps = p.unavailable().len();
    let aggregates = spec
        .aggregates
        .iter()
        .map(|a| a.logical())
        .collect::<Vec<_>>()
        .join(", ");
    let currency = spec
        .currency
        .clone()
        .unwrap_or_else(|| "All currencies".to_string());
    let dependents = if p.dependents.is_empty() {
        "No dependent rollup.".to_string()
    } else {
        format!(
            "{} are recomputed after {}.",
            p.dependents.join(", "),
            spec.target
        )
    };

    rsx! {
        Sheet {
            title: "Review backfill".to_string(),
            on_close: close,
            width: SheetWidth::Medium,
            footer: rsx! {
                SheetFooter {
                    error: ui.error.clone(),
                    hint: match kind {
                        ReviewKind::Run => Some("Starting records one job under the key above. Submitting it again is the same job.".to_string()),
                        ReviewKind::Read => Some("Read only. Close this and choose Run backfill to start.".to_string()),
                    },
                    button { class: "btn", onclick: move |_| close.call(()), "Close" }
                    if kind == ReviewKind::Run {
                        button {
                            class: "btn primary",
                            disabled: ui.busy || !spec.issues().is_empty(),
                            onclick: {
                                let backend = backend.clone();
                                move |_| { spawn_forever(run(signal, backend.clone())); }
                            },
                            Ico { icon: Icon::Play, size: 16 }
                            "Run backfill"
                        }
                    }
                }
            },
            div { class: "sheet-form ts-review",
                dl { class: "ts-kv",
                    dt { "Job" }
                    dd { "{spec.job_name}. {spec.description}" }
                    dt { "Target" }
                    dd { "{spec.target}, keyed by {spec.target_key}" }
                    dt { "Source" }
                    dd { "{spec.source}, time field {spec.time_field}, snapshot {date_minute(p.snapshot_at, 0)} UTC" }
                    dt { "Range asked" }
                    dd { "[{date_minute(p.requested.start, 0)}, {date_minute(p.requested.end, 0)}) UTC" }
                    dt { "Range run" }
                    dd {
                        "[{date_minute(p.range.start, 0)}, {date_minute(p.range.end, 0)}) UTC"
                        if p.expanded() {
                            " · widened to whole buckets"
                        }
                    }
                    dt { "Aggregates" }
                    dd { "{aggregates}" }
                    dt { "Currency" }
                    dd { "{currency}" }
                    dt { "Write mode" }
                    dd { "{spec.mode.label()}. Conflicts: {spec.conflict.label().to_lowercase()}" }
                    dt { "Late arrivals" }
                    dd { "{spec.late.label()}" }
                    dt { "Buckets" }
                    dd { "{p.buckets.len()} affected, {changed} would change" }
                    dt { "Rows read" }
                    dd { "{thousands(p.scanned_rows)}" }
                    dt { "Cost and locks" }
                    dd { class: "ts-muted", "Not reported" }
                    dt { "Recomputed after" }
                    dd { "{dependents}" }
                    dt { "Checkpoint" }
                    dd { if spec.checkpoint { "Enabled. A retry or resume continues after the last published bucket." } else { "Disabled. A retry starts from the beginning." } }
                    dt { "Preview revision" }
                    dd { class: "mono", "{p.revision}" }
                    dt { "Idempotency key" }
                    dd { class: "mono", "{key}" }
                }
                if gaps > 0 {
                    div { class: "callout warn",
                        Ico { icon: Icon::TriangleAlert, size: 16 }
                        span { "{gaps} buckets are past raw retention. They keep their current values: the source rows no longer exist, and a bucket is never rebuilt from lossy state." }
                    }
                }
                ReviewBuckets { preview: p }
            }
        }
    }
}

#[component]
fn ReviewBuckets(preview: BackfillPreview) -> Element {
    rsx! {
        div { class: "dt-wrap ts-review-table",
            table { class: "dt compact",
                thead { tr { th { "Bucket (UTC)" } th { class: "num", "Existing" } th { class: "num", "Proposed" } th { class: "num", "Change" } } }
                tbody {
                    for b in preview.buckets.iter().take(60) {
                        {
                            let existing = b.existing.map_or_else(|| "None".to_string(), thousands);
                            let proposed = b.proposed.map_or_else(|| "Not retained".to_string(), thousands);
                            let change = b.change().map_or_else(|| "—".to_string(), |c| format!("{c:+}"));
                            rsx! {
                                tr { key: "{b.start}",
                                    td { class: "mono", "{date_minute(b.start, 0)}" }
                                    td { class: "num", "{existing}" }
                                    td { class: "num", "{proposed}" }
                                    td { class: "num", "{change}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn RunDetails(run: Option<BackfillRun>, on_close: EventHandler<()>) -> Element {
    rsx! {
        Sheet {
            title: "Backfill job".to_string(),
            on_close,
            footer: rsx! {
                SheetFooter { error: None, hint: None, button { class: "btn", onclick: move |_| on_close.call(()), "Close" } }
            },
            div { class: "sheet-form",
                if let Some(r) = run {
                    {
                        let stopped = r.stopped.map_or_else(|| "Still running".to_string(), |t| format!("{} UTC", date_minute(t, 0)));
                        let checkpoint = r.checkpoint.map_or_else(|| "None".to_string(), |t| format!("{} UTC", date_minute(t, 0)));
                        rsx! {
                            dl { class: "ts-kv",
                                dt { "Job" }
                                dd { "{r.id}" }
                                dt { "Target" }
                                dd { "{r.target}" }
                                dt { "State" }
                                dd { span { class: "status", span { class: "status-dot {r.state.tone()}" } "{r.state.label()}" } }
                                dt { "Range" }
                                dd { "[{date_minute(r.range.start, 0)}, {date_minute(r.range.end, 0)}) UTC" }
                                dt { "Started" }
                                dd { "{date_minute(r.started, 0)} UTC" }
                                dt { "Stopped" }
                                dd { "{stopped}" }
                                dt { "Rows" }
                                dd { "{thousands(r.rows)} of {thousands(r.planned)} planned" }
                                dt { "Checkpoint" }
                                dd { "{checkpoint}" }
                                dt { "Idempotency key" }
                                dd { class: "mono", "{r.idempotency_key}" }
                            }
                        }
                    }
                } else {
                    p { "That job is no longer in the history." }
                }
            }
        }
    }
}
