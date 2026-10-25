//! The Query panel: the exact SQL the draft runs, the fields it can use, and
//! the properties of the answer. Edit SQL opens the SQL in the Query page:
//! a builder round trip would drop any clause it cannot draw.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::tabs::TabBar;
use crate::models::format::{date_minute, thousands};
use crate::models::timeseries::{ExploreQuery, ExploreResult, GapPolicy, TimeSource};
use crate::state::timeseries::QueryTab;
use crate::views::events::common::SqlBlock;

/// The role a field plays in the draft, as short words.
pub fn roles(q: &ExploreQuery, source: &TimeSource, field: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    if source
        .time_fields
        .iter()
        .any(|t| t.name == field && t.time_key)
    {
        out.push("TIME_KEY");
    }
    if q.time_field == field {
        out.push("time");
    }
    if q.measure.field.as_deref() == Some(field) {
        out.push("value");
    }
    if q.group_by.as_deref() == Some(field) {
        out.push("group");
    }
    if q.filters.iter().any(|f| f.field == field) {
        out.push("filter");
    }
    out
}

#[component]
pub fn QueryBody(
    draft: ExploreQuery,
    source: TimeSource,
    shown: Option<ExploreResult>,
    tab: QueryTab,
    changed: bool,
    on_tab: EventHandler<QueryTab>,
    on_gap: EventHandler<GapPolicy>,
    on_edit_sql: EventHandler<()>,
) -> Element {
    let unit = source.unit_of(&draft);
    let time_key = source
        .time_fields
        .iter()
        .any(|t| t.name == draft.time_field && t.time_key);
    let items: Vec<&'static str> = QueryTab::ALL.iter().map(|t| t.label()).collect();
    let active = QueryTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    let sql = draft.sql();

    rsx! {
        div { class: "ts-query",
            div { class: "ts-query-tabs",
                TabBar {
                    items,
                    active,
                    class: "sub",
                    on_select: move |i: usize| {
                        if let Some(t) = QueryTab::ALL.get(i) {
                            on_tab.call(*t);
                        }
                    },
                }
                button {
                    class: "link-btn ts-edit-sql",
                    title: "Open this SQL in the Query page. It does not run.",
                    onclick: move |_| on_edit_sql.call(()),
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit SQL"
                }
            }
            match tab {
                QueryTab::Sql => rsx! {
                    SqlBlock { sql, class: "ts-sql" }
                    if changed {
                        div { class: "ts-draft-note", role: "status",
                            "This SQL is the draft. The chart still shows the last run. Press Run to update it."
                        }
                    }
                },
                QueryTab::Fields => rsx! {
                    div { class: "dt-wrap ts-fields",
                        table { class: "dt",
                            thead { tr { th { "Field" } th { "Type" } th { "Used as" } } }
                            tbody {
                                for f in source.fields.iter() {
                                    tr { key: "{f.name}",
                                        td { class: "mono", "{f.name}" }
                                        td { class: "ts-type", "{f.type_label}" }
                                        td {
                                            for r in roles(&draft, &source, &f.name) {
                                                span { key: "{r}", class: "tag", "{r}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            }
            h3 { class: "ts-props-title", "Properties" }
            dl { class: "ts-props",
                dt { "Collection" }
                dd { "{draft.collection}" }
                dt { "Time field" }
                dd {
                    "{draft.time_field}"
                    if time_key {
                        span { class: "badge outline", "TIME_KEY" }
                    }
                }
                dt { "Display unit" }
                dd {
                    match unit {
                        Some(u) => rsx! { "{u}" },
                        None => rsx! { span { class: "ts-muted", title: "No currency is pinned, so no unit is shown. Units are never read off a field name.", "None" } },
                    }
                }
                dt { "Missing buckets" }
                dd {
                    select {
                        class: "select native",
                        "aria-label": "Missing buckets",
                        title: "{draft.gap.hint()}",
                        onchange: move |e| {
                            let v = e.value();
                            if let Some(p) = GapPolicy::ALL.into_iter().find(|p| p.label() == v) {
                                on_gap.call(p);
                            }
                        },
                        for p in GapPolicy::ALL {
                            option { key: "{p.label()}", value: "{p.label()}", selected: p == draft.gap, "{p.label()}" }
                        }
                    }
                }
                if let Some(r) = shown {
                    dt { "Rows read" }
                    dd { "{thousands(r.rows_matched)} of {thousands(r.rows_scanned)}" }
                    dt { "Observed" }
                    dd { "{date_minute(r.observed_at, 0)} UTC" }
                }
            }
            div { class: "ts-gap-hint", "{draft.gap.hint()}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;

    #[tokio::test]
    async fn fields_say_how_the_draft_uses_them() {
        let catalog = MockConnectionService::ready()
            .time_sources()
            .await
            .expect("catalog");
        let source = catalog.source("sales").expect("sales");
        let q = source.default_query(catalog.observed_at).expect("query");
        assert_eq!(roles(&q, source, "sold_at"), ["TIME_KEY", "time"]);
        assert_eq!(roles(&q, source, "revenue"), ["value"]);
        assert_eq!(roles(&q, source, "store_id"), ["group"]);
        assert_eq!(roles(&q, source, "currency"), ["filter"]);
        assert!(roles(&q, source, "quantity").is_empty());
    }
}
