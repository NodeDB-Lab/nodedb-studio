//! Command palette (Cmd+K). Opens only while connected.
//!
//! Rendered inside the router (via `StudioLayout`) so navigation items can use
//! the navigator. Open state is the shared `Signal<bool>` provided by `App`.

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, ConnectError, apply_connect};
use crate::state::connections_registry::{Credentials, SavedConnection};
use crate::state::ui::{ModalKind, PrefsPane};

/// The palette's "Switch to …" targets: every saved connection that can be
/// opened, except the one already active, in registry order, each with the
/// credentials to send.
///
/// A saved entry's stored profile IS its explicit username; the palette
/// switches between entries that already have one, so there is no field to
/// type into here. An entry with no profile yields a blank, which the seam
/// rejects with MissingUsername and the app root renders: never a silent
/// fallback to `admin`.
fn switch_targets(
    registry: &[SavedConnection],
    current: Option<&str>,
) -> Vec<(String, Credentials)> {
    registry
        .iter()
        .filter(|c| c.status.is_connectable() && Some(c.name.as_str()) != current)
        .map(|c| {
            let creds = Credentials {
                username: c
                    .profile
                    .as_ref()
                    .map(|p| p.user.clone())
                    .unwrap_or_default(),
                password: None,
            };
            (c.name.clone(), creds)
        })
        .collect()
}

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

    // .read(), not .peek(): this runs at render time. peek() would freeze the
    // list at the render where the palette opened, so a registry that resolves
    // later (a real backend awaiting the network) would never show its entries.
    let current = active.read().as_ref().map(|c| c.name.clone());
    let targets = switch_targets(&registry.read(), current.as_deref());

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
                    div { class: "palette-item", onclick: move |_| { modal.set(Some(ModalKind::Preferences(PrefsPane::Appearance))); open.set(false); },
                        "Open preferences" span { class: "meta", "⌘," }
                    }
                    div { class: "palette-item", onclick: move |_| { modal.set(Some(ModalKind::Preferences(PrefsPane::Appearance))); open.set(false); },
                        "Toggle theme" span { class: "meta", "⌘⇧L" }
                    }

                    div { class: "palette-section", "Connections" }
                    for (name, creds) in targets {
                        div { key: "{name}", class: "palette-item", onclick: {
                                let svc = service.clone();
                                let name = name.clone();
                                move |_| {
                                    let svc = svc.clone();
                                    let name = name.clone();
                                    let creds = creds.clone();
                                    connect_error.set(ConnectError(None));
                                    // spawn_forever: the palette closes on the next
                                    // line, and a scope-bound task would be dropped.
                                    spawn_forever(async move {
                                        let result = svc.connect(&name, &creds).await;
                                        let err = apply_connect(&mut active.write(), result);
                                        connect_error.set(ConnectError(err));
                                    });
                                    open.set(false);
                                }
                            },
                            "Switch to {name}"
                        }
                    }
                    div { class: "palette-item", onclick: move |_| { active.set(None); open.set(false); },
                        "Disconnect" span { class: "meta", "⌘D" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::connection_service::{ConnectionService, MockConnectionService};

    /// The saved-connection registry, read through the seam as the app does.
    async fn registry() -> Vec<SavedConnection> {
        MockConnectionService::ready()
            .list_connections()
            .await
            .expect("mock registry")
    }

    fn names(targets: &[(String, Credentials)]) -> Vec<&str> {
        targets.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// Matches the mockup while on local-nodedb-dev: the other two reachable
    /// connections, not the offline one, not the current one.
    #[tokio::test]
    async fn lists_reachable_connections_except_the_current_one() {
        let t = switch_targets(&registry().await, Some("local-nodedb-dev"));
        assert_eq!(names(&t), ["staging-cluster", "prod-replica-eu"]);
    }

    #[tokio::test]
    async fn each_target_carries_its_own_username() {
        let t = switch_targets(&registry().await, Some("local-nodedb-dev"));
        assert!(t.iter().all(|(_, c)| !c.username.trim().is_empty()));
    }

    /// The registry loads asynchronously; until it does there is nothing to
    /// offer rather than a stale hardcoded list.
    #[test]
    fn empty_registry_offers_nothing() {
        assert!(switch_targets(&[], Some("local-nodedb-dev")).is_empty());
    }
}
