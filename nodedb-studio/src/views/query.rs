//! Query workspace: schema tree + editor + results.
//!
//! The editor is a static highlighted `<pre>` placeholder. There is no real
//! code editor yet; CodeMirror or Monaco would slot in here later.

use dioxus::prelude::*;

use crate::components::data_grid::DataGrid;
use crate::models::explorer::RecordRow;
use crate::models::workbench::ResultSet;

#[component]
pub fn Query() -> Element {
    // SEAM-UNWIRED: static placeholder. Delete when Query reads `run_query`
    // through the seam, which also needs `use_resource`, `AsyncState::from_value`
    // and `AsyncView` for the loading, empty and error states this view does
    // not render yet (see views/explorer/sidebar.rs for the shape).
    let results = ResultSet::new(
        vec!["type".into(), "n".into(), "avg_load".into()],
        [
            ("page_view", "14,820", "312"),
            ("click", "8,041", "—"),
            ("scroll", "5,209", "—"),
            ("form_submit", "481", "1,820"),
            ("error", "104", "—"),
            ("signup", "22", "2,140"),
            ("purchase", "9", "1,491"),
        ]
        .iter()
        .enumerate()
        .map(|(i, (t, n, a))| RecordRow {
            id: format!("row-{i}"),
            cells: vec![t.to_string(), n.to_string(), a.to_string()],
        })
        .collect(),
        142,
        "14M".into(),
    )
    .unwrap_or_default();
    // ponytail: unwrap_or_default hides a ragged edit as a blank grid; the test
    // below pins the placeholder at 7 rows so that edit fails the suite instead.
    rsx! {
        div { class: "view active",
            div { class: "query-view",
                div { class: "query-tabs",
                    div { class: "query-tab active", "events · last hour " span { class: "close", "×" } }
                    div { class: "query-tab", "top users " span { class: "dirty" } " " span { class: "close", "×" } }
                    div { class: "query-tab", "orders by region " span { class: "close", "×" } }
                    div { class: "query-tab-add", "+" }
                }
                div { class: "query-body-3col",
                    aside { class: "query-schema",
                        div { class: "query-schema-toolbar", input { placeholder: "Filter schema…" } }
                        div { class: "qs-section",
                            div { class: "qs-section-label", "Tables & collections" }
                            details { class: "qs-table", open: true,
                                summary { span { class: "chev", "▸" } span { class: "ico", "D" } span { class: "name", "events" } span { class: "count", "2.4M" } }
                                QsCol { name: "_id", ty: "string" }
                                QsCol { name: "type", ty: "string" }
                                QsCol { name: "user_id", ty: "string" }
                                QsCol { name: "ts", ty: "timestamp" }
                                QsCol { name: "props", ty: "jsonb" }
                                QsCol { name: "tags", ty: "array" }
                            }
                            details { class: "qs-table",
                                summary { span { class: "chev", "▸" } span { class: "ico", "D" } span { class: "name", "users" } span { class: "count", "12,481" } }
                                QsCol { name: "_id", ty: "string", pk: true }
                                QsCol { name: "name", ty: "string" }
                                QsCol { name: "email", ty: "string" }
                                QsCol { name: "country", ty: "char(2)" }
                                QsCol { name: "created_at", ty: "timestamp" }
                            }
                            details { class: "qs-table",
                                summary { span { class: "chev", "▸" } span { class: "ico", "S" } span { class: "name", "orders" } span { class: "count", "442,003" } }
                                QsCol { name: "id", ty: "int8", pk: true }
                                QsCol { name: "customer_id", ty: "string" }
                                QsCol { name: "total", ty: "decimal" }
                                QsCol { name: "status", ty: "enum" }
                            }
                            details { class: "qs-table", summary { span { class: "chev", "▸" } span { class: "ico", "V" } span { class: "name", "doc_embeddings" } span { class: "count", "1.1M" } } }
                            details { class: "qs-table", summary { span { class: "chev", "▸" } span { class: "ico", "G" } span { class: "name", "social_graph" } span { class: "count", "3.2M" } } }
                        }
                        div { class: "qs-section",
                            div { class: "qs-section-label", "Views & MVs" }
                            details { class: "qs-table", summary { span { class: "chev", "▸" } span { class: "ico", "M" } span { class: "name", "mv_top_users_24h" } } }
                        }
                    }
                    div { class: "query-main", style: "display: grid; grid-template-rows: 200px 1fr; overflow: hidden;",
                        div { class: "query-editor",
                            div { class: "gutter", "1" br {} "2" br {} "3" br {} "4" br {} "5" br {} "6" br {} "7" }
                            pre {
                                span { class: "sql-comment", "-- events from the last hour, grouped by type" }
                                "\n"
                                span { class: "sql-kw", "SELECT" }
                                "\n  type,\n  "
                                span { class: "sql-kw", "COUNT" }
                                "(*) "
                                span { class: "sql-kw", "AS" }
                                " n,\n  "
                                span { class: "sql-kw", "AVG" }
                                "((props->>"
                                span { class: "sql-str", "'ms_to_load'" }
                                ")::"
                                span { class: "sql-kw", "int" }
                                ") "
                                span { class: "sql-kw", "AS" }
                                " avg_load\n"
                                span { class: "sql-kw", "FROM" }
                                " events\n"
                                span { class: "sql-kw", "WHERE" }
                                " ts > "
                                span { class: "sql-kw", "NOW" }
                                "() - "
                                span { class: "sql-str", "'1 hour'" }
                                "::"
                                span { class: "sql-kw", "interval" }
                                "\n"
                                span { class: "sql-kw", "GROUP BY" }
                                " type "
                                span { class: "sql-kw", "ORDER BY" }
                                " n "
                                span { class: "sql-kw", "DESC" }
                                ";"
                            }
                        }
                        div { class: "query-results",
                            div { class: "results-toolbar",
                                span { class: "ok", "● {results.rows().len()} rows · {results.elapsed_ms} ms" }
                                span { "analytics · local-nodedb-dev" }
                                div { style: "margin-left:auto; display: flex; gap: 6px;",
                                    button { class: "btn small primary", "▸ Run (⌘↵)" }
                                    button { class: "btn small", "Explain" }
                                    button { class: "btn small", "Export" }
                                }
                            }
                            div { class: "results-body",
                                DataGrid { result: results.clone() }
                            }
                            div { class: "results-footer",
                                span { "page 1 of 1" }
                                span { "rows: {results.rows().len()}" }
                                span { "scanned: {results.scanned}" }
                                span { "plan: index_scan(events_ts_idx)" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn QsCol(name: String, ty: String, #[props(default = false)] pk: bool) -> Element {
    rsx! {
        div { class: "qs-col",
            span { "{name}" }
            if pk {
                span { class: "badge-pk", "pk" }
            }
            span { class: "type", "{ty}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The placeholder is built through the checked constructor and falls back
    /// to an empty set on failure. This pins it so a ragged edit to the literal
    /// fails here rather than rendering a blank workbench.
    #[test]
    fn placeholder_renders_all_seven_rows() {
        let mut dom = VirtualDom::new(Query);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert_eq!(html.matches("<tr><td>").count(), 7, "{html}");
        assert!(html.contains("<th>avg_load</th>"), "{html}");
    }
}
