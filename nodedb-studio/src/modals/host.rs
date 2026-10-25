//! The modal host: renders whichever modal the shared `ModalKind` signal
//! reports as open.

use dioxus::prelude::*;

use crate::components::modal::Modal;
use crate::modals::confirm_delete::ConfirmDelete;
use crate::modals::connection_form::ConnectionDialog;
use crate::modals::entity_forms::{
    DocForm, GraphEdgeForm, GraphNodeForm, KvForm, SpatialForm, StrictForm, VectorForm,
};
use crate::modals::preferences::PreferencesDialog;
use crate::state::ui::ModalKind;

/// Renders the currently-open modal (from the shared `ModalKind` signal), or
/// nothing. Overlays everything; provided once near the app root.
#[component]
pub fn ModalHost() -> Element {
    let modal = use_context::<Signal<Option<ModalKind>>>();
    match *modal.read() {
        None => rsx! {},
        Some(ModalKind::NewConnection) => rsx! { ConnectionDialog {} },
        Some(ModalKind::Preferences(pane)) => rsx! { PreferencesDialog { pane } },
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
    use std::rc::Rc;

    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::backend::Backend;
    use crate::state::connection::{ActiveConnection, ConnectError};
    use crate::state::connection_form::{ConnectionForm, TestState};
    use crate::state::connections_registry::SavedConnection;
    use crate::state::preferences::Preferences;
    use crate::state::ui::{ManagerOpen, ManagerSelection, PrefsPane};

    /// The context every dialog reads.
    fn provide_app(kind: ModalKind, form: ConnectionForm) {
        use_context_provider(|| Signal::new(Some(kind)));
        use_context_provider(|| Signal::new(Preferences::default()));
        use_context_provider(|| Signal::new(form));
        use_context_provider(|| Signal::new(Vec::<SavedConnection>::new()));
        use_context_provider(|| Rc::new(MockConnectionService::ready()) as Rc<dyn Backend>);
        use_context_provider(|| Signal::new(None::<ActiveConnection>));
        use_context_provider(|| Signal::new(ConnectError(None)));
        use_context_provider(|| Signal::new(ManagerOpen::default()));
        use_context_provider(|| Signal::new(ManagerSelection::default()));
    }

    fn render_with(kind: ModalKind, form: ConnectionForm) -> String {
        #[derive(Clone, PartialEq)]
        struct Seed(ModalKind, ConnectionForm);
        fn app(seed: Seed) -> Element {
            provide_app(seed.0, seed.1);
            rsx! { ModalHost {} }
        }
        let mut dom = VirtualDom::new_with_props(app, Seed(kind, form));
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    fn render(kind: ModalKind) -> String {
        render_with(kind, ConnectionForm::blank())
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

    /// A deep-link lands on its pane, with the list marking that pane
    /// active, instead of always opening at Appearance.
    #[test]
    fn preferences_opens_at_the_requested_pane() {
        let headings = [
            (PrefsPane::Appearance, "Appearance"),
            (PrefsPane::Editor, "Editor"),
            (PrefsPane::Keyboard, "Keyboard shortcuts"),
            (PrefsPane::Security, "Security"),
            (PrefsPane::Telemetry, "Telemetry"),
            (PrefsPane::About, "About NodeDB Studio"),
        ];
        for (pane, heading) in headings {
            let html = render(ModalKind::Preferences(pane));
            assert!(html.contains("Application preferences"), "{pane:?}: {html}");
            let active = html
                .find("prefs-cat active")
                .unwrap_or_else(|| panic!("{pane:?} has no active category: {html}"));
            let button = &html[active..];
            let button = &button[..button.find("</button>").unwrap_or(button.len())];
            assert!(button.contains(pane.label()), "{pane:?} list: {html}");
            assert_eq!(html.matches("prefs-cat active").count(), 1, "{pane:?}");
            assert!(
                html.contains(&format!(r#"<h2 class="pane-title">{heading}</h2>"#)),
                "{pane:?} pane: {html}"
            );
        }
    }

    #[test]
    fn the_appearance_pane_lists_every_real_preference() {
        let html = render(ModalKind::Preferences(PrefsPane::Appearance));
        for label in [
            "Theme",
            "Interface density",
            "Font size",
            "Editor font",
            "Reduce motion",
            "Timezone",
            "Restore defaults",
            "Save preferences",
        ] {
            assert!(html.contains(label), "missing {label}: {html}");
        }
    }

    #[test]
    fn the_connection_dialog_follows_the_form() {
        let html = render(ModalKind::NewConnection);
        assert!(html.contains("New connection"), "{html}");
        assert!(html.contains("Origin cluster"));
        assert!(html.contains("Local Lite"));
        // The renderer writes the ampersand as a numeric entity.
        assert!(html.contains("Save &#38; connect"), "{html}");
        assert!(html.contains("Test connection"));
        // No error before an attempt.
        assert!(!html.contains("field-error"), "{html}");

        let edit = ConnectionForm {
            editing: Some("staging-cluster".into()),
            name: "staging-cluster".into(),
            ..ConnectionForm::blank()
        };
        assert!(render_with(ModalKind::NewConnection, edit).contains("Edit connection"));

        let lite = ConnectionForm {
            kind: crate::models::connections::ConnectionKind::Lite,
            ..ConnectionForm::blank()
        };
        let html = render_with(ModalKind::NewConnection, lite);
        assert!(html.contains("Database file"), "{html}");
        assert!(!html.contains("Require TLS"), "{html}");
    }

    #[test]
    fn an_attempted_empty_form_shows_its_errors_and_no_pass() {
        let attempted = ConnectionForm {
            attempted: true,
            ..ConnectionForm::blank()
        };
        let html = render_with(ModalKind::NewConnection, attempted);
        assert!(html.contains("Enter a name."), "{html}");
        assert!(html.contains("Enter a host."), "{html}");
        assert!(!html.contains("Connection successful"));
    }

    #[test]
    fn a_passed_test_shows_the_result_and_not_a_save() {
        let passed = ConnectionForm {
            test: TestState::Passed(crate::models::connections::ConnectionTest {
                latency_ms: 22,
                tls_verified: true,
                server_version: "1.4.2".into(),
                db_count: 12,
            }),
            ..ConnectionForm::blank()
        };
        let html = render_with(ModalKind::NewConnection, passed);
        assert!(html.contains("Connection successful"), "{html}");
        assert!(html.contains("TLS verified · latency 22ms"), "{html}");
    }

    #[test]
    fn delete_button_starts_disabled() {
        let html = render(ModalKind::ConfirmDelete);
        assert!(html.contains("disabled"), "{html}");
    }
}
