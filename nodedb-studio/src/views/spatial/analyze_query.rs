//! The Query panel: the complete SQL of the checked form, read-only, and the
//! source's catalog details.

use dioxus::prelude::*;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::console::Protocol;
use crate::models::format::thousands;
use crate::models::spatial::GeoSource;
use crate::models::spatial_text::crs_unit;
use crate::models::sql_text::tokenize;
use crate::state::spatial::SpatialUi;

const PLACEHOLDER: &str = "-- Fix the marked fields to build the SQL.";

#[component]
pub fn QueryBody(sql: Option<String>, source: GeoSource, key_id: String, open: bool) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut analyze = ui.analyze;
    let copier = use_copier();
    let text = sql.clone().unwrap_or_else(|| PLACEHOLDER.to_string());
    let lines = text.lines().count();
    let gutter = (1..=lines)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let unknown = || "Unknown".to_string();
    let count = |v: Option<u64>| v.map_or_else(unknown, thousands);
    let rows = vec![
        ("Collection", source.source.collection.clone()),
        ("Geometry field", source.source.field.clone()),
        ("Storage", source.storage.clone()),
        (
            "SRID",
            source.srid.map_or_else(unknown, |s| format!("EPSG:{s}")),
        ),
        (
            "CRS units",
            source
                .srid
                .and_then(crs_unit)
                .map_or_else(unknown, str::to_string),
        ),
        ("Ordinates", source.ordinates.clone()),
        ("Geometries", count(source.geometry_count)),
        ("Records", count(source.record_count)),
        (
            "Geometry index",
            if source.indexes.is_empty() {
                "None listed".to_string()
            } else {
                source
                    .indexes
                    .iter()
                    .map(|i| format!("{} ({})", i.name, i.kind))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        ),
    ];
    rsx! {
        div { class: "sp-query",
            div { class: "sp-sql-head",
                span { class: "sp-sql-note", "Read-only. It matches the form." }
                button {
                    class: "icon-btn sm",
                    title: "Copy the SQL",
                    "aria-label": "Copy the SQL",
                    disabled: sql.is_none(),
                    onclick: {
                        let text = text.clone();
                        move |_| copier.copy(text.clone())
                    },
                    Ico { icon: Icon::Copy, size: 14 }
                }
            }
            div { class: "code-editor sp-sql",
                pre { class: "gutter", "aria-hidden": "true", "{gutter}" }
                pre { class: "code",
                    for (i , t) in tokenize(&text, Protocol::Sql).iter().enumerate() {
                        span { key: "{i}", class: "{t.kind.class()}", "{t.text}" }
                    }
                }
            }
            div { class: "sp-source",
                button {
                    class: "sp-disclose",
                    "aria-expanded": "{open}",
                    onclick: move |_| {
                        let mut a = analyze.write();
                        let d = a.draft_mut(&key_id);
                        d.source_open = !d.source_open;
                    },
                    Ico { icon: if open { Icon::ChevronDown } else { Icon::ChevronRight }, size: 16 }
                    "Source details"
                }
                if open {
                    dl { class: "sp-kv",
                        for (k , v) in rows {
                            div { key: "{k}", class: "sp-kv-row",
                                dt { "{k}" }
                                dd { "{v}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
