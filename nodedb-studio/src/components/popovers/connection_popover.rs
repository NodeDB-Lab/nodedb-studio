//! Connection switch popover: current connection, a switch list of the others
//! (self excluded), edit, and disconnect.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::connection_service::ConnectionService;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::{ConnStatus, SavedConnection};
use crate::state::ui::{ModalKind, Popover};

#[component]
pub fn ConnectionPopover() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let service = use_context::<Rc<dyn ConnectionService>>();

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
                    rsx! {
                        div {
                            class: "{item_class}",
                            onclick: move |_| {
                                if !disabled {
                                    // Async at the seam: clone svc + name into the task,
                                    // set `active` (Copy) only after the await resolves.
                                    let svc = svc.clone();
                                    let name = name.clone();
                                    spawn(async move {
                                        if let Ok(s) = svc.connect(&name).await { active.set(Some(s)); }
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
