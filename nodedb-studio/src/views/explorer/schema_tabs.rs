//! The Schema, Indexes, and Full-text tabs of a record collection. They
//! read what the catalog declares and change nothing: structure edits stage
//! and review in Designer.

use dioxus::prelude::*;

use crate::components::icon::Icon;
use crate::models::collection::Collection;
use crate::models::schema::CollectionSchema;
use crate::models::search::{SearchMode, VectorInput};
use crate::routes::Route;
use crate::state::explorer::RecordView;
use crate::state::search::SearchUi;
use crate::views::explorer::details_vector::{IndexList, IndexedFields};
use crate::views::explorer::links::OpenLink;

fn designer_note(collection: &str) -> Element {
    rsx! {
        div { class: "tab-note",
            span { "Structure changes for {collection} are staged and reviewed in Designer." }
            OpenLink { route: Route::Designer {}, label: "Open Designer".to_string(), icon: Icon::Workflow }
        }
    }
}

#[component]
pub fn SchemaTab(collection: Collection, schema: CollectionSchema) -> Element {
    let flexible = !collection.profile.enforces_schema();
    rsx! {
        div { class: "tab-body",
            if flexible {
                div { class: "callout info",
                    "{collection.profile.schema_label()}: these fields are declared, and a record may carry others."
                }
            }
            div { class: "dt-wrap",
                table { class: "dt",
                    thead { tr { th { "Field" } th { "Type" } th { "Required" } th { "Notes" } } }
                    tbody {
                        for f in schema.fields.iter() {
                            {
                                let mut notes: Vec<String> = Vec::new();
                                if schema.id_field.as_deref() == Some(f.name.as_str()) {
                                    notes.push("Record id".to_string());
                                }
                                if f.time_key {
                                    notes.push("Event time".to_string());
                                }
                                if let Some(target) = &f.reference {
                                    notes.push(format!("Logical reference to {target}"));
                                }
                                rsx! {
                                    tr { key: "{f.name}",
                                        td { class: "key", "{f.name}" }
                                        td { class: "mono", "{f.ty.label()}" }
                                        td { if f.required { "Yes" } else { "No" } }
                                        td { "{notes.join(\", \")}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            {designer_note(&collection.name)}
        }
    }
}

#[component]
pub fn IndexesTab(collection: Collection, schema: CollectionSchema) -> Element {
    rsx! {
        div { class: "tab-body",
            IndexList { indexes: schema.indexes.clone() }
            {designer_note(&collection.name)}
        }
    }
}

#[component]
pub fn FullTextTab(collection: Collection, schema: CollectionSchema) -> Element {
    let view = use_context::<Signal<RecordView>>();
    let search = try_use_context::<SearchUi>();
    rsx! {
        div { class: "tab-body",
            if let Some(index) = schema.text_index.clone() {
                IndexedFields { index }
                div { class: "tab-note",
                    span { "Ranked results come from Search. This tab lists the source fields the index reads." }
                    OpenLink {
                        route: Route::Search { tab: crate::routes::tabs::SearchTab::Results },
                        label: "Open in Search".to_string(),
                        icon: Icon::Search,
                        on_open: {
                            let name = collection.name.clone();
                            move |_| {
                                let query = view.peek().text.trim().to_string();
                                if let Some(search) = search {
                                    search.open_query(&name, SearchMode::Keyword, &query, VectorInput::None);
                                }
                            }
                        },
                    }
                }
            } else {
                div { class: "empty-state",
                    h3 { "No full-text index" }
                    p { "{collection.name} has no full-text index on this connection." }
                }
            }
        }
    }
}
