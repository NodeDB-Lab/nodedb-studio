//! Command palette (Cmd+K). Opens only while connected.
//!
//! Rendered inside the router (via `StudioLayout`) so navigation items can use
//! the navigator. Open state is the shared `Signal<bool>` provided by `Studio`.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::services::connection_service::ConnectionService;
use crate::state::connection::ActiveConnection;
use crate::state::ui::ModalKind;

#[component]
pub fn CommandPalette() -> Element {
    let mut open = use_context::<Signal<bool>>();
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let service = use_context::<std::rc::Rc<dyn ConnectionService>>();
    let nav = use_navigator();

    if !*open.read() {
        return rsx! {};
    }

    // Switch connection by name, then close. `service` is an Rc (not Copy), so
    // each switch handler clones it.
    let switch_svc = service.clone();

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
                            move |_| {
                                if let Some(s) = svc.connect("staging-cluster") { active.set(Some(s)); }
                                open.set(false);
                            }
                        },
                        "Switch to staging-cluster"
                    }
                    div { class: "palette-item", onclick: {
                            let svc = switch_svc.clone();
                            move |_| {
                                if let Some(s) = svc.connect("prod-replica-eu") { active.set(Some(s)); }
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
