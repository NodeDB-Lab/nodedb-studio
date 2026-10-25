//! The details of one audit event. The event is immutable: Studio shows
//! what the server stored and offers no edit. The permission diff and the
//! request origin are the whole payload. No secret or token appears.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::models::format::date_second;
use crate::routes::Route;
use crate::routes::tabs::AdminTab;
use crate::services::backend::Backend;
use crate::state::admin::{AccessMode, AccessUi, AdminUi};
use crate::views::admin::common::Cut;
use crate::views::events::common::StatusText;

/// Open the principal an event acted on in Access. Names resolve to ids
/// through the directory, because an id never changes and a name can.
async fn open_principal(backend: Rc<dyn Backend>, mut ui: Signal<AccessUi>, name: String) {
    if let Ok(dir) = backend.access_directory().await
        && let Some(p) = dir.principals.iter().find(|p| p.name == name)
    {
        let mut w = ui.write();
        w.mode = AccessMode::Principals;
        w.principal = Some(p.id.clone());
    }
}

#[component]
fn Diff(title: String, lines: Vec<String>) -> Element {
    rsx! {
        h4 { class: "inspector-section sub ad-diff-title", "{title}" }
        pre { class: "code-block ad-diff", tabindex: "0",
            for (i , l) in lines.iter().enumerate() {
                div { key: "{i}", "{l}" }
            }
        }
    }
}

#[component]
pub fn EventDetails(id: String) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let nav = use_navigator();
    let access = use_context::<AdminUi>().access;
    let mut event = use_resource({
        let backend = backend.clone();
        let id = id.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let id = id.clone();
            async move { backend.audit_event(&id).await }
        }
    });
    let state = resource_state(&event);
    let Some(e) = state.loaded().cloned() else {
        return rsx! {
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| event.restart(),
            }
        };
    };
    let principal = e.principal_name().map(str::to_string);
    let open = {
        let principal = principal.clone();
        move |_| {
            let (backend, name) = (backend.clone(), principal.clone().unwrap_or_default());
            spawn_forever(async move { open_principal(backend, access, name).await });
            nav.push(Route::Admin {
                tab: AdminTab::Access,
            });
        }
    };
    rsx! {
        div { class: "inspector ad-event",
            div { class: "kv",
                div { class: "kv-row", span { "Event ID" } span { class: "mono", "{e.id}" } }
                div { class: "kv-row", span { "Timestamp (UTC)" } span { "{date_second(e.at, 0)}" } }
                div { class: "kv-row", span { "Actor" } Cut { text: e.actor.clone() } }
                div { class: "kv-row", span { "Action" } span { "{e.action}" } }
                div { class: "kv-row", span { "Target type" } span { "{e.target_type.label()}" } }
                div { class: "kv-row",
                    span { "Target" }
                    if principal.is_some() {
                        button { class: "link-btn", title: "Open this principal in Access", onclick: open,
                            Cut { text: e.target.clone() }
                            Ico { icon: Icon::ExternalLink, size: 16 }
                        }
                    } else {
                        Cut { text: e.target.clone() }
                    }
                }
                div { class: "kv-row", span { "Database" } span { "{e.database}" } }
                div { class: "kv-row",
                    span { "Result" }
                    StatusText { tone: e.result.tone(), text: e.result.label().to_string() }
                }
                div { class: "kv-row", span { "Correlation ID" } span { class: "mono", "{e.correlation}" } }
            }
            Diff { title: "Before (permissions)".to_string(), lines: e.before.clone() }
            Diff { title: "After (permissions)".to_string(), lines: e.after.clone() }
            h4 { class: "inspector-section", "Request origin" }
            div { class: "kv",
                div { class: "kv-row", span { "IP address" } span { class: "mono", "{e.origin.ip}" } }
                div { class: "kv-row", span { "Client" } Cut { text: e.origin.client.clone() } }
                div { class: "kv-row", span { "Method" } span { "{e.origin.method}" } }
            }
            p { class: "inspector-note",
                "An audit event is append-only. It holds no secret and no token."
            }
        }
    }
}
