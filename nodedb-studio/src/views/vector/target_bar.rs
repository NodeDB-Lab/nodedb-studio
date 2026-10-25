//! The target row: the collection and vector field every tab works on, and
//! what the catalog says about that field.

use dioxus::prelude::*;

use crate::models::vector::{VectorSetup, VectorTarget};
use crate::state::vector::{TargetChoice, VectorUi};

#[component]
pub fn TargetBar(setup: VectorSetup, target: VectorTarget) -> Element {
    let mut choice = use_context::<VectorUi>().choice;
    let collections: Vec<String> = setup.collections().iter().map(|c| c.to_string()).collect();
    let fields: Vec<String> = setup
        .fields(&target.collection)
        .iter()
        .map(|f| f.to_string())
        .collect();
    rsx! {
        div { class: "toolbar nowrap vec-target",
            label { r#for: "vec-collection", class: "toolbar-label", "Collection" }
            select {
                id: "vec-collection",
                class: "select native",
                onchange: move |e| choice.set(TargetChoice { collection: Some(e.value()), field: None }),
                for c in collections.iter() {
                    option { key: "{c}", value: "{c}", selected: *c == target.collection, "{c}" }
                }
            }
            label { r#for: "vec-field", class: "toolbar-label", "Field" }
            select {
                id: "vec-field",
                class: "select native",
                onchange: {
                    let collection = target.collection.clone();
                    move |e| choice.set(TargetChoice { collection: Some(collection.clone()), field: Some(e.value()) })
                },
                for f in fields.iter() {
                    option { key: "{f}", value: "{f}", selected: *f == target.field, "{f}" }
                }
            }
            span { class: "toolbar-note",
                "{target.dimensions} dimensions · {target.metric.label()}"
            }
        }
    }
}
