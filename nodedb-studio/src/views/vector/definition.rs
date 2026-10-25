//! The Definition panel: the index as the catalog defines it, read-only.
//! Changes go through the Designer, which stages and reviews them.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::thousands;
use crate::models::vector_index::IndexDefinition;

#[component]
pub fn DefinitionBody(
    definition: IndexDefinition,
    index: String,
    on_edit: EventHandler<String>,
) -> Element {
    let kind = definition.kind.label();
    let quantization = definition.quantization.label();
    let precision = definition.raw_precision.text(|p| p.label().to_string());
    let seal = thousands(definition.seal_threshold);
    rsx! {
        div { class: "vec-def",
            table { class: "vec-kv",
                tbody {
                    tr { th { scope: "row", "Index type" } td { "{kind}" } }
                    tr { th { scope: "row", "Quantization" } td { "{quantization}" } }
                    tr { th { scope: "row", "Raw vector precision" } td { "{precision}" } }
                }
            }
            h3 { class: "vec-heading", "Construction" }
            table { class: "vec-kv",
                tbody {
                    if let Some(h) = definition.hnsw {
                        tr { th { scope: "row", "M" } td { class: "mono", "{h.m}" } }
                        tr { th { scope: "row", "M0" } td { class: "mono", "{h.m0}" } }
                        tr { th { scope: "row", "ef_construction" } td { class: "mono", "{h.ef_construction}" } }
                    }
                    if let Some(i) = definition.ivf {
                        tr { th { scope: "row", "Trained" } td { if i.trained { "Yes" } else { "No" } } }
                        tr { th { scope: "row", "Training threshold" } td { "{thousands(i.training_threshold)} vectors" } }
                        tr { th { scope: "row", "Cells" } td { "{i.cells}" } }
                        tr { th { scope: "row", "nprobe" } td { class: "mono", "{i.nprobe}" } }
                    }
                    tr { th { scope: "row", "Seal threshold" } td { "{seal}" } }
                }
            }
        }
        div { class: "vec-def-foot",
            button {
                class: "btn wide",
                title: "Open {index} in the Designer. Edits there are staged and reviewed before anything changes.",
                onclick: move |_| on_edit.call(index.clone()),
                Ico { icon: Icon::FileText, size: 16 }
                "Edit in Designer"
            }
        }
    }
}
