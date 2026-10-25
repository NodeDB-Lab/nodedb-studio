//! The modal host: renders whichever modal the shared `ModalKind` signal
//! reports as open.

use dioxus::prelude::*;

use crate::modals::confirm_delete::ConfirmDelete;
use crate::modals::connection_form::ConnectionDialog;
use crate::modals::preferences::PreferencesDialog;
use crate::modals::record_editor::RecordEditorDialog;
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
        Some(ModalKind::RecordEditor) => rsx! { RecordEditorDialog {} },
        Some(ModalKind::ConfirmDelete) => rsx! { ConfirmDelete {} },
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::models::collection::Profile;
    use crate::models::records::Record;
    use crate::models::schema::{CollectionSchema, FieldType, SchemaField};
    use crate::services::MockConnectionService;
    use crate::services::backend::Backend;
    use crate::state::connection::{ActiveConnection, ConnectError};
    use crate::state::connection_form::{ConnectionForm, TestState};
    use crate::state::connections_registry::SavedConnection;
    use crate::state::explorer::{RecordView, RecordsEpoch};
    use crate::state::preferences::Preferences;
    use crate::state::record_editor::{DeleteRequest, DeleteTarget, RecordEditor};
    use crate::state::ui::{ManagerOpen, ManagerSelection, PrefsPane};

    fn sample_editor() -> RecordEditor {
        let mut schema = CollectionSchema::new("users");
        schema.id_field = Some("id".into());
        schema.fields = vec![SchemaField {
            name: "id".into(),
            ty: FieldType::String { max: None },
            required: true,
            reference: None,
            time_key: false,
        }];
        let stored = Record::new(
            "u1",
            18,
            vec![("id", nodedb_types::Value::String("u1".into()))],
        );
        RecordEditor::edit("users", Profile::Document, schema, stored)
    }

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
        use_context_provider(|| Signal::new(Some(sample_editor())));
        use_context_provider(|| {
            Signal::new(Some(DeleteRequest::new(DeleteTarget::Record {
                collection: "users".into(),
                id: "u1".into(),
                revision: 18,
            })))
        });
        use_context_provider(|| RecordsEpoch(Signal::new(0)));
        use_context_provider(|| Signal::new(RecordView::default()));
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

    /// The editor names the record and its revision, and offers the review
    /// step before any write.
    #[test]
    fn the_record_editor_states_the_revision_and_offers_review() {
        let html = render(ModalKind::RecordEditor);
        assert!(html.contains("Edit record users/u1, revision 18"), "{html}");
        assert!(html.contains("Review changes"), "{html}");
        assert!(html.contains("Revision check"), "{html}");
        assert!(html.contains("Save as draft"), "{html}");
    }

    #[test]
    fn an_untouched_edit_cannot_be_reviewed() {
        let html = render(ModalKind::RecordEditor);
        let at = html.find("Review changes").expect("button");
        let button = &html[html[..at].rfind("<button").expect("tag")..at];
        assert!(button.contains("disabled"), "{button}");
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
    fn delete_button_starts_disabled_and_names_the_record() {
        let html = render(ModalKind::ConfirmDelete);
        assert!(html.contains("Delete users/u1"), "{html}");
        let at = html.find("Delete permanently").expect("button");
        let button = &html[html[..at].rfind("<button").expect("tag")..at];
        assert!(button.contains("disabled"), "{button}");
    }
}
