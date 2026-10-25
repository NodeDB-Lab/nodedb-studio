//! The Lifecycle sheets: edit retention, define a rollup, edit the
//! late-data policy, and review pending changes. The first three only queue
//! a change. The review is where a change is read in full, bound to the
//! catalog revision, and applied.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::timeseries::Bucket;
use crate::models::ts_lifecycle::{
    BeyondHorizon, DraftField, LatePolicy, LateWrites, Lifecycle, LifecycleChange, RefreshPolicy,
    Retention, RetentionDraft, RollupDraft,
};
use crate::state::timeseries::{LifeSheet, TimeseriesUi};
use crate::views::events::common::FormField;
use crate::views::timeseries::life_ops::{
    candidate_dimensions, candidate_outputs, queue, set_input, toggle_dimension, toggle_output,
};
use crate::views::timeseries::life_retention::RetentionInput;
use crate::views::timeseries::life_review::ReviewSheet;

#[component]
pub fn LifeSheets(sheet: LifeSheet, l: Lifecycle, collection: String, pending: usize) -> Element {
    let ts = use_context::<TimeseriesUi>();
    let mut life = ts.life;
    let close = use_callback(move |()| {
        life.write().sheet = None;
    });

    match sheet {
        LifeSheet::Policy(draft) => {
            let issue = draft.issue();
            let unchanged = !draft.changes(&l);
            rsx! {
                Sheet {
                    title: "Edit retention policy".to_string(),
                    on_close: close,
                    footer: rsx! {
                        SheetFooter {
                            error: issue.clone(),
                            hint: Some("Nothing changes yet. The review shows what expires.".to_string()),
                            button { class: "btn", onclick: move |_| close.call(()), "Cancel" }
                            button {
                                class: "btn primary",
                                disabled: issue.is_some() || unchanged,
                                onclick: {
                                    let draft = draft.clone();
                                    move |_| {
                                        let mut w = life.write();
                                        queue(&mut w, LifecycleChange::Retention(draft.clone()));
                                        w.sheet = None;
                                    }
                                },
                                "Add to review"
                            }
                        }
                    },
                    div { class: "sheet-form",
                        RetentionInput {
                            id: "lp-raw".to_string(),
                            label: format!("Raw retention of {}", l.collection),
                            value: draft.raw_keep,
                            on_change: {
                                let draft = draft.clone();
                                move |r: Retention| {
                                    life.write().sheet = Some(LifeSheet::Policy(RetentionDraft { raw_keep: r, ..draft.clone() }));
                                }
                            },
                        }
                        for (name , keep) in draft.rollup_keep.iter() {
                            {
                                let name = name.clone();
                                rsx! {
                                    RetentionInput {
                                        key: "{name}",
                                        id: format!("lp-{name}"),
                                        label: format!("Retention of {name}"),
                                        value: *keep,
                                        on_change: {
                                            let (draft, name) = (draft.clone(), name.clone());
                                            move |r: Retention| {
                                                let mut next = draft.clone();
                                                for (n, k) in next.rollup_keep.iter_mut() {
                                                    if *n == name {
                                                        *k = r;
                                                    }
                                                }
                                                life.write().sheet = Some(LifeSheet::Policy(next));
                                            }
                                        },
                                    }
                                }
                            }
                        }
                        div { class: "callout info",
                            Ico { icon: Icon::Info, size: 16 }
                            span { "Raw retention and rollup retention are independent. A rollup keeps its buckets after raw rows expire. Archive policies are not available: no destination is designed." }
                        }
                    }
                }
            }
        }
        LifeSheet::Rollup(draft) => {
            let issues = draft.issues(&l);
            let inputs = l.inputs();
            let candidates = candidate_outputs(&l, &draft.input);
            let dims = candidate_dimensions(&l, &draft.input);
            let lossy = dims.iter().any(|d| !draft.group_by.contains(d));
            let has = |f: DraftField| {
                issues
                    .iter()
                    .find(|i| i.field == f)
                    .map(|i| i.message.clone())
            };
            let shown_issue = |f: DraftField| {
                if draft.name.trim().is_empty() && f == DraftField::Name {
                    None
                } else {
                    has(f)
                }
            };
            let refresh_choices = [
                "Every 1m", "Every 5m", "Every 1h", "On seal", "On flush", "Manual",
            ];
            let refresh_now = draft.refresh.label();
            rsx! {
                Sheet {
                    title: "New rollup".to_string(),
                    on_close: close,
                    width: SheetWidth::Narrow,
                    footer: rsx! {
                        SheetFooter {
                            error: issues.iter().find(|i| i.field != DraftField::Name || !draft.name.trim().is_empty()).map(|i| i.message.clone()),
                            hint: Some("Nothing is created yet. The review previews the definition and its dependencies.".to_string()),
                            button { class: "btn", onclick: move |_| close.call(()), "Cancel" }
                            button {
                                class: "btn primary",
                                disabled: !issues.is_empty(),
                                onclick: {
                                    let draft = draft.clone();
                                    move |_| {
                                        let mut w = life.write();
                                        queue(&mut w, LifecycleChange::CreateRollup(draft.clone()));
                                        w.sheet = None;
                                    }
                                },
                                "Add to review"
                            }
                        }
                    },
                    div { class: "sheet-form",
                        FormField { label: "Name".to_string(), id: "nr-name".to_string(), error: shown_issue(DraftField::Name),
                            input {
                                id: "nr-name",
                                class: "input mono",
                                placeholder: "sales_weekly",
                                value: "{draft.name}",
                                autofocus: true,
                                oninput: {
                                    let draft = draft.clone();
                                    move |e| life.write().sheet = Some(LifeSheet::Rollup(RollupDraft { name: e.value(), ..draft.clone() }))
                                },
                            }
                        }
                        div { class: "field-row two",
                            FormField { label: "Input".to_string(), id: "nr-input".to_string(), error: has(DraftField::Input),
                                select {
                                    id: "nr-input",
                                    class: "select native",
                                    onchange: {
                                        let (draft, l) = (draft.clone(), l.clone());
                                        move |e| {
                                            let mut next = draft.clone();
                                            set_input(&mut next, &l, &e.value());
                                            life.write().sheet = Some(LifeSheet::Rollup(next));
                                        }
                                    },
                                    for i in inputs.iter() {
                                        option { key: "{i}", value: "{i}", selected: *i == draft.input, "{i}" }
                                    }
                                }
                            }
                            FormField { label: "Bucket".to_string(), id: "nr-bucket".to_string(), error: has(DraftField::Bucket),
                                select {
                                    id: "nr-bucket",
                                    class: "select native",
                                    onchange: {
                                        let draft = draft.clone();
                                        move |e| {
                                            if let Some(b) = Bucket::from_label(&e.value()) {
                                                life.write().sheet = Some(LifeSheet::Rollup(RollupDraft { bucket: b, ..draft.clone() }));
                                            }
                                        }
                                    },
                                    for b in Bucket::ALL {
                                        option { key: "{b.label()}", value: "{b.label()}", selected: b == draft.bucket, "{b.label()}" }
                                    }
                                }
                            }
                        }
                        FormField { label: "Outputs".to_string(), id: "nr-outputs".to_string(), error: has(DraftField::Outputs),
                            hint: Some("Store SUM and COUNT, not AVG: sums and counts merge exactly. Divide when reading.".to_string()),
                            div { class: "check-list", id: "nr-outputs",
                                for o in candidates.iter() {
                                    {
                                        let on = draft.outputs.iter().any(|x| x.func == o.func && x.column == o.column);
                                        let (draft, o) = (draft.clone(), o.clone());
                                        rsx! {
                                            label { key: "{o.alias}", class: "check-row",
                                                input {
                                                    r#type: "checkbox",
                                                    class: "checkbox",
                                                    checked: on,
                                                    onchange: {
                                                        let (draft, o) = (draft.clone(), o.clone());
                                                        move |_| {
                                                            let mut next = draft.clone();
                                                            toggle_output(&mut next, &o);
                                                            life.write().sheet = Some(LifeSheet::Rollup(next));
                                                        }
                                                    },
                                                }
                                                span { class: "mono", "{o.logical()}" }
                                                span { class: "ts-muted", "as {o.alias}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        FormField { label: "Dimensions".to_string(), id: "nr-dims".to_string(), error: has(DraftField::Group),
                            div { class: "check-list", id: "nr-dims",
                                for d in dims.iter() {
                                    {
                                        let on = draft.group_by.contains(d);
                                        let (draft, d) = (draft.clone(), d.clone());
                                        rsx! {
                                            label { key: "{d}", class: "check-row",
                                                input {
                                                    r#type: "checkbox",
                                                    class: "checkbox",
                                                    checked: on,
                                                    onchange: {
                                                        let (draft, d) = (draft.clone(), d.clone());
                                                        move |_| {
                                                            let mut next = draft.clone();
                                                            toggle_dimension(&mut next, &d);
                                                            life.write().sheet = Some(LifeSheet::Rollup(next));
                                                        }
                                                    },
                                                }
                                                span { class: "mono", "{d}" }
                                            }
                                        }
                                    }
                                }
                            }
                            if lossy {
                                label { class: "check-row ts-lossy",
                                    input {
                                        r#type: "checkbox",
                                        class: "checkbox",
                                        checked: draft.allow_lossy,
                                        onchange: {
                                            let draft = draft.clone();
                                            move |e| life.write().sheet = Some(LifeSheet::Rollup(RollupDraft { allow_lossy: e.checked(), ..draft.clone() }))
                                        },
                                    }
                                    span { "Allow dropping dimensions. Merged series cannot be split again." }
                                }
                            }
                        }
                        div { class: "field-row two",
                            FormField { label: "Refresh".to_string(), id: "nr-refresh".to_string(),
                                select {
                                    id: "nr-refresh",
                                    class: "select native",
                                    onchange: {
                                        let draft = draft.clone();
                                        move |e| {
                                            let v = e.value();
                                            let policy = match v.as_str() {
                                                "On seal" => RefreshPolicy::OnSeal,
                                                "On flush" => RefreshPolicy::OnFlush,
                                                "Manual" => RefreshPolicy::Manual,
                                                other => RefreshPolicy::Interval(other.trim_start_matches("Every ").to_string()),
                                            };
                                            life.write().sheet = Some(LifeSheet::Rollup(RollupDraft { refresh: policy, ..draft.clone() }));
                                        }
                                    },
                                    for c in refresh_choices {
                                        option { key: "{c}", value: "{c}", selected: c == refresh_now, "{c}" }
                                    }
                                }
                            }
                            RetentionInput {
                                id: "nr-keep".to_string(),
                                label: "Retention".to_string(),
                                value: draft.keep,
                                on_change: {
                                    let draft = draft.clone();
                                    move |r: Retention| life.write().sheet = Some(LifeSheet::Rollup(RollupDraft { keep: r, ..draft.clone() }))
                                },
                            }
                        }
                    }
                }
            }
        }
        LifeSheet::Late(policy) => {
            let issue = policy.issue();
            rsx! {
                Sheet {
                    title: "Edit late-data policy".to_string(),
                    on_close: close,
                    footer: rsx! {
                        SheetFooter {
                            error: issue.map(str::to_string),
                            hint: Some("Nothing changes yet. The review shows the effect.".to_string()),
                            button { class: "btn", onclick: move |_| close.call(()), "Cancel" }
                            button {
                                class: "btn primary",
                                disabled: issue.is_some() || policy == l.late,
                                onclick: move |_| {
                                    let mut w = life.write();
                                    queue(&mut w, LifecycleChange::LatePolicy(policy));
                                    w.sheet = None;
                                },
                                "Add to review"
                            }
                        }
                    },
                    div { class: "sheet-form",
                        FormField { label: "Event time".to_string(), id: "lt-event".to_string(),
                            input { id: "lt-event", class: "input mono", readonly: true, value: "{l.time_field} · UTC" }
                        }
                        FormField { label: "Correction horizon (days)".to_string(), id: "lt-horizon".to_string(),
                            hint: Some("An example value, never a default: a write older than this is handled by the policy below.".to_string()),
                            input {
                                id: "lt-horizon",
                                class: "input",
                                r#type: "number",
                                min: "1",
                                value: "{policy.horizon_days}",
                                oninput: move |e| {
                                    let days = e.value().trim().parse().unwrap_or(0);
                                    life.write().sheet = Some(LifeSheet::Late(LatePolicy { horizon_days: days, ..policy }));
                                },
                            }
                        }
                        FormField { label: "Writes inside the horizon".to_string(), id: "lt-within".to_string(),
                            select {
                                id: "lt-within",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    if let Some(w) = LateWrites::ALL.into_iter().find(|w| w.label() == v) {
                                        life.write().sheet = Some(LifeSheet::Late(LatePolicy { within: w, ..policy }));
                                    }
                                },
                                for w in LateWrites::ALL {
                                    option { key: "{w.label()}", value: "{w.label()}", selected: w == policy.within, "{w.label()}" }
                                }
                            }
                        }
                        FormField { label: "Writes past the horizon".to_string(), id: "lt-beyond".to_string(),
                            select {
                                id: "lt-beyond",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    if let Some(b) = BeyondHorizon::ALL.into_iter().find(|b| b.label() == v) {
                                        life.write().sheet = Some(LifeSheet::Late(LatePolicy { beyond: b, ..policy }));
                                    }
                                },
                                for b in BeyondHorizon::ALL {
                                    option { key: "{b.label()}", value: "{b.label()}", selected: b == policy.beyond, "{b.label()}" }
                                }
                            }
                        }
                    }
                }
            }
        }
        LifeSheet::Review {
            review,
            busy,
            error,
        } => rsx! {
            ReviewSheet { review, busy, error, collection, pending }
        },
    }
}
