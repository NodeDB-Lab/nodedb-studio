//! The Late data & backfill body: how a write that arrives after its bucket
//! closed is handled, and a backfill to preview. Preview backfill reads;
//! it never changes a bucket.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::date_minute;
use crate::models::timeseries::parse_utc;
use crate::models::ts_backfill::BackfillRun;
use crate::models::ts_lifecycle::Lifecycle;
use crate::state::timeseries::QuickBackfill;

/// Why the quick backfill cannot be previewed, if it cannot.
pub fn quick_issue(q: &QuickBackfill, l: &Lifecycle) -> Option<&'static str> {
    match (parse_utc(&q.start), parse_utc(&q.end)) {
        (Some(a), Some(b)) if b > a => {}
        (Some(_), Some(_)) => return Some("The end must be after the start."),
        _ => return Some("Use YYYY-MM-DD HH:MM, in UTC."),
    }
    if l.rollup(&q.target).is_none() {
        return Some("Choose a rollup to recompute.");
    }
    None
}

/// The target as the select shows it: dependents are recomputed with it.
pub fn target_label(l: &Lifecycle, name: &str) -> String {
    if l.dependents(name).is_empty() {
        name.to_string()
    } else {
        format!("{name} + dependent rollups")
    }
}

#[component]
pub fn LateBody(
    l: Lifecycle,
    quick: QuickBackfill,
    latest: Option<BackfillRun>,
    on_quick: EventHandler<QuickBackfill>,
    on_preview: EventHandler<()>,
    on_runs: EventHandler<()>,
) -> Element {
    let issue = quick_issue(&quick, &l);
    let late = l.late;

    rsx! {
        div { class: "ts-late",
            div { class: "ts-late-props",
                div { class: "ts-pill-field",
                    span { class: "ts-pill-label", "Event time" }
                    span { class: "ts-pill", "{l.time_field} · UTC" }
                }
                div { class: "ts-pill-field",
                    span { class: "ts-pill-label", "Correction horizon" }
                    span { class: "ts-pill short", "{late.horizon_days}d" }
                }
                div { class: "ts-pill-field grow",
                    span { class: "ts-pill-label", "Late writes" }
                    span { class: "ts-pill", "{late.within.label()}" }
                }
            }
            div { class: "ts-backfill-form",
                h3 { "Backfill" }
                div { class: "ts-bf-row",
                    label { r#for: "bf-start", "Range" }
                    div { class: "ts-date",
                        input {
                            id: "bf-start",
                            class: if issue.is_some() { "input mono invalid" } else { "input mono" },
                            value: "{quick.start}",
                            "aria-label": "Range start, UTC, included",
                            oninput: {
                                let quick = quick.clone();
                                move |e| on_quick.call(QuickBackfill { start: e.value(), ..quick.clone() })
                            },
                        }
                        Ico { icon: Icon::Calendar, size: 16 }
                    }
                    span { class: "ts-arrow", "→" }
                    div { class: "ts-date",
                        input {
                            class: if issue.is_some() { "input mono invalid" } else { "input mono" },
                            value: "{quick.end}",
                            "aria-label": "Range end, UTC, excluded",
                            oninput: {
                                let quick = quick.clone();
                                move |e| on_quick.call(QuickBackfill { end: e.value(), ..quick.clone() })
                            },
                        }
                        Ico { icon: Icon::Calendar, size: 16 }
                    }
                    span { class: "ts-utc", "UTC" }
                    label { r#for: "bf-target", "Target" }
                    select {
                        id: "bf-target",
                        class: "select native",
                        onchange: {
                            let quick = quick.clone();
                            move |e| on_quick.call(QuickBackfill { target: e.value(), ..quick.clone() })
                        },
                        for r in l.rollups.iter() {
                            option { key: "{r.name}", value: "{r.name}", selected: r.name == quick.target, "{target_label(&l, &r.name)}" }
                        }
                    }
                }
                div { class: "ts-bf-row",
                    span { class: "ts-muted", "Existing retained {l.collection} records only." }
                    div { class: "toolbar-spacer" }
                    if let Some(msg) = issue {
                        span { class: "field-error", role: "alert", "{msg}" }
                    }
                    button {
                        class: "btn primary",
                        disabled: issue.is_some(),
                        title: "Dry run: shows the buckets a backfill would replace. Changes nothing.",
                        onclick: move |_| on_preview.call(()),
                        "Preview backfill"
                    }
                }
            }
            div { class: "ts-history",
                h3 { "History" }
                div { class: "dt-wrap",
                    table { class: "dt",
                        thead { tr { th { "Run ID" } th { "Requested" } th { "Status" } th { "Details" } } }
                        tbody {
                            match latest {
                                Some(r) => rsx! {
                                    tr {
                                        td { class: "key", "{r.id}" }
                                        td { "{date_minute(r.started, 0)} UTC" }
                                        td { span { class: "status", span { class: "status-dot {r.state.tone()}" } "{r.state.label()}" } }
                                        td { button { class: "link-btn", onclick: move |_| on_runs.call(()), "View runs" } }
                                    }
                                },
                                None => rsx! { tr { td { colspan: "4", class: "ts-muted", "No backfill has run for this collection." } } },
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
    async fn the_quick_backfill_is_checked_before_it_previews() {
        let l = MockConnectionService::ready()
            .lifecycle("sales")
            .await
            .expect("l");
        let mut q = QuickBackfill::default();
        assert_eq!(quick_issue(&q, &l), None);
        q.end = q.start.clone();
        assert!(quick_issue(&q, &l).is_some());
        q.end = "soon".into();
        assert!(quick_issue(&q, &l).is_some());
        let q = QuickBackfill {
            target: "gone".into(),
            ..QuickBackfill::default()
        };
        assert!(quick_issue(&q, &l).is_some());
    }

    #[tokio::test]
    async fn a_target_with_dependents_says_they_recompute_too() {
        let l = MockConnectionService::ready()
            .lifecycle("sales")
            .await
            .expect("l");
        assert_eq!(
            target_label(&l, "sales_hourly"),
            "sales_hourly + dependent rollups"
        );
        assert_eq!(target_label(&l, "sales_daily"), "sales_daily");
    }
}
