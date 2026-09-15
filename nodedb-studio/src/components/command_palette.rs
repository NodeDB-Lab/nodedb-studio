//! Command palette (Cmd+K). Opens only while connected.
//!
//! Rendered inside the router (via `StudioLayout`) so navigation items can use
//! the navigator. Open state is the shared `Signal<bool>` provided by `Studio`.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, ConnectError, apply_connect};
use crate::state::connections_registry::{Credentials, SavedConnection};
use crate::state::ui::ModalKind;

#[component]
pub fn CommandPalette() -> Element {
    let mut open = use_context::<Signal<bool>>();
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    // The palette closes on click, so a failure it owned would never render;
    // the surface lives at the app root instead.
    let mut connect_error = use_context::<Signal<ConnectError>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let service = use_context::<std::rc::Rc<dyn Backend>>();
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let nav = use_navigator();

    if !*open.read() {
        return rsx! {};
    }

    // Switch connection by name, then close. `service` is an Rc (not Copy), so
    // each switch handler clones it.
    let switch_svc = service.clone();

    // A saved entry's stored profile IS its explicit username; the palette
    // switches between entries that already have one, so there is no field to
    // type into here. An entry with no profile yields a blank, which the seam
    // rejects with MissingUsername and the app root renders — never a silent
    // fallback to `admin`. The connections fixture carries the invariant
    // that keeps connectable entries from reaching that state.
    let creds_for = |name: &str| -> Credentials {
        Credentials {
            username: registry
                .peek()
                .iter()
                .find(|c| c.name == name)
                .and_then(|c| c.profile.as_ref())
                .map(|p| p.user.clone())
                .unwrap_or_default(),
            password: None,
        }
    };

    rsx! {
        div {
            class: "palette-overlay open",
            onclick: move |_| open.set(false),
            div {
                class: "palette",
                onclick: move |e| e.stop_propagation(),
                input { placeholder: "Search or run command…" }
                div { class: "palette-results",
                    div { class: "palette-section", "Navigate" }
                    div { class: "palette-item", onclick: move |_| { nav.push(Route::Explorer {}); open.set(false); },
                        "Open Explorer" span { class: "meta", "G E" }
                    }
                    div { class: "palette-item", onclick: move |_| { nav.push(Route::Query {}); open.set(false); },
                        "Open Query" span { class: "meta", "G Q" }
                    }
                    div { class: "palette-item", onclick: move |_| { nav.push(Route::GraphExplorer {}); open.set(false); },
                        "Open Graph Explorer" span { class: "meta", "G G" }
                    }
                    div { class: "palette-item", onclick: move |_| { nav.push(Route::Streams { tab: "landing".to_string() }); open.set(false); },
                        "Open Streams & Events" span { class: "meta", "G S" }
                    }

                    div { class: "palette-section", "Actions" }
                    div { class: "palette-item", onclick: move |_| { modal.set(Some(ModalKind::NewConnection)); open.set(false); },
                        "New connection…" span { class: "meta", "⌘N" }
                    }
                    div { class: "palette-item",
                        "Run current query" span { class: "meta", "⌘↵" }
                    }
                    div { class: "palette-item", onclick: move |_| { modal.set(Some(ModalKind::Preferences)); open.set(false); },
                        "Open preferences" span { class: "meta", "⌘," }
                    }
                    div { class: "palette-item", onclick: move |_| { modal.set(Some(ModalKind::Preferences)); open.set(false); },
                        "Toggle theme" span { class: "meta", "⌘⇧L" }
                    }

                    div { class: "palette-section", "Connections" }
                    div { class: "palette-item", onclick: {
                            let svc = switch_svc.clone();
                            let creds = creds_for("staging-cluster");
                            move |_| {
                                let svc = svc.clone();
                                let creds = creds.clone();
                                connect_error.set(ConnectError(None));
                                spawn(async move {
                                    let result = svc.connect("staging-cluster", &creds).await;
                                    let err = apply_connect(&mut active.write(), result);
                                    connect_error.set(ConnectError(err));
                                });
                                open.set(false);
                            }
                        },
                        "Switch to staging-cluster"
                    }
                    div { class: "palette-item", onclick: {
                            let svc = switch_svc.clone();
                            let creds = creds_for("prod-replica-eu");
                            move |_| {
                                let svc = svc.clone();
                                let creds = creds.clone();
                                connect_error.set(ConnectError(None));
                                spawn(async move {
                                    let result = svc.connect("prod-replica-eu", &creds).await;
                                    let err = apply_connect(&mut active.write(), result);
                                    connect_error.set(ConnectError(err));
                                });
                                open.set(false);
                            }
                        },
                        "Switch to prod-replica-eu"
                    }
                    div { class: "palette-item", onclick: move |_| { active.set(None); open.set(false); },
                        "Disconnect" span { class: "meta", "⌘D" }
                    }
                }
            }
        }
    }
}
