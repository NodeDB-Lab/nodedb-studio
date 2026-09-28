//! The modal host: renders whichever modal the shared `ModalKind` signal
//! reports as open.

use dioxus::prelude::*;

use crate::components::modal::Modal;
use crate::modals::confirm_delete::ConfirmDelete;
use crate::modals::entity_forms::{
    DocForm, GraphEdgeForm, GraphNodeForm, KvForm, SpatialForm, StrictForm, VectorForm,
};
use crate::modals::new_connection::NewConnectionForm;
use crate::modals::preferences::PreferencesPanes;
use crate::state::ui::ModalKind;

/// Renders the currently-open modal (from the shared `ModalKind` signal), or
/// nothing. Overlays everything; provided once near the app root.
#[component]
pub fn ModalHost() -> Element {
    let modal = use_context::<Signal<Option<ModalKind>>>();
    match *modal.read() {
        None => rsx! {},
        Some(ModalKind::NewConnection) => rsx! {
            Modal { title: "New connection", NewConnectionForm {} }
        },
        Some(ModalKind::Preferences) => rsx! {
            Modal { title: "Preferences", wide: true, PreferencesPanes {} }
        },
        Some(ModalKind::DocForm) => rsx! {
            Modal { title: "Edit document · evt_01H8QXG2K…", width: 640, DocForm {} }
        },
        Some(ModalKind::StrictForm) => rsx! {
            Modal { title: "Edit row · orders / 442003", width: 560, StrictForm {} }
        },
        Some(ModalKind::VectorForm) => rsx! {
            Modal { title: "Edit vector · e_001", width: 640, VectorForm {} }
        },
        Some(ModalKind::GraphNodeForm) => rsx! {
            Modal { title: "Edit node · u_44182", width: 540, GraphNodeForm {} }
        },
        Some(ModalKind::GraphEdgeForm) => rsx! {
            Modal { title: "New edge", width: 540, GraphEdgeForm {} }
        },
        Some(ModalKind::KvForm) => rsx! {
            Modal { title: "Edit · session:u_44182", width: 540, KvForm {} }
        },
        Some(ModalKind::SpatialForm) => rsx! {
            Modal { title: "Edit feature · feature_8281", width: 640, SpatialForm {} }
        },
        Some(ModalKind::ConfirmDelete) => rsx! {
            Modal { title: "Delete record", width: 420, ConfirmDelete {} }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(kind: ModalKind) -> Element {
        use_context_provider(|| Signal::new(Some(kind)));
        rsx! { ModalHost {} }
    }

    fn render(kind: ModalKind) -> String {
        let mut dom = VirtualDom::new_with_props(app, kind);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    /// Each entity/confirm variant opens its own body, not a neighbour's.
    #[test]
    fn every_entity_variant_renders_its_own_body() {
        let cases = [
            (ModalKind::DocForm, "Edit document", "Document JSON"),
            (ModalKind::StrictForm, "Edit row", "decimal(10,2)"),
            (ModalKind::VectorForm, "Edit vector", "768 floats"),
            (ModalKind::GraphNodeForm, "Edit node", "label schema"),
            (ModalKind::GraphEdgeForm, "New edge", "Create edge"),
            (ModalKind::KvForm, "session:u_44182", "no expiry"),
            (ModalKind::SpatialForm, "Edit feature", "SRID 4326"),
            (
                ModalKind::ConfirmDelete,
                "Delete record",
                "Delete permanently",
            ),
        ];
        for (kind, title, body) in cases {
            let html = render(kind);
            assert!(html.contains(title), "{kind:?} title: {html}");
            assert!(html.contains(body), "{kind:?} body: {html}");
        }
    }

    #[test]
    fn delete_button_starts_disabled() {
        let html = render(ModalKind::ConfirmDelete);
        assert!(html.contains("disabled"), "{html}");
    }
}
