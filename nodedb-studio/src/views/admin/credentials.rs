//! Administration → Credentials: the aliases the server uses to reach
//! providers, and the editor of the chosen one.
//!
//! Two panels. A row holds a reference to a secret, never the secret. A
//! credential never appears in a saved query or an exported log, and a
//! remote provider is called only when someone presses Test connection:
//! nothing calls it on its own.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::admin_credential::CredentialDraft;
use crate::models::admin_credential::{Credential, CredentialList};
use crate::routes::tabs::AdminTab;
use crate::services::backend::Backend;
use crate::state::admin::{AdminUi, NEW_CREDENTIAL, pick};
use crate::state::workspace::Surface;
use crate::views::admin::common::{Cut, ReviewSheet};
use crate::views::admin::cred_edit::CredentialEditor;
use crate::views::admin::tasks;
use crate::views::events::common::{NoticeLine, StatusText};

const SURFACE: Surface = Surface::Admin(AdminTab::Credentials);
const EDIT_SIZE: PanelSize = PanelSize::width(480.0, 420.0, 760.0);

pub fn chosen<'a>(l: &'a CredentialList, alias: Option<&str>) -> Option<&'a Credential> {
    pick(&l.items, alias, |c| &c.alias)
}

#[component]
pub fn CredentialsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<AdminUi>().credentials;
    let epoch = use_memo(move || ui.read().epoch);

    let mut list = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch();
            async move { backend.credentials().await }
        }
    });
    let state = resource_state(&list);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let needle = u.filter.trim().to_lowercase();
    let new_draft = (u.selected.as_deref() == Some(NEW_CREDENTIAL))
        .then(|| u.creating.clone())
        .flatten();
    let selected = data
        .as_ref()
        .and_then(|l| chosen(l, u.selected.as_deref()).cloned());

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                Panel {
                    surface: SURFACE,
                    id: "credentials",
                    title: "Credentials",
                    class: "fill",
                    NoticeLine {
                        notice: u.notice.clone(),
                        on_dismiss: move |_| ui.write().notice = None,
                    }
                    div { class: "toolbar nowrap cl-filters",
                        div { class: "search-field grow",
                            Ico { icon: Icon::Search, size: 16 }
                            input {
                                class: "input",
                                placeholder: "Search credentials…",
                                "aria-label": "Search credentials",
                                value: "{u.filter}",
                                oninput: move |e| ui.write().filter = e.value(),
                            }
                        }
                        button {
                            class: "btn primary",
                            title: "Create a credential alias",
                            onclick: move |_| {
                                let mut w = ui.write();
                                w.creating = Some(CredentialDraft::blank());
                                w.selected = Some(NEW_CREDENTIAL.to_string());
                                w.test = crate::state::admin::CredTestRun::Idle;
                            },
                            Ico { icon: Icon::Plus, size: 16 }
                            "Add credential"
                        }
                    }
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| list.restart(),
                        empty_message: "No credential alias exists. The server calls no provider.",
                    }
                    if let Some(l) = data.as_ref() {
                        div { class: "dt-wrap",
                            table { class: "dt cl-table ad-table ad-creds",
                                thead {
                                    tr {
                                        th { "Alias" }
                                        th { "Provider" }
                                        th { "Status" }
                                        th { "Scope" }
                                    }
                                }
                                tbody {
                                    if let Some(d) = new_draft.as_ref() {
                                        tr {
                                            key: "{NEW_CREDENTIAL}",
                                            class: "clickable selected",
                                            "aria-selected": "true",
                                            td { class: "key",
                                                span { class: "ad-name",
                                                    span { class: "unsaved-dot", title: "Not created yet" }
                                                    Cut { text: if d.alias.trim().is_empty() { "New credential".to_string() } else { d.alias.trim().to_string() } }
                                                }
                                            }
                                            td { "{d.kind.label()}" }
                                            td { span { class: "badge amber", "Not created" } }
                                            td { "analytics" }
                                        }
                                    }
                                    for c in l.items.iter().filter(|c| c.matches(&needle)) {
                                        {
                                            let alias = c.alias.clone();
                                            let is_selected = new_draft.is_none() && selected.as_ref().is_some_and(|s| s.alias == c.alias);
                                            let dirty = u.draft_of(&c.alias).is_some_and(|d| d.differs_from(c));
                                            rsx! {
                                                tr {
                                                    key: "{c.alias}",
                                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                                    "aria-selected": "{is_selected}",
                                                    tabindex: "0",
                                                    onclick: move |_| {
                                                        let mut w = ui.write();
                                                        w.selected = Some(alias.clone());
                                                        w.test = crate::state::admin::CredTestRun::Idle;
                                                    },
                                                    td { class: "key",
                                                        span { class: "ad-name",
                                                            if dirty {
                                                                span { class: "unsaved-dot", title: "Unsaved edits" }
                                                            }
                                                            Cut { text: c.alias.clone() }
                                                        }
                                                    }
                                                    td { "{c.kind.label()}" }
                                                    td { StatusText { tone: c.status.tone(), text: c.status.label().to_string() } }
                                                    td { Cut { text: c.scope.clone() } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "dt-footer", "{l.items.len()} credentials · Rows hold secret references only." }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "edit", size: EDIT_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "edit",
                    title: "Edit credential",
                    size: EDIT_SIZE,
                    if let (Some(_), Some(l)) = (new_draft.clone(), data.clone()) {
                        CredentialEditor { key: "{NEW_CREDENTIAL}", credential: None, list: l }
                    } else if let (Some(c), Some(l)) = (selected.clone(), data.clone()) {
                        CredentialEditor { key: "{c.alias}", credential: Some(c), list: l }
                    }
                }
                if let Some(state) = u.review.clone() {
                    ReviewSheet {
                        state,
                        back_label: "Back to editor",
                        on_back: move |_| ui.write().review = None,
                        on_apply: {
                            let backend = backend.clone();
                            move |_| {
                                let backend = backend.clone();
                                spawn_forever(async move { tasks::apply_review(ui, backend).await });
                            }
                        },
                    }
                }
            }
        }
    }
}
