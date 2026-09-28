//! Connection switch popover: current connection, a switch list of the others
//! (self excluded), edit, and disconnect.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, ConnectError, apply_connect};
use crate::state::connections_registry::{ConnStatus, Credentials, SavedConnection};
use crate::state::ui::{ModalKind, Popover};

#[component]
pub fn ConnectionPopover() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let service = use_context::<Rc<dyn Backend>>();
    // The popover closes on click (see `popover.set(None)` below), so a failure
    // it owned would never render; the surface lives at the app root instead.
    let mut connect_error = use_context::<Signal<ConnectError>>();

    let conn = active.read();
    let Some(c) = conn.as_ref() else {
        return rsx! {};
    };
    let current_name = c.name.clone();
    let current_sub = c.sub.clone();

    // Switch list excludes the current connection.
    let others: Vec<SavedConnection> = registry
        .read()
        .iter()
        .filter(|s| s.name != current_name)
        .cloned()
        .collect();

    rsx! {
        div { class: "conn-popover open", onclick: move |e| e.stop_propagation(),
            div { class: "cp-current",
                div { class: "name", span { class: "dot" } span { "{current_name}" } }
                div { class: "sub", "{current_sub}" }
            }
            div { class: "cp-section", "Switch to" }
            for sc in others {
                {
                    let name = sc.name.clone();
                    let (dot_class, meta, disabled) = match sc.status {
                        ConnStatus::Online => ("ok", sc.ping.clone().unwrap_or_default(), false),
                        ConnStatus::ReadOnly => ("warn", "read-only".to_string(), false),
                        ConnStatus::Offline => ("off", "offline".to_string(), true),
                    };
                    let svc = service.clone();
                    let item_class = if disabled { "cp-item disabled" } else { "cp-item" };
                    // The stored profile IS this entry's explicit username; the
                    // popover only switches between already-saved connections. A
                    // profile-less entry yields a blank, which the seam rejects
                    // with MissingUsername and the app root renders.
                    let creds = Credentials {
                        username: sc
                            .profile
                            .as_ref()
                            .map(|p| p.user.clone())
                            .unwrap_or_default(),
                        password: None,
                    };
                    rsx! {
                        div {
                            class: "{item_class}",
                            onclick: move |_| {
                                if !disabled {
                                    // Async at the seam: clone svc + name into the task,
                                    // set `active` (Copy) only after the await resolves.
                                    let svc = svc.clone();
                                    let name = name.clone();
                                    let creds = creds.clone();
                                    connect_error.set(ConnectError(None));
                                    // spawn_forever, not spawn: this popover is
                                    // conditionally mounted and closes on the next
                                    // line, and Dioxus drops a scope's tasks when
                                    // the scope goes away. A plain spawn dies at
                                    // the await against any backend that actually
                                    // yields, leaving no session and no error.
                                    spawn_forever(async move {
                                        let result = svc.connect(&name, &creds).await;
                                        let err = apply_connect(&mut active.write(), result);
                                        connect_error.set(ConnectError(err));
                                    });
                                    popover.set(None);
                                }
                            },
                            span { class: "dot {dot_class}" }
                            " {sc.name} "
                            span { class: "meta", "{meta}" }
                        }
                    }
                }
            }
            div { class: "cp-divider" }
            div {
                class: "cp-action",
                onclick: move |_| { popover.set(None); modal.set(Some(ModalKind::NewConnection)); },
                "Edit connection…"
            }
            div {
                class: "cp-action danger",
                onclick: move |_| active.set(None),
                "Disconnect " span { class: "kbd", "⌘D" }
            }
        }
    }
}
