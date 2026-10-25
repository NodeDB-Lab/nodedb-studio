//! The query builder: two single rows that fit at 1440px. The first picks
//! the data and the window and runs it. The second shapes the answer.
//! Nothing runs until Run.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::records::Filter;
use crate::models::timeseries::{
    Aggregate, Bucket, ExploreQuery, IssueField, Measure, Preset, QueryIssue, TimeCatalog,
    TimeSource, TimeSpan, parse_utc,
};
use crate::models::timeseries_axis::range_label;
use crate::models::ts_dashboard::Refresh;
use crate::state::timeseries::ExploreUi;
use crate::views::timeseries::explore_tasks::{choose_collection, set_range};
use crate::views::timeseries::picker::CollectionPicker;

/// Change the draft. Nothing else about the tab moves.
fn edit(mut ex: Signal<ExploreUi>, f: impl FnOnce(&mut ExploreQuery)) {
    if let Some(q) = ex.write().draft.as_mut() {
        f(q);
    }
}

/// The value choice: every row, or one numeric field.
fn set_value(q: &mut ExploreQuery, field: &str) {
    if field.is_empty() {
        q.measure = Measure::count_rows();
    } else {
        let aggregate = match q.measure.aggregate {
            Aggregate::Count => Aggregate::Sum,
            other @ (Aggregate::Sum | Aggregate::Avg | Aggregate::Min | Aggregate::Max) => other,
        };
        q.measure = Measure::of(aggregate, field);
    }
}

/// The aggregate choice. Counting counts rows: a field is never counted.
fn set_aggregate(q: &mut ExploreQuery, aggregate: Aggregate) {
    match (aggregate, &q.measure.field) {
        (Aggregate::Count, _) => q.measure = Measure::count_rows(),
        (_, Some(f)) => q.measure = Measure::of(aggregate, f),
        (_, None) => {}
    }
}

