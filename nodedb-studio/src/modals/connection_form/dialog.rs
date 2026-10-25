//! The New / Edit connection dialog.
//!
//! Test, Save, and Save & connect are three different acts. Test reaches
//! the server and persists nothing. Save writes the entry, and the password
//! only to the keychain when the box is checked. Save & connect saves, then
//! opens the session.
//!
//! The save and test tasks outlive the dialog (`spawn_forever`), so they
//! write only root state: the form, the registry, the error bar. A signal
//! owned by the dialog is gone once it closes.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::connector::{Connector, use_connector};
use crate::components::modal::Modal;
use crate::modals::connection_form::fields::{
    AuthFields, LiteFields, NameAndTarget, OriginFields, TlsFields,
};
use crate::modals::connection_form::test_result::TestOutcome;
use crate::models::connections::ConnectionKind;
use crate::services::backend::Backend;
use crate::state::connection::ConnectError;
use crate::state::connection_form::{ConnectionForm, Purpose, TestState};
use crate::state::connections_registry::{Credentials, SavedConnection};
use crate::state::ui::{ManagerSelection, ModalKind};

/// The names a new entry must not take: every saved name except the one
/// being edited.
fn taken_names(registry: &[SavedConnection], editing: Option<&str>) -> Vec<String> {
    registry
        .iter()
        .map(|c| c.name.clone())
        .filter(|n| Some(n.as_str()) != editing)
        .collect()
}

/// The identity a Save & connect sends: the typed user name, else the saved
/// entry's. The typed secret goes along, since an unchecked keychain box
/// leaves it nowhere else.
fn connect_credentials(typed: Credentials, saved: &SavedConnection) -> Credentials {
    let username = if typed.username.trim().is_empty() {
        saved.credentials().username
    } else {
        typed.username
    };
    Credentials {
        username,
        password: typed.password,
    }
}

#[component]
pub fn ConnectionDialog() -> Element {
    let mut form = use_context::<Signal<ConnectionForm>>();
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let connector = use_connector();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let env = SaveEnv {
        form,
        modal,
        registry,
        selection: use_context(),
        error: use_context(),
    };
    // The secret must not outlive the dialog.
    use_drop(move || form.set(ConnectionForm::blank()));

    let f = form.read().clone();
    let taken = taken_names(&registry.read(), f.editing.as_deref());
    let errors = f.visible_errors(Purpose::Save, &taken);

    let run_test = {
        let backend = backend.clone();
        let taken = taken.clone();
        move |_| {
            form.write().attempted = true;
            let (draft, creds) = {
                let f = form.peek();
                match f.draft(Purpose::Test, &taken) {
                    Ok(draft) => (draft, f.credentials()),
                    Err(_) => return,
                }
            };
            form.write().test = TestState::Running;
            let backend = backend.clone();
            spawn_forever(async move {
                let result = backend.test_connection(&draft, &creds).await;
                form.write().test = match result {
                    Ok(found) => TestState::Passed(found),
                    Err(e) => TestState::Failed(e.to_string()),
                };
            });
        }
    };

    let mut save = {
        let backend = backend.clone();
        let taken = taken.clone();
        let connector = connector.clone();
        move |connect: bool| {
            form.write().attempted = true;
            let (draft, creds, replacing) = {
                let f = form.peek();
                match f.draft(Purpose::Save, &taken) {
                    Ok(draft) => (draft, f.credentials(), f.editing.clone()),
                    Err(_) => return,
                }
            };
            form.write().busy = true;
            let backend = backend.clone();
            let connector = connector.clone();
            spawn_forever(async move {
                save_task(env, backend, connector, draft, creds, replacing, connect).await;
            });
        }
    };

    rsx! {
        Modal {
            title: f.title().to_string(),
            width: 680,
            dismiss_on_scrim: false,
            class: "connection-dialog",
            div { class: "dialog-body",
                NameAndTarget { errors: errors.clone() }
                match f.kind {
                    ConnectionKind::Origin => rsx! {
                        OriginFields { errors: errors.clone() }
                        AuthFields { errors: errors.clone() }
                        TlsFields {}
                    },
                    ConnectionKind::Lite => rsx! { LiteFields { errors: errors.clone() } },
                }
                div { class: "test-row",
                    button {
                        class: "btn",
                        r#type: "button",
                        disabled: f.test == TestState::Running || f.busy,
                        onclick: run_test,
                        "Test connection"
                    }
                    TestOutcome { test: f.test.clone() }
                }
            }
            footer { class: "dialog-footer",
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: move |_| modal.set(None),
                    "Cancel"
                }
                button {
                    class: "btn",
                    r#type: "button",
                    disabled: f.busy,
                    onclick: {
                        let mut save = save.clone();
                        move |_| save(false)
                    },
                    "Save"
                }
                button {
                    class: "btn primary",
                    r#type: "button",
                    disabled: f.busy,
                    onclick: move |_| save(true),
                    "Save & connect"
                }
            }
        }
    }
}

