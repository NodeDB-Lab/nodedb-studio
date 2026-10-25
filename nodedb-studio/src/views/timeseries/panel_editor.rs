//! The panel editor drawer: one panel's collection, time field, value,
//! filters, bucket, grouping, gap policy, and chart, with a live preview.
//! The preview runs the same query the panel will run.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter};
use crate::models::records::Filter;
use crate::models::timeseries::{
    Aggregate, Bucket, ChartKind, GapPolicy, Measure, TimeCatalog, TimeSource,
};
use crate::models::ts_dashboard::{Dashboard, PanelDef};
use crate::services::backend::Backend;
use crate::state::timeseries::{DashUi, PanelEditor};
use crate::views::events::common::FormField;
use crate::views::timeseries::chart::TsChart;
use crate::views::timeseries::dash_ops::apply_editor;
use crate::views::timeseries::toolbar::FilterAdder;

/// Every measure the collection offers, `COUNT(*)` first.
pub fn value_options(source: &TimeSource) -> Vec<Measure> {
    let mut out = vec![Measure::count_rows()];
    for f in source.numeric_fields() {
        for a in [
            Aggregate::Sum,
            Aggregate::Avg,
            Aggregate::Min,
            Aggregate::Max,
        ] {
            out.push(Measure::of(a, &f.name));
        }
    }
    out
}

/// What stops the panel from being applied: the panel's own title, and the
/// query it will run for this document.
pub fn problems(doc: &Dashboard, panel: &PanelDef, source: Option<&TimeSource>) -> Vec<String> {
    let mut out = Vec::new();
    if panel.title.trim().is_empty() {
        out.push("Name the panel.".to_string());
    }
    match source {
        Some(s) => out.extend(doc.run_query(panel).check(s).into_iter().map(|i| i.message)),
        None => out.push(format!(
            "{} is not in this database.",
            panel.query.collection
        )),
    }
    out
}

fn edit(mut ui: Signal<DashUi>, f: impl FnOnce(&mut PanelDef)) {
    if let Some(e) = ui.write().editor.as_mut() {
        f(&mut e.draft);
    }
}

