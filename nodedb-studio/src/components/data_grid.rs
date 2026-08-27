//! Tabular renderer for a seam `ResultSet`: column headers plus keyed rows.
//!
//! Pure presentation. Every cell is already a display string, because every
//! wire scalar arrives as one; this component never parses. Sorting or typed
//! comparison is not possible from display strings ("14,820", "—"), so it
//! waits on the model carrying raw values, not on this component.
//!
//! Headers are keyed by ordinal: SQL allows duplicate column names
//! (`SELECT a, a`) and Dioxus panics in debug on duplicate sibling keys.
//! Rows are keyed by `RecordRow.id`, which the seam guarantees unique.

use dioxus::prelude::*;

use crate::models::workbench::ResultSet;

#[component]
pub fn DataGrid(result: ResultSet) -> Element {
    rsx! {
        table { class: "data-grid",
            thead {
                tr {
                    for (i, col) in result.columns().iter().enumerate() {
                        th { key: "{i}", "{col}" }
                    }
                }
            }
            tbody {
                for row in result.rows() {
                    tr { key: "{row.id}",
                        for (i, cell) in row.cells.iter().enumerate() {
                            td { key: "{i}", "{cell}" }
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
    use crate::models::explorer::RecordRow;

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    fn row(id: &str, cells: &[&str]) -> RecordRow {
        RecordRow {
            id: id.into(),
            cells: cells.iter().map(|c| c.to_string()).collect(),
        }
    }

    fn grid(columns: &[&str], rows: Vec<RecordRow>) -> ResultSet {
        ResultSet::new(
            columns.iter().map(|c| c.to_string()).collect(),
            rows,
            0,
            String::new(),
        )
        .expect("test fixtures are rectangular")
    }

    #[test]
    fn renders_rows_verbatim_in_column_and_row_order() {
        fn app() -> Element {
            let r = grid(
                &["type", "n"],
                vec![
                    row("r1", &["page_view", "14,820"]),
                    row("r2", &["click", "8,041"]),
                ],
            );
            rsx! { DataGrid { result: r } }
        }
        let html = render(app);
        // Whole rows verbatim: order and header/cell alignment, not just presence.
        assert!(
            html.contains("<thead><tr><th>type</th><th>n</th></tr></thead>"),
            "{html}"
        );
        // Contiguous, so row order is pinned too, not only each row's contents.
        assert!(
            html.contains(
                "<tr><td>page_view</td><td>14,820</td></tr><tr><td>click</td><td>8,041</td></tr>"
            ),
            "{html}"
        );
        assert_eq!(
            html.matches("<tr>").count(),
            3,
            "1 header row + 2 data rows"
        );
    }

    #[test]
    fn zero_rows_still_renders_headers() {
        fn app() -> Element {
            rsx! { DataGrid { result: grid(&["type", "n"], Vec::new()) } }
        }
        let html = render(app);
        assert!(html.contains("<th>type</th>"));
        assert!(!html.contains("<td>"), "no cells for zero rows");
    }

    /// Duplicate column names are legal SQL and must not produce duplicate
    /// sibling keys, which panic in debug builds.
    #[test]
    fn duplicate_column_names_render_both_headers() {
        fn app() -> Element {
            rsx! { DataGrid { result: grid(&["a", "a"], vec![row("r1", &["1", "2"])]) } }
        }
        let html = render(app);
        assert_eq!(html.matches("<th>a</th>").count(), 2, "{html}");
    }
}