/// The root signals a save task writes. Captured at render: a task spawned
/// with `spawn_forever` runs outside every component, so it cannot read
/// context.
#[derive(Clone, Copy)]
struct SaveEnv {
    form: Signal<ConnectionForm>,
    modal: Signal<Option<ModalKind>>,
    registry: Signal<Vec<SavedConnection>>,
    selection: Signal<ManagerSelection>,
    error: Signal<ConnectError>,
}

/// Save, refresh the registry, and optionally connect. Writes root state
/// only. An error shows in the error bar and leaves the dialog open, so the
/// field that caused it can change.
async fn save_task(
    env: SaveEnv,
    backend: Rc<dyn Backend>,
    connector: Connector,
    draft: crate::models::connections::ConnectionDraft,
    creds: Credentials,
    replacing: Option<String>,
    connect: bool,
) {
    let SaveEnv {
        mut form,
        mut modal,
        mut registry,
        mut selection,
        mut error,
    } = env;
    let saved = backend
        .save_connection(draft, creds.clone(), replacing)
        .await;
    match saved {
        Ok(saved) => {
            if let Ok(list) = backend.list_connections().await {
                registry.set(list);
            }
            selection.set(ManagerSelection(Some(saved.name.clone())));
            if !connect {
                modal.set(None);
                return;
            }
            // The entry exists now: a retry after a failed connect edits it
            // and does not add a second one.
            {
                let mut f = form.write();
                f.editing = Some(saved.name.clone());
                f.busy = false;
            }
            if connector
                .run(&saved.name, &connect_credentials(creds, &saved))
                .await
            {
                modal.set(None);
            }
        }
        Err(e) => {
            form.write().busy = false;
            error.set(ConnectError(Some(e)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::connection_service::ConnectionService;

    async fn registry() -> Vec<SavedConnection> {
        MockConnectionService::ready()
            .list_connections()
            .await
            .expect("mock registry")
    }

    #[tokio::test]
    async fn an_edit_may_keep_its_own_name() {
        let saved = registry().await;
        let taken = taken_names(&saved, Some("staging-cluster"));
        assert!(!taken.contains(&"staging-cluster".to_string()));
        assert!(taken.contains(&"Production".to_string()));
        assert_eq!(taken_names(&saved, None).len(), saved.len());
    }

    #[tokio::test]
    async fn connect_uses_the_typed_user_then_the_saved_one() {
        let saved = registry()
            .await
            .into_iter()
            .find(|c| c.name == "staging-cluster")
            .expect("staging");
        let typed = Credentials {
            username: "hana".into(),
            password: Some("pw".into()),
        };
        let out = connect_credentials(typed, &saved);
        assert_eq!(out.username, "hana");
        assert_eq!(out.password.as_deref(), Some("pw"));
        let blank = Credentials::default();
        assert_eq!(connect_credentials(blank, &saved).username, "admin");
    }
}
