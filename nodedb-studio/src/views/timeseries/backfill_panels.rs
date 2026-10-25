//! The bodies of the Backfill panels: the job's source and range, the
//! buckets it would replace, its identity, the run controls, and history.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::{date_minute, thousands};
use crate::models::ts_backfill::{
    BackfillPreview, BackfillRun, ConflictPolicy, RunMode, WriteMode,
};
use crate::models::ts_lifecycle::{LateWrites, Lifecycle};
use crate::state::timeseries::{BackfillUi, PreviewState, ReviewKind, TimeseriesUi};
use crate::views::events::common::{FormField, InfoTip};
use crate::views::timeseries::backfill_ops::{
    apply_range, buckets_title, preview_outdated, set_target, target_key_options, valid_preview,
};

fn edit(mut ui: Signal<BackfillUi>, f: impl FnOnce(&mut BackfillUi)) {
    f(&mut ui.write());
}

#[component]
pub fn PreviewForm(ui: BackfillUi, l: Lifecycle, database: String) -> Element {
    let ts = use_context::<TimeseriesUi>();
    let signal = ts.backfill;
    let spec = ui.spec.clone();
    let range_error = {
        let mut probe = ui.clone();
        apply_range(&mut probe).err()
    };
    let source = format!("{database}.{}", l.collection);

    rsx! {
        div { class: "ts-form",
            FormField { label: "Target table".to_string(), id: "bf-target".to_string(), class: "ts-row".to_string(),
                select {
                    id: "bf-target",
                    class: "select native",
                    onchange: {
                        let l = l.clone();
                        move |e| {
                            let name = e.value();
                            edit(signal, |u| set_target(u, &l, &name));
                        }
                    },
                    for r in l.rollups.iter() {
                        option { key: "{r.name}", value: "{r.name}", selected: r.name == spec.target, "{r.name}" }
                    }
                }
            }
            FormField { label: "Source data".to_string(), id: "bf-source".to_string(), class: "ts-row".to_string(),
                select { id: "bf-source", class: "select native",
                    option { value: "{source}", selected: true, "{source}" }
                }
            }
            FormField { label: "Time field (UTC)".to_string(), id: "bf-time".to_string(), class: "ts-row".to_string(),
                select { id: "bf-time", class: "select native",
                    option { value: "{l.time_field}", selected: true, "{l.time_field}" }
                }
            }
            FormField {
                label: "Time range (UTC)".to_string(),
                id: "bf-start".to_string(),
                class: "ts-row".to_string(),
                error: range_error.map(str::to_string),
                div { class: "ts-range-pair",
                    div { class: "ts-date",
                        input {
                            id: "bf-start",
                            class: if range_error.is_some() { "input mono invalid" } else { "input mono" },
                            value: "{ui.start}",
                            "aria-label": "Range start, included",
                            oninput: move |e| edit(signal, |u| {
                                u.start = e.value();
                                let _ = apply_range(u);
                            }),
                        }
                        Ico { icon: Icon::Calendar, size: 16 }
                    }
                    span { class: "ts-arrow", "–" }
                    div { class: "ts-date",
                        input {
                            class: if range_error.is_some() { "input mono invalid" } else { "input mono" },
                            value: "{ui.end}",
                            "aria-label": "Range end, excluded",
                            oninput: move |e| edit(signal, |u| {
                                u.end = e.value();
                                let _ = apply_range(u);
                            }),
                        }
                        Ico { icon: Icon::Calendar, size: 16 }
                    }
                }
            }
            div { class: "field ts-row",
                label { "Aggregate" }
                div { class: "ts-aggs",
                    for a in spec.aggregates.iter() {
                        select { key: "{a.alias}", class: "select native", "aria-label": "Aggregate {a.logical()}", disabled: true,
                            title: "A backfill recomputes every output the rollup stores, so a bucket is never half rebuilt.",
                            option { selected: true, "{a.logical()}" }
                        }
                    }
                    button {
                        class: "icon-btn",
                        disabled: true,
                        title: "A backfill recomputes every output the rollup stores. Add an output with a new rollup.",
                        "aria-label": "Add aggregate",
                        Ico { icon: Icon::Plus, size: 16 }
                    }
                }
            }
            FormField { label: "Currency filter".to_string(), id: "bf-currency".to_string(), class: "ts-row".to_string(),
                select {
                    id: "bf-currency",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        edit(signal, |u| u.spec.currency = (v != "All currencies").then_some(v));
                    },
                    option { value: "USD", selected: spec.currency.as_deref() == Some("USD"), "USD" }
                    option { value: "All currencies", selected: spec.currency.is_none(), "All currencies" }
                }
            }
            FormField { label: "Late arrival policy".to_string(), id: "bf-late".to_string(), class: "ts-row".to_string(),
                select {
                    id: "bf-late",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        if let Some(w) = LateWrites::ALL.into_iter().find(|w| w.label() == v) {
                            edit(signal, |u| u.spec.late = w);
                        }
                    },
                    for w in LateWrites::ALL {
                        option { key: "{w.label()}", value: "{w.label()}", selected: w == spec.late, "{w.label()}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn BucketsBody(ui: BackfillUi) -> Element {
    let spec = ui.spec.clone();
    let title = buckets_title(&spec);
    let outdated = preview_outdated(&ui);
    let day = spec.bucket.seconds() >= 86_400;

    rsx! {
        match &ui.preview {
            PreviewState::Draft => rsx! { div { class: "ts-state", "Run a dry run to list the buckets. {title} are read, not changed." } },
            PreviewState::Running(_) => rsx! { div { class: "ts-state", "Running the dry run…" } },
            PreviewState::Failed(e) => rsx! {
                div { class: "callout err ts-panel-error", role: "alert",
                    Ico { icon: Icon::CircleAlert, size: 16 }
                    span { class: "notice-text", "{e}" }
                }
            },
            PreviewState::Ready(p) => rsx! {
                if outdated {
                    div { class: "callout warn ts-stale", role: "status",
                        Ico { icon: Icon::TriangleAlert, size: 16 }
                        span { class: "notice-text", "The settings changed after this dry run. These buckets show the earlier settings. Run the dry run again." }
                    }
                }
                if p.expanded() {
                    div { class: "ts-expand", role: "status",
                        "Requested {date_minute(p.requested.start, 0)} to {date_minute(p.requested.end, 0)} expands to whole buckets: {date_minute(p.range.start, 0)} to {date_minute(p.range.end, 0)} UTC."
                    }
                }
                div { class: "dt-wrap ts-bucket-table",
                    table { class: "dt",
                        thead {
                            tr {
                                th { if day { "Day (UTC)" } else { "Bucket (UTC)" } }
                                th { class: "num", "Existing count" }
                                th { class: "num", "Proposed count" }
                                th { class: "num", "Change" }
                            }
                        }
                        tbody {
                            for b in p.buckets.iter() {
                                {
                                    let label = if day { b.day() } else { date_minute(b.start, 0) };
                                    let existing = b.existing.map_or_else(|| "None".to_string(), thousands);
                                    rsx! {
                                tr { key: "{b.start}",
                                    td { class: "mono", "{label}" }
                                    td { class: "num", "{existing}" }
                                    td { class: "num",
                                        match b.proposed {
                                            Some(v) => rsx! { "{thousands(v)}" },
                                            None => rsx! { span { class: "ts-muted", title: "The source rows are past retention. Nothing can be recomputed, and nothing is invented.", "Not retained" } },
                                        }
                                    }
                                    td { class: "num",
                                        match b.change() {
                                            Some(c) if c > 0 => rsx! { span { class: "ts-up", "+{c}" } },
                                            Some(c) if c < 0 => rsx! { span { class: "ts-down", "{c}" } },
                                            Some(_) => rsx! { "0" },
                                            None => rsx! { span { class: "ts-muted", "—" } },
                                        }
                                    }
                                }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "dt-footer ts-footer",
                    "Counts are source rows per bucket. Averages are never averaged: a rollup stores sums and counts and divides them."
                }
            },
        }
    }
}

#[component]
pub fn JobForm(ui: BackfillUi, l: Lifecycle) -> Element {
    let ts = use_context::<TimeseriesUi>();
    let signal = ts.backfill;
    let spec = ui.spec.clone();
    let keys = target_key_options(&l, &spec.target);
    let name_issue = spec
        .issues()
        .into_iter()
        .find(|i| i.contains("Name the job"));

    rsx! {
        div { class: "ts-form",
            FormField { label: "Job name".to_string(), id: "bj-name".to_string(), class: "ts-row".to_string(),
                error: name_issue,
                input {
                    id: "bj-name",
                    class: "input mono",
                    value: "{spec.job_name}",
                    oninput: move |e| edit(signal, |u| u.spec.job_name = e.value()),
                }
            }
            FormField { label: "Description".to_string(), id: "bj-desc".to_string(), class: "ts-row".to_string(),
                input {
                    id: "bj-desc",
                    class: "input",
                    value: "{spec.description}",
                    oninput: move |e| edit(signal, |u| u.spec.description = e.value()),
                }
            }
            div { class: "field ts-row",
                label { r#for: "bj-mode", "Mode" }
                select {
                    id: "bj-mode",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        if let Some(m) = WriteMode::ALL.into_iter().find(|m| m.label() == v) {
                            edit(signal, |u| u.spec.mode = m);
                        }
                    },
                    for m in WriteMode::ALL {
                        option { key: "{m.label()}", value: "{m.label()}", selected: m == spec.mode, "{m.label()}" }
                    }
                }
                InfoTip { text: "Replace affected buckets rewrites each bucket in the range. Fill missing buckets only leaves buckets that exist as they are.".to_string() }
            }
            div { class: "field ts-row",
                label { r#for: "bj-key", "Target key" }
                select {
                    id: "bj-key",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        edit(signal, |u| u.spec.target_key = v);
                    },
                    for k in keys.iter() {
                        option { key: "{k}", value: "{k}", selected: *k == spec.target_key, "{k}" }
                    }
                }
                InfoTip { text: "The key that identifies one bucket in the target. A run writes each key once.".to_string() }
            }
            div { class: "field ts-row",
                label { r#for: "bj-checkpoint", "Checkpoint" }
                select {
                    id: "bj-checkpoint",
                    class: "select native",
                    onchange: move |e| {
                        let on = e.value() == "Enabled";
                        edit(signal, |u| u.spec.checkpoint = on);
                    },
                    option { value: "Enabled", selected: spec.checkpoint, "Enabled" }
                    option { value: "Disabled", selected: !spec.checkpoint, "Disabled" }
                }
                InfoTip { text: "A checkpoint records the last published bucket. A retry or resume starts after it, so no row counts twice.".to_string() }
            }
            div { class: "field ts-row",
                label { r#for: "bj-conflict", "Conflict policy" }
                select {
                    id: "bj-conflict",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        if let Some(c) = ConflictPolicy::ALL.into_iter().find(|c| c.label() == v) {
                            edit(signal, |u| u.spec.conflict = c);
                        }
                    },
                    for c in ConflictPolicy::ALL {
                        option { key: "{c.label()}", value: "{c.label()}", selected: c == spec.conflict, "{c.label()}" }
                    }
                }
                InfoTip { text: "What happens when a bucket was written by something else since the preview.".to_string() }
            }
        }
    }
}

#[component]
pub fn RunBody(ui: BackfillUi, on_dry_run: EventHandler<()>) -> Element {
    let ts = use_context::<TimeseriesUi>();
    let signal = ts.backfill;
    let valid = valid_preview(&ui).cloned();
    let key = valid.as_ref().map(|p| ui.spec.idempotency_key(&p.revision));
    let outdated = preview_outdated(&ui);
    let running = matches!(ui.preview, PreviewState::Running(_));

    rsx! {
        div { class: "ts-run",
            div { class: "ts-mode",
                span { class: "ts-mode-label", "Mode" }
                label { class: "check-row",
                    input {
                        r#type: "radio",
                        class: "radio",
                        name: "bf-run-mode",
                        checked: ui.mode == RunMode::DryRun,
                        onchange: move |_| edit(signal, |u| u.mode = RunMode::DryRun),
                    }
                    span { "Dry run (preview only)" }
                }
                label { class: "check-row",
                    input {
                        r#type: "radio",
                        class: "radio",
                        name: "bf-run-mode",
                        checked: ui.mode == RunMode::Run,
                        onchange: move |_| edit(signal, |u| u.mode = RunMode::Run),
                    }
                    span { "Run backfill" }
                }
            }
            match (&valid, &ui.preview) {
                (Some(p), _) => rsx! { DryRunResult { preview: p.clone(), idem: key.clone().unwrap_or_default(), on_again: on_dry_run } },
                (None, PreviewState::Running(_)) => rsx! {
                    div { class: "ts-dry pending", role: "status", "Running the dry run…" }
                },
                (None, PreviewState::Failed(e)) => rsx! {
                    div { class: "callout err", role: "alert",
                        Ico { icon: Icon::CircleAlert, size: 16 }
                        span { class: "notice-text", "{e}" }
                        button { class: "btn sm", onclick: move |_| on_dry_run.call(()), "Run dry run again" }
                    }
                },
                (None, _) => rsx! {
                    div { class: "ts-dry pending", role: "status",
                        if outdated { "The settings changed since the last dry run." } else { "No dry run yet for these settings." }
                        button { class: "btn sm", onclick: move |_| on_dry_run.call(()), "Run dry run" }
                    }
                },
            }
            if let Some(e) = ui.error.clone() {
                div { class: "field-error", role: "alert", "{e}" }
            }
            div { class: "ts-run-actions",
                button {
                    class: "btn",
                    disabled: valid.is_none() || running,
                    title: if valid.is_some() { "Read the whole job. Nothing starts from here." } else { "Run a dry run first." },
                    onclick: move |_| edit(signal, |u| u.review = Some(ReviewKind::Read)),
                    "Review"
                }
                button {
                    class: "btn primary",
                    disabled: valid.is_none() || running || ui.busy,
                    title: if valid.is_some() { "Review the job, then start it." } else { "Run a dry run first." },
                    onclick: move |_| edit(signal, |u| {
                        u.mode = RunMode::Run;
                        u.review = Some(ReviewKind::Run);
                    }),
                    Ico { icon: Icon::Play, size: 16 }
                    "Run backfill"
                }
            }
        }
    }
}

#[component]
fn DryRunResult(preview: BackfillPreview, idem: String, on_again: EventHandler<()>) -> Element {
    let scanned = thousands(preview.scanned_rows);
    let cost = preview
        .cost
        .clone()
        .unwrap_or_else(|| "Not reported".to_string());
    let unavailable = preview.unavailable().len();
    rsx! {
        div { class: "ts-dry done", role: "status",
            div { class: "ts-dry-head",
                Ico { icon: Icon::CircleCheck, size: 18 }
                strong { "Dry run completed" }
                button { class: "link-btn", onclick: move |_| on_again.call(()), "Run again" }
            }
            div { class: "ts-dry-figures",
                div {
                    span { class: "ts-dry-label", "Estimated source rows scanned" }
                    span { class: "ts-dry-value", "{scanned}" }
                }
                div {
                    span { class: "ts-dry-label", "Estimated cost" }
                    span {
                        class: "ts-dry-value",
                        title: "The server reports no cost estimate, so none is shown.",
                        "{cost}"
                    }
                }
            }
            if unavailable > 0 {
                div { class: "ts-dry-note", "{unavailable} buckets are past raw retention and stay as they are." }
            }
            div { class: "ts-dry-key", "Preview {preview.revision} · key {idem}" }
        }
    }
}

#[component]
pub fn HistoryBody(
    runs: Vec<BackfillRun>,
    on_view: EventHandler<String>,
    on_resume: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "dt-wrap ts-history-table",
            table { class: "dt compact",
                thead {
                    tr {
                        th { "Run" }
                        th { "State" }
                        th { "Started (UTC)" }
                        th { class: "num", "Rows" }
                        th { "Checkpoint" }
                        th { "Action" }
                    }
                }
                tbody {
                    for r in runs.iter() {
                        {
                            let id = r.id.clone();
                            let resumable = r.resumable();
                            let short = |t: i64| date_minute(t, 0).get(5..).unwrap_or_default().to_string();
                            let started = short(r.started);
                            let checkpoint = r.checkpoint.map_or_else(|| "—".to_string(), short);
                            rsx! {
                                tr { key: "{r.id}",
                                    td { class: "key", "{r.id}" }
                                    td { span { class: "status", span { class: "status-dot {r.state.tone()}" } "{r.state.label()}" } }
                                    td { class: "mono", "{started}" }
                                    td { class: "num", "{thousands(r.rows)}" }
                                    td { class: "mono", "{checkpoint}" }
                                    td {
                                        if resumable {
                                            button { class: "link-btn", title: "Continue from the checkpoint. Rows up to it are not read again.",
                                                onclick: { let id = id.clone(); move |_| on_resume.call(id.clone()) },
                                                "Resume"
                                            }
                                        } else {
                                            button { class: "link-btn", onclick: { let id = id.clone(); move |_| on_view.call(id.clone()) }, "View" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if runs.is_empty() {
                        tr { td { colspan: "7", class: "ts-muted", "No backfill job has run for this target." } }
                    }
                }
            }
        }
    }
}
