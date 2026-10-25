//! Export of a result page: copy as CSV or JSON. Saving a file waits on the
//! native file dialog, which Studio does not ship yet.

use dioxus::prelude::*;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::workbench::ResultSet;

/// RFC 4180: a cell with a comma, a quote, or a line break is quoted.
fn csv_cell(cell: &str) -> String {
    if cell.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

/// The page as CSV with a header row. Cells are the display strings.
pub fn csv(result: &ResultSet) -> String {
    let mut out = result
        .columns()
        .iter()
        .map(|c| csv_cell(c))
        .collect::<Vec<_>>()
        .join(",");
    for row in result.rows() {
        out.push('\n');
        out.push_str(
            &row.cells
                .iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out
}

fn json_string(s: &str) -> String {
    sonic_rs::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

/// The page as a JSON array of objects, one per row, in column order.
pub fn json(result: &ResultSet) -> String {
    let rows: Vec<String> = result
        .rows()
        .iter()
        .map(|row| {
            let fields: Vec<String> = result
                .columns()
                .iter()
                .zip(&row.cells)
                .map(|(k, v)| format!("{}:{}", json_string(k), json_string(v)))
                .collect();
            format!("{{{}}}", fields.join(","))
        })
        .collect();
    format!("[{}]", rows.join(","))
}

#[component]
pub fn ExportMenu(result: Option<ResultSet>) -> Element {
    let mut open = use_signal(|| false);
    let copier = use_copier();
    let has_rows = result.as_ref().is_some_and(|r| !r.rows().is_empty());
    let page = result.clone();
    let page_json = result.clone();
    rsx! {
        div { class: "anchor",
            button {
                class: "btn",
                disabled: !has_rows,
                title: if has_rows { "Export the rows on this page" } else { "Run a query that returns rows to export" },
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |e| { e.stop_propagation(); open.set(!open()); },
                Ico { icon: Icon::Download, size: 16 }
                "Export"
                Ico { icon: Icon::ChevronDown, size: 16 }
            }
            if open() && has_rows {
                div {
                    class: "popover align-right",
                    role: "menu",
                    "aria-label": "Export",
                    onclick: move |e| e.stop_propagation(),
                    button {
                        class: "menu-item",
                        role: "menuitem",
                        onclick: move |_| {
                            if let Some(r) = &page { copier.copy(csv(r)); }
                            open.set(false);
                        },
                        Ico { icon: Icon::Copy, size: 16 }
                        "Copy as CSV"
                    }
                    button {
                        class: "menu-item",
                        role: "menuitem",
                        onclick: move |_| {
                            if let Some(r) = &page_json { copier.copy(json(r)); }
                            open.set(false);
                        },
                        Ico { icon: Icon::Braces, size: 16 }
                        "Copy as JSON"
                    }
                    div { class: "menu-sep" }
                    button {
                        class: "menu-item",
                        role: "menuitem",
                        disabled: true,
                        title: "Saving a file needs the native file dialog, which Studio does not ship yet",
                        Ico { icon: Icon::Download, size: 16 }
                        "Save to file"
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

    fn page() -> ResultSet {
        let row = |id: &str, a: &str, b: &str| RecordRow {
            id: id.into(),
            cells: vec![a.into(), b.into()],
        };
        ResultSet::new(
            vec!["name".into(), "n".into()],
            vec![row("1", "plain", "14,820"), row("2", "say \"hi\"", "2")],
            0,
            String::new(),
        )
        .expect("rectangular")
    }

    #[test]
    fn csv_quotes_commas_and_quotes() {
        assert_eq!(
            csv(&page()),
            "name,n\nplain,\"14,820\"\n\"say \"\"hi\"\"\",2"
        );
    }

    #[test]
    fn json_is_an_array_of_objects_in_column_order() {
        assert_eq!(
            json(&page()),
            r#"[{"name":"plain","n":"14,820"},{"name":"say \"hi\"","n":"2"}]"#
        );
    }

    #[test]
    fn an_empty_page_exports_a_header_and_an_empty_array() {
        let rs = ResultSet::new(vec!["a".into()], Vec::new(), 0, String::new()).expect("ok");
        assert_eq!(csv(&rs), "a");
        assert_eq!(json(&rs), "[]");
    }
}
