//! The right-hand panel: the schema of the connected database, and the
//! details of the selected saved query. One panel, two views.

use std::collections::BTreeSet;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::Panel;
use crate::components::tabs::TabBar;
use crate::models::saved_query::QueryLibrary;
use crate::models::workbench::{SchemaField, SchemaTable, SchemaTree};
use crate::services::async_state::AsyncState;
use crate::state::query::{QueryWorkspace, SidePane};
use crate::state::workspace::Surface;
use crate::views::query::details::DetailsBody;

const SURFACE: Surface = Surface::Query;

#[component]
pub fn SidePanel(
    schema: AsyncState<SchemaTree>,
    library: AsyncState<QueryLibrary>,
    on_retry: EventHandler<()>,
    size: crate::components::panel::PanelSize,
) -> Element {
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let side = ws.read().side;
    let title = match side {
        SidePane::Schema => "Schema",
        SidePane::Details => "Query details",
    };
    rsx! {
        Panel { surface: SURFACE, id: "schema", title, size,
            TabBar {
                class: "sub side-tabs",
                items: SidePane::ALL.iter().map(|s| s.label()).collect::<Vec<_>>(),
                active: SidePane::ALL.iter().position(|s| *s == side).unwrap_or(0),
                on_select: move |i: usize| {
                    if let Some(s) = SidePane::ALL.get(i) { ws.write().side = *s; }
                },
            }
            match side {
                SidePane::Schema => rsx! { SchemaBody { tree: schema, on_retry } },
                SidePane::Details => rsx! { DetailsBody { library } },
            }
        }
    }
}

/// The tables whose name or a field name contains `needle`, ignoring case.
fn matching<'a>(all: &'a [SchemaTable], needle: &str) -> Vec<&'a SchemaTable> {
    let n = needle.trim().to_lowercase();
    all.iter()
        .filter(|t| {
            n.is_empty()
                || t.name.to_lowercase().contains(&n)
                || t.fields.iter().any(|f| f.name.to_lowercase().contains(&n))
        })
        .collect()
}

#[component]
fn SchemaBody(tree: AsyncState<SchemaTree>, on_retry: EventHandler<()>) -> Element {
    let mut filter = use_signal(String::new);
    // The first table starts open, as the design shows.
    let open = use_signal(|| BTreeSet::from(["events".to_string()]));
    let needle = filter.read().clone();
    rsx! {
        div { class: "toolbar nowrap catalog-search",
            div { class: "search-field grow",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    placeholder: "Search schema\u{2026}",
                    "aria-label": "Search schema",
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
            }
        }
        AsyncView {
            loading: tree.is_loading(),
            empty: tree.is_empty(),
            error: tree.error_message(),
            retriable: tree.is_retriable(),
            on_retry: move |_| on_retry.call(()),
            empty_message: "This database has no tables or collections.",
        }
        if let Some(t) = tree.loaded() {
            div { class: "schema-tree",
                SchemaSection { label: "Tables & collections", tables: matching(&t.tables, &needle).into_iter().cloned().collect::<Vec<_>>(), open, searching: !needle.trim().is_empty() }
                SchemaSection { label: "Views & MVs", tables: matching(&t.views, &needle).into_iter().cloned().collect::<Vec<_>>(), open, searching: !needle.trim().is_empty() }
                if matching(&t.tables, &needle).is_empty() && matching(&t.views, &needle).is_empty() {
                    div { class: "async-empty", "Nothing matches \u{201c}{needle}\u{201d}." }
                }
            }
        }
    }
}

#[component]
fn SchemaSection(
    label: &'static str,
    tables: Vec<SchemaTable>,
    open: Signal<BTreeSet<String>>,
    searching: bool,
) -> Element {
    if tables.is_empty() {
        return rsx! {};
    }
    let mut open = open;
    rsx! {
        div { class: "section-label schema-label", "{label}" }
        for t in tables {
            {
                // A search opens the tables it matched.
                let expanded = searching || open.read().contains(&t.name);
                let name = t.name.clone();
                rsx! {
                    div { key: "{t.name}", class: "schema-table",
                        button {
                            class: "schema-row",
                            "aria-expanded": "{expanded}",
                            onclick: move |_| {
                                let mut set = open.write();
                                if !set.remove(&name) { set.insert(name.clone()); }
                            },
                            Ico { icon: if expanded { Icon::ChevronDown } else { Icon::ChevronRight }, size: 16 }
                            span { class: "badge letter {t.kind.tone()}", title: "{t.kind.label()}", "{t.kind.letter()}" }
                            span { class: "schema-name", "{t.name}" }
                            span { class: "schema-count", "{t.count_label()}" }
                        }
                        if expanded {
                            for f in t.fields.iter() {
                                FieldRow { key: "{f.name}", field: f.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FieldRow(field: SchemaField) -> Element {
    rsx! {
        div { class: "schema-field",
            Ico { icon: Icon::FileText, size: 14 }
            span { class: "schema-name mono", "{field.name}" }
            if field.key {
                span { class: "badge amber", title: "Primary key", "pk" }
            }
            span { class: "schema-type", "{field.type_name}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::workbench_data::WorkbenchData;

    #[tokio::test]
    async fn the_search_matches_a_table_or_a_field_ignoring_case() {
        let tree = MockConnectionService::ready()
            .schema_tree()
            .await
            .expect("schema");
        assert_eq!(matching(&tree.tables, "").len(), tree.tables.len());
        let by_name: Vec<&str> = matching(&tree.tables, "ORD")
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(by_name, ["orders"]);
        let by_field: Vec<&str> = matching(&tree.tables, "email")
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(by_field, ["users"]);
        assert!(matching(&tree.tables, "zzz").is_empty());
    }
}