#[component]
pub fn ExploreToolbar(
    catalog: TimeCatalog,
    source: TimeSource,
    query: ExploreQuery,
    issues: Vec<QueryIssue>,
    running: bool,
    on_run: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let ui = use_context::<crate::state::timeseries::TimeseriesUi>();
    let ex = ui.explore;
    let picker_open = ex.read().picker_open;
    let refresh = ex.read().refresh;
    let has_back = !ex.read().previous_ranges.is_empty();
    let blocked = !issues.is_empty();
    let numeric: Vec<String> = source
        .numeric_fields()
        .iter()
        .map(|f| f.name.clone())
        .collect();
    let groups: Vec<String> = source
        .group_fields()
        .iter()
        .map(|f| f.name.clone())
        .collect();
    let value = query.measure.field.clone().unwrap_or_default();
    let aggregates: Vec<Aggregate> = if query.measure.field.is_some() {
        vec![
            Aggregate::Sum,
            Aggregate::Avg,
            Aggregate::Min,
            Aggregate::Max,
        ]
    } else {
        vec![Aggregate::Count]
    };
    let now = catalog.observed_at;
    let range_issue = issues.iter().any(|i| i.field == IssueField::Range);

    rsx! {
        div { class: "ts-toolbar",
            div { class: "ts-ctl",
                label { r#for: "ts-collection", "Collection" }
                CollectionPicker {
                    sources: catalog.sources.clone(),
                    current: query.collection.clone(),
                    open: picker_open,
                    on_toggle: move |open: bool| {
                        let mut w = ex;
                        w.write().picker_open = open;
                    },
                    on_pick: {
                        let catalog = catalog.clone();
                        move |name: String| {
                            let mut w = ex;
                            let mut ui = w.write();
                            ui.picker_open = false;
                            if let Some(s) = catalog.source(&name) {
                                choose_collection(&mut ui, s, catalog.observed_at, None);
                            }
                        }
                    },
                }
            }
            div { class: "ts-ctl",
                label { r#for: "ts-time", "Time" }
                select {
                    id: "ts-time",
                    class: "select native",
                    "aria-label": "Time field",
                    onchange: move |e| {
                        let v = e.value();
                        edit(ex, |q| q.time_field = v);
                    },
                    for t in source.time_fields.iter() {
                        option {
                            key: "{t.name}",
                            value: "{t.name}",
                            selected: t.name == query.time_field,
                            "{t.name}"
                        }
                    }
                }
            }
            div { class: "ts-ctl",
                label { r#for: "ts-range", "Range" }
                RangeButton {
                    span: query.span,
                    now,
                    invalid: range_issue,
                    on_apply: move |span: TimeSpan| {
                        let mut w = ex;
                        set_range(&mut w.write(), span);
                    },
                }
                if has_back {
                    button {
                        class: "icon-btn",
                        title: "Return to the previous range",
                        "aria-label": "Return to the previous range",
                        onclick: move |_| {
                            let mut w = ex;
                            if crate::views::timeseries::explore_tasks::previous_range(&mut w.write()) {
                                on_run.call(());
                            }
                        },
                        Ico { icon: Icon::RotateCcw, size: 16 }
                    }
                }
                span { class: "ts-utc", title: "Buckets align to UTC and bounds are half-open: start included, end excluded.", "UTC" }
            }
            div { class: "ts-ctl",
                label { r#for: "ts-refresh", "Refresh" }
                select {
                    id: "ts-refresh",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        let next = Refresh::ALL.into_iter().find(|r| r.label() == v).unwrap_or(Refresh::Off);
                        let mut w = ex;
                        w.write().refresh = next;
                    },
                    for r in Refresh::ALL {
                        option { key: "{r.label()}", value: "{r.label()}", selected: r == refresh, "{r.label()}" }
                    }
                }
            }
            div { class: "toolbar-spacer" }
            if running {
                button {
                    class: "btn",
                    title: "Stop waiting. The last result stays.",
                    onclick: move |_| on_cancel.call(()),
                    Ico { icon: Icon::X, size: 16 }
                    "Cancel"
                }
            } else {
                button {
                    class: "btn primary",
                    disabled: blocked,
                    title: if blocked { "Fix the query first. The first problem is listed under the builder." } else { "Run the query" },
                    onclick: move |_| on_run.call(()),
                    Ico { icon: Icon::Play, size: 16 }
                    "Run"
                }
            }
        }
        div { class: "ts-toolbar second",
            div { class: "ts-ctl",
                label { r#for: "ts-value", "Value" }
                select {
                    id: "ts-value",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        edit(ex, |q| set_value(q, &v));
                    },
                    option { value: "", selected: value.is_empty(), "All rows" }
                    for f in numeric.iter() {
                        option { key: "{f}", value: "{f}", selected: *f == value, "{f}" }
                    }
                }
            }
            div { class: "ts-ctl",
                label { r#for: "ts-agg", "Aggregate" }
                select {
                    id: "ts-agg",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        let picked = Aggregate::ALL.into_iter().find(|a| a.label() == v);
                        if let Some(a) = picked {
                            edit(ex, |q| set_aggregate(q, a));
                        }
                    },
                    for a in aggregates.iter() {
                        option { key: "{a.label()}", value: "{a.label()}", selected: *a == query.measure.aggregate, "{a.label()}" }
                    }
                }
            }
            div { class: "ts-ctl",
                label { r#for: "ts-bucket", "Bucket" }
                select {
                    id: "ts-bucket",
                    class: "select native",
                    onchange: move |e| {
                        if let Some(b) = Bucket::from_label(&e.value()) {
                            edit(ex, |q| q.bucket = b);
                        }
                    },
                    for b in Bucket::ALL {
                        option { key: "{b.label()}", value: "{b.label()}", selected: b == query.bucket, "{b.label()}" }
                    }
                }
            }
            div { class: "ts-ctl",
                label { r#for: "ts-group", "Group by" }
                select {
                    id: "ts-group",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        edit(ex, |q| q.group_by = (!v.is_empty()).then_some(v));
                    },
                    option { value: "", selected: query.group_by.is_none(), "None" }
                    for g in groups.iter() {
                        option { key: "{g}", value: "{g}", selected: query.group_by.as_deref() == Some(g.as_str()), "{g}" }
                    }
                }
            }
            for f in query.filters.iter() {
                {
                    let field = f.field.clone();
                    let label = f.label();
                    rsx! {
                        span { key: "{field}", class: "chip",
                            Ico { icon: Icon::Filter, size: 16 }
                            "{label}"
                            button {
                                class: "chip-remove",
                                title: "Remove the filter on {field}",
                                "aria-label": "Remove filter {label}",
                                onclick: move |_| edit(ex, |q| q.filters.retain(|x| x.field != field)),
                                Ico { icon: Icon::X, size: 16 }
                            }
                        }
                    }
                }
            }
            FilterAdder {
                choices: source
                    .fields
                    .iter()
                    .filter(|f| f.name != query.time_field && f.type_label.starts_with("string"))
                    .map(|f| f.name.clone())
                    .collect::<Vec<_>>(),
                on_add: move |f: Filter| {
                    edit(ex, |q| {
                        q.filters.retain(|x| x.field != f.field);
                        q.filters.push(f);
                    });
                },
            }
        }
        if let Some(first) = issues.first() {
            div { class: "ts-issue", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                span { "{first.message}" }
            }
        }
    }
}

/// The range control: the window as a label, and a popover to change it.
#[component]
pub fn RangeButton(
    span: TimeSpan,
    now: i64,
    invalid: bool,
    on_apply: EventHandler<TimeSpan>,
) -> Element {
    let mut open = use_signal(|| false);
    let mut start = use_signal(String::new);
    let mut end = use_signal(String::new);
    let mut error = use_signal(|| None::<&'static str>);
    let label = range_label(span.start, span.end);

    let mut apply = move |next: TimeSpan| {
        on_apply.call(next);
        open.set(false);
    };

    rsx! {
        div { class: "anchor",
            button {
                id: "ts-range",
                class: if invalid { "select invalid" } else { "select" },
                "aria-haspopup": "dialog",
                "aria-expanded": "{open()}",
                title: "Range: start included, end excluded, in UTC",
                onclick: move |_| {
                    if !open() {
                        start.set(crate::models::format::date_minute(span.start, 0));
                        end.set(crate::models::format::date_minute(span.end, 0));
                        error.set(None);
                    }
                    open.set(!open());
                },
                Ico { icon: Icon::Calendar, size: 16 }
                span { "{label}" }
                Ico { icon: Icon::ChevronDown, size: 16 }
            }
            if open() {
                div { class: "ts-scrim", onclick: move |_| open.set(false) }
                div {
                    class: "popover align-left ts-range-pop",
                    role: "dialog",
                    "aria-label": "Range",
                    onclick: move |e| e.stop_propagation(),
                    onkeydown: move |e| {
                        if e.key() == Key::Escape {
                            e.stop_propagation();
                            open.set(false);
                        }
                    },
                    div { class: "menu-title", "Range (UTC)" }
                    div { class: "ts-presets",
                        for p in Preset::ALL {
                            button {
                                key: "{p.label()}",
                                class: "btn sm",
                                title: "Cut back from the catalog time, once",
                                onclick: move |_| apply(p.span(now)),
                                "{p.label()}"
                            }
                        }
                    }
                    div { class: "ts-range-fields",
                        div { class: "field",
                            label { r#for: "ts-start", "Start, included" }
                            input {
                                id: "ts-start",
                                class: "input mono",
                                placeholder: "2026-06-07 00:00",
                                value: "{start}",
                                oninput: move |e| start.set(e.value()),
                            }
                        }
                        div { class: "field",
                            label { r#for: "ts-end", "End, excluded" }
                            input {
                                id: "ts-end",
                                class: "input mono",
                                placeholder: "2026-06-14 00:00",
                                value: "{end}",
                                oninput: move |e| end.set(e.value()),
                            }
                        }
                    }
                    if let Some(msg) = error() {
                        div { class: "field-error", role: "alert", "{msg}" }
                    }
                    div { class: "ts-pop-actions",
                        button { class: "btn sm", onclick: move |_| open.set(false), "Cancel" }
                        button {
                            class: "btn sm primary",
                            onclick: move |_| {
                                match (parse_utc(&start.read()), parse_utc(&end.read())) {
                                    (Some(a), Some(b)) if b > a => apply(TimeSpan::new(a, b)),
                                    (Some(_), Some(_)) => error.set(Some("The end must be after the start.")),
                                    _ => error.set(Some("Use YYYY-MM-DD HH:MM, in UTC.")),
                                }
                            },
                            "Apply"
                        }
                    }
                }
            }
        }
    }
}

/// `+ Filter`: a field and a value, joined by equality.
#[component]
pub fn FilterAdder(choices: Vec<String>, on_add: EventHandler<Filter>) -> Element {
    let mut open = use_signal(|| false);
    let mut field = use_signal(String::new);
    let mut text = use_signal(String::new);
    let picked = if choices.contains(&field.read()) {
        field.read().clone()
    } else {
        choices.first().cloned().unwrap_or_default()
    };

    rsx! {
        div { class: "anchor",
            button {
                class: "btn",
                disabled: choices.is_empty(),
                "aria-haspopup": "dialog",
                "aria-expanded": "{open()}",
                onclick: move |_| open.set(!open()),
                Ico { icon: Icon::Plus, size: 16 }
                "Filter"
            }
            if open() {
                div { class: "ts-scrim", onclick: move |_| open.set(false) }
                div {
                    class: "popover align-left ts-filter-pop",
                    role: "dialog",
                    "aria-label": "Add filter",
                    onclick: move |e| e.stop_propagation(),
                    onkeydown: move |e| {
                        if e.key() == Key::Escape {
                            e.stop_propagation();
                            open.set(false);
                        }
                    },
                    div { class: "menu-title", "Filter rows" }
                    div { class: "ts-filter-row",
                        select {
                            class: "select native",
                            "aria-label": "Filter field",
                            onchange: move |e| field.set(e.value()),
                            for c in choices.iter() {
                                option { key: "{c}", value: "{c}", selected: *c == picked, "{c}" }
                            }
                        }
                        span { class: "ts-eq", "=" }
                        input {
                            class: "input",
                            placeholder: "value",
                            "aria-label": "Filter value",
                            value: "{text}",
                            oninput: move |e| text.set(e.value()),
                        }
                    }
                    div { class: "ts-pop-actions",
                        button { class: "btn sm", onclick: move |_| open.set(false), "Cancel" }
                        button {
                            class: "btn sm primary",
                            disabled: text.read().trim().is_empty() || picked.is_empty(),
                            onclick: {
                                let picked = picked.clone();
                                move |_| {
                                    on_add.call(Filter { field: picked.clone(), value: text.read().trim().to_string() });
                                    text.set(String::new());
                                    open.set(false);
                                }
                            },
                            "Add"
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
    use crate::models::format::epoch;
    use crate::models::timeseries::GapPolicy;

    fn query() -> ExploreQuery {
        ExploreQuery {
            collection: "sales".into(),
            time_field: "sold_at".into(),
            measure: Measure::of(Aggregate::Sum, "revenue"),
            filters: Vec::new(),
            bucket: Bucket::Day1,
            group_by: None,
            gap: GapPolicy::Missing,
            span: TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0)),
        }
    }

    #[test]
    fn picking_a_field_keeps_the_aggregate_and_picking_all_rows_counts() {
        let mut q = query();
        q.measure = Measure::of(Aggregate::Max, "quantity");
        set_value(&mut q, "revenue");
        assert_eq!(q.measure.label(), "MAX(revenue)");
        set_value(&mut q, "");
        assert_eq!(q.measure.label(), "COUNT(*)");
        set_value(&mut q, "revenue");
        assert_eq!(
            q.measure.label(),
            "SUM(revenue)",
            "a count becomes a sum on a field"
        );
    }

    #[test]
    fn counting_never_names_a_field() {
        let mut q = query();
        set_aggregate(&mut q, Aggregate::Count);
        assert_eq!(q.measure.label(), "COUNT(*)");
        set_aggregate(&mut q, Aggregate::Avg);
        assert_eq!(q.measure.label(), "COUNT(*)", "no field to average");
    }
}
