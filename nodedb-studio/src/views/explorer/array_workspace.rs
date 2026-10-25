//! The Records and details panels of an array collection.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::panel::{Panel, ResizeHandle, Side};
use crate::components::tabs::TabBar;
use crate::models::collection::Collection;
use crate::services::backend::Backend;
use crate::state::explorer::{CollectionTab, RecordView};
use crate::views::explorer::array::{
    ArrayDetails, ArrayMetadata, AttributesTable, DimensionsTable, SliceView,
};
use crate::views::explorer::page::{DETAILS_SIZE, SURFACE};
use crate::views::explorer::workspace::CollectionBadges;

#[component]
pub fn ArrayWorkspace(collection: Collection) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let name = collection.name.clone();

    let mut schema = {
        let (backend, name) = (backend.clone(), name.clone());
        use_resource(move || {
            let (backend, name) = (backend.clone(), name.clone());
            let _conn = conn();
            async move { backend.array_schema(&name).await }
        })
    };
    // What the last Run slice asked for. A memo, so typing in the range
    // inputs reads nothing until Run slice applies them.
    let request = use_memo(move || view.read().slice.applied.clone());
    let mut slice = {
        let (backend, name) = (backend.clone(), name.clone());
        use_resource(move || {
            let (backend, name) = (backend.clone(), name.clone());
            let req = request();
            let _conn = conn();
            async move { backend.array_slice(&name, &req).await }
        })
    };
    let schema_state = resource_state(&schema);
    let slice_state = resource_state(&slice);

    let tabs = CollectionTab::for_collection(&collection);
    let tab = if tabs.contains(&view.read().tab) {
        view.read().tab
    } else {
        CollectionTab::Data
    };
    let labels: Vec<&'static str> = tabs.iter().map(|t| t.label()).collect();
    let index = tabs.iter().position(|t| *t == tab).unwrap_or(0);

    rsx! {
        Panel {
            surface: SURFACE,
            id: "records",
            title: collection.name.clone(),
            class: "fill",
            title_extra: rsx! { CollectionBadges { collection: collection.clone(), read_only: false } },
            TabBar {
                items: labels,
                active: index,
                class: "collection-tabs",
                on_select: move |i: usize| {
                    if let Some(t) = tabs.get(i) {
                        view.write().tab = *t;
                    }
                },
            }
            AsyncView {
                loading: schema_state.is_loading(),
                empty: false,
                error: schema_state.error_message(),
                retriable: schema_state.is_retriable(),
                on_retry: move |_| schema.restart(),
            }
            if let Some(s) = schema_state.loaded().cloned() {
                match tab {
                    CollectionTab::Dimensions => rsx! { div { class: "tab-body", DimensionsTable { schema: s } } },
                    CollectionTab::Attributes => rsx! { div { class: "tab-body", AttributesTable { schema: s } } },
                    CollectionTab::Metadata => rsx! { div { class: "tab-body", ArrayMetadata { schema: s } } },
                    CollectionTab::Data
                    | CollectionTab::FullText
                    | CollectionTab::Schema
                    | CollectionTab::Indexes
                    | CollectionTab::Graph => rsx! {
                        SliceView {
                            schema: s,
                            slice: slice_state.clone(),
                            on_retry: move |_| slice.restart(),
                        }
                    },
                }
            }
        }
        ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
        Panel { surface: SURFACE, id: "details", title: "Dimensions & details", size: DETAILS_SIZE,
            if let Some(s) = schema_state.loaded().cloned() {
                ArrayDetails { schema: s, slice: slice_state }
            } else {
                div { class: "async-empty", "Dimensions load with the array." }
            }
        }
    }
}