#[component]
pub fn PanelEditorSheet(
    catalog: TimeCatalog,
    doc: Dashboard,
    editor: PanelEditor,
    database: String,
    on_close: EventHandler<()>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let tsui = use_context::<crate::state::timeseries::TimeseriesUi>();
    let ui = tsui.dash;
    let draft = editor.draft.clone();
    let source = catalog.source(&draft.query.collection).cloned();
    let problems = problems(&doc, &draft, source.as_ref());
    let blocked = !problems.is_empty();
    let eligible: Vec<&TimeSource> = catalog.sources.iter().filter(|s| s.eligible()).collect();
    let is_new = editor.target.is_none();

    // The preview runs the panel's query over the document's range.
    let run_query = doc.run_query(&draft);
    let preview = {
        let backend = backend.clone();
        let q = run_query.clone();
        let skip = blocked;
        use_resource(use_reactive!(|q, skip| {
            let backend = backend.clone();
            async move {
                if skip {
                    return None;
                }
                Some(backend.explore(&q).await)
            }
        }))
    };
    let preview_value = preview.read().clone();
    let preview_state = match preview_value {
        Some(Some(r)) => Some(crate::services::async_state::AsyncState::from_value(Some(
            r,
        ))),
        _ => None,
    };

    let measure_label = draft.query.measure.label();
    let values: Vec<Measure> = source.as_ref().map(value_options).unwrap_or_default();
    let groups: Vec<String> = source
        .as_ref()
        .map(|s| s.group_fields().iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default();
    let own_filters = draft.query.filters.clone();
    let inherited: Vec<Filter> = doc
        .filters
        .iter()
        .filter(|f| !own_filters.iter().any(|o| o.field == f.field))
        .cloned()
        .collect();
    let filter_choices: Vec<String> = source
        .as_ref()
        .map(|s| {
            s.fields
                .iter()
                .filter(|f| f.name != draft.query.time_field && f.type_label.starts_with("string"))
                .map(|f| f.name.clone())
                .collect()
        })
        .unwrap_or_default();

    rsx! {
        Sheet {
            title: "Panel editor".to_string(),
            on_close,
            footer: rsx! {
                SheetFooter {
                    error: problems.first().cloned(),
                    hint: Some("Applying changes the dashboard. Nothing is stored until Save dashboard.".to_string()),
                    button { class: "btn", onclick: move |_| on_close.call(()), "Cancel" }
                    button {
                        class: "btn primary",
                        disabled: blocked,
                        onclick: move |_| {
                            let mut w = ui;
                            apply_editor(&mut w.write());
                        },
                        if is_new { "Add panel" } else { "Apply panel" }
                    }
                }
            },
            div { class: "sheet-form ts-editor",
                FormField { label: "Title".to_string(), id: "pe-title".to_string(),
                    input {
                        id: "pe-title",
                        class: "input",
                        value: "{draft.title}",
                        oninput: move |e| {
                            let v = e.value();
                            edit(ui, |p| p.title = v);
                        },
                    }
                }
                div { class: "field-row two",
                    FormField { label: "Database".to_string(), id: "pe-db".to_string(),
                        hint: Some("Panels read the database chosen in the header.".to_string()),
                        select { id: "pe-db", class: "select native", disabled: true,
                            option { "{database}" }
                        }
                    }
                    FormField { label: "Collection".to_string(), id: "pe-coll".to_string(),
                        select {
                            id: "pe-coll",
                            class: "select native",
                            onchange: {
                                let catalog = catalog.clone();
                                move |e| {
                                    let name = e.value();
                                    if let Some(q) = catalog.source(&name).and_then(|s| s.default_query(catalog.observed_at)) {
                                        edit(ui, |p| p.query = q);
                                    }
                                }
                            },
                            for s in eligible.iter() {
                                option { key: "{s.collection}", value: "{s.collection}", selected: s.collection == draft.query.collection, "{s.collection}" }
                            }
                        }
                    }
                }
                if let Some(src) = source.clone() {
                    div { class: "field-row two",
                        FormField { label: "Time field".to_string(), id: "pe-time".to_string(),
                            select {
                                id: "pe-time",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    edit(ui, |p| p.query.time_field = v);
                                },
                                for t in src.time_fields.iter() {
                                    option { key: "{t.name}", value: "{t.name}", selected: t.name == draft.query.time_field, "{t.name}" }
                                }
                            }
                        }
                        FormField { label: "Value".to_string(), id: "pe-value".to_string(),
                            select {
                                id: "pe-value",
                                class: "select native",
                                onchange: {
                                    let values = values.clone();
                                    move |e| {
                                        let v = e.value();
                                        if let Some(m) = values.iter().find(|m| m.label() == v) {
                                            let m = m.clone();
                                            edit(ui, |p| p.query.measure = m);
                                        }
                                    }
                                },
                                for m in values.iter() {
                                    option { key: "{m.label()}", value: "{m.label()}", selected: m.label() == measure_label, "{m.label()}" }
                                }
                            }
                        }
                    }
                }
                div { class: "field",
                    span { class: "field-label", "Filter" }
                    div { class: "ts-chips",
                        for f in inherited.iter() {
                            span {
                                key: "i-{f.field}",
                                class: "chip",
                                title: "Set on the dashboard. Every panel applies it.",
                                Ico { icon: Icon::Lock, size: 16 }
                                "{f.label()}"
                            }
                        }
                        for f in own_filters.iter() {
                            {
                                let field = f.field.clone();
                                let label = f.label();
                                rsx! {
                                    span { key: "o-{field}", class: "chip",
                                        Ico { icon: Icon::Filter, size: 16 }
                                        "{label}"
                                        button {
                                            class: "chip-remove",
                                            "aria-label": "Remove filter {label}",
                                            onclick: move |_| edit(ui, |p| p.query.filters.retain(|x| x.field != field)),
                                            Ico { icon: Icon::X, size: 16 }
                                        }
                                    }
                                }
                            }
                        }
                        FilterAdder {
                            choices: filter_choices,
                            on_add: move |f: Filter| {
                                edit(ui, |p| {
                                    p.query.filters.retain(|x| x.field != f.field);
                                    p.query.filters.push(f);
                                });
                            },
                        }
                    }
                }
                div { class: "field-row two",
                    FormField { label: "Bucket".to_string(), id: "pe-bucket".to_string(),
                        select {
                            id: "pe-bucket",
                            class: "select native",
                            onchange: move |e| {
                                if let Some(b) = Bucket::from_label(&e.value()) {
                                    edit(ui, |p| p.query.bucket = b);
                                }
                            },
                            for b in Bucket::ALL {
                                option { key: "{b.label()}", value: "{b.label()}", selected: b == draft.query.bucket, "{b.label()}" }
                            }
                        }
                    }
                    FormField { label: "Group by".to_string(), id: "pe-group".to_string(),
                        select {
                            id: "pe-group",
                            class: "select native",
                            onchange: move |e| {
                                let v = e.value();
                                edit(ui, |p| p.query.group_by = (!v.is_empty()).then_some(v));
                            },
                            option { value: "", selected: draft.query.group_by.is_none(), "None" }
                            for g in groups.iter() {
                                option { key: "{g}", value: "{g}", selected: draft.query.group_by.as_deref() == Some(g.as_str()), "{g}" }
                            }
                        }
                    }
                }
                div { class: "field-row two",
                    FormField { label: "Gap handling".to_string(), id: "pe-gap".to_string(),
                        hint: Some(draft.query.gap.hint().to_string()),
                        select {
                            id: "pe-gap",
                            class: "select native",
                            onchange: move |e| {
                                let v = e.value();
                                if let Some(g) = GapPolicy::ALL.into_iter().find(|g| g.label() == v) {
                                    edit(ui, |p| p.query.gap = g);
                                }
                            },
                            for g in GapPolicy::ALL {
                                option { key: "{g.label()}", value: "{g.label()}", selected: g == draft.query.gap, "{g.label()}" }
                            }
                        }
                    }
                    FormField { label: "Chart".to_string(), id: "pe-kind".to_string(),
                        select {
                            id: "pe-kind",
                            class: "select native",
                            onchange: move |e| {
                                let v = e.value();
                                if let Some(k) = ChartKind::ALL.into_iter().find(|k| k.label() == v) {
                                    edit(ui, |p| p.kind = k);
                                }
                            },
                            for k in ChartKind::ALL {
                                option { key: "{k.label()}", value: "{k.label()}", selected: k == draft.kind, "{k.label()}" }
                            }
                        }
                    }
                }
                h3 { class: "section-title", "Preview" }
                div { class: "ts-preview",
                    match preview_state {
                        Some(state) => rsx! {
                            AsyncView {
                                loading: false,
                                empty: state.is_empty(),
                                error: state.error_message(),
                                empty_message: "No rows in this range.".to_string(),
                            }
                            if let Some(r) = state.loaded().cloned() {
                                TsChart { result: r, kind: draft.kind, limit: 3, compact: true, caption: false }
                            }
                        },
                        None => rsx! {
                            if blocked {
                                div { class: "ts-state", "Fix the query to see the preview." }
                            } else {
                                div { class: "ts-state", "Loading…" }
                            }
                        },
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
    use crate::models::timeseries::TimeSpan;
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;

    async fn setup() -> (Dashboard, TimeSource) {
        let svc = MockConnectionService::ready();
        let doc = svc.dashboard("dash-1").await.expect("doc");
        let source = svc
            .time_sources()
            .await
            .expect("catalog")
            .source("sales")
            .cloned()
            .expect("sales");
        (doc, source)
    }

    #[tokio::test]
    async fn every_value_option_is_a_valid_measure_of_the_collection() {
        let (doc, source) = setup().await;
        let opts = value_options(&source);
        assert_eq!(opts[0].label(), "COUNT(*)");
        assert!(opts.iter().any(|m| m.label() == "SUM(revenue)"));
        assert!(opts.iter().all(|m| m.label() != "SUM(store_id)"));
        let _ = doc;
    }

    #[tokio::test]
    async fn a_stored_panel_has_no_problems() {
        let (doc, source) = setup().await;
        assert!(problems(&doc, &doc.panels[0], Some(&source)).is_empty());
    }

    #[tokio::test]
    async fn the_panel_needs_a_title_and_a_collection_that_exists() {
        let (doc, source) = setup().await;
        let mut p = doc.panels[0].clone();
        p.title = "  ".into();
        assert_eq!(problems(&doc, &p, Some(&source)), ["Name the panel."]);
        assert!(problems(&doc, &doc.panels[0], None)[0].contains("not in this database"));
    }

    /// The dashboard's filter reaches the panel, so a panel on a collection
    /// without that field is refused here, not at run time.
    #[tokio::test]
    async fn a_dashboard_filter_the_collection_lacks_is_a_problem() {
        let (mut doc, source) = setup().await;
        doc.filters.push(Filter {
            field: "region".into(),
            value: "APAC".into(),
        });
        assert!(!problems(&doc, &doc.panels[0], Some(&source)).is_empty());
        doc.range = TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0));
    }
}
