//! Left navigation rail.
//!
//! Capability-driven: items carrying a `cap` get the `cap-hidden` class when
//! the active connection lacks that capability (mirrors the mockup's
//! `data-cap` + `applyCapabilities`). The active item is derived from the
//! current route's nav key, so e.g. any `/streams/*` keeps "Streams" lit.
//! Badge counts come from the seam's `nav_badges()`.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::models::shell::NavBadges;
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::connection::{ActiveConnection, Capability};

/// The badge text for each counted item, or `None` for no badge.
///
/// Zero shows no badge, as the mockup does for an idle item. A load still in
/// flight or a failed read also shows none: a badge is a hint, not a
/// screen, and the screens it points at report their own load errors. A
/// count in its place would be a number the server never sent.
fn badge_texts(
    badges: Option<&Result<NavBadges, StudioError>>,
) -> (Option<String>, Option<String>) {
    let text = |n: u32| (n > 0).then(|| n.to_string());
    match badges {
        Some(Ok(b)) => (text(b.query), text(b.streams)),
        Some(Err(_)) | None => (None, None),
    }
}

#[component]
pub fn Rail() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let badges = use_resource(move || {
        let backend = backend.clone();
        // Counts belong to the connection: reading it re-runs this on a switch.
        let _conn = active.read().as_ref().map(|c| c.name.clone());
        async move { backend.nav_badges().await }
    });
    let (query_badge, streams_badge) = badge_texts(badges.read().as_ref());

    rsx! {
        aside { class: "rail",
            div { class: "rail-brand",
                div { class: "logo", "N" }
                div { class: "name", "NodeDB " span { "Studio" } }
            }

            div { class: "rail-section",
                div { class: "rail-section-label", "Workspace" }
                RailItem { route: Route::Explorer {}, label: "Explorer",
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        path { d: "M2 3h12v3H2zM2 7h12v3H2zM2 11h12v3H2z" }
                    }
                }
                RailItem { route: Route::Query {}, label: "Query", badge: query_badge,
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        path { d: "M3 4l2 2-2 2M7 8h6" }
                    }
                }
                RailItem { route: Route::Designer {}, label: "Designer",
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        rect { x: "2", y: "3", width: "5", height: "4" }
                        rect { x: "9", y: "3", width: "5", height: "4" }
                        rect { x: "2", y: "9", width: "5", height: "4" }
                        rect { x: "9", y: "9", width: "5", height: "4" }
                        path { d: "M7 5h2M7 11h2M4.5 7v2M11.5 7v2" }
                    }
                }
            }

            div { class: "rail-section",
                div { class: "rail-section-label", "Tools" }
                RailItem { route: Route::GraphExplorer {}, label: "Graph Explorer", cap: Capability::Graph,
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        circle { cx: "4", cy: "4", r: "2" }
                        circle { cx: "12", cy: "4", r: "2" }
                        circle { cx: "8", cy: "12", r: "2" }
                        path { d: "M5.5 5.5L7 10.5M10.5 5.5L9 10.5M6 4h4" }
                    }
                }
                RailItem { route: Route::VectorSpace {}, label: "Vector Space", cap: Capability::Vector,
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        circle { cx: "3", cy: "5", r: "1" }
                        circle { cx: "6", cy: "9", r: "1" }
                        circle { cx: "10", cy: "4", r: "1" }
                        circle { cx: "11", cy: "11", r: "1" }
                        circle { cx: "13", cy: "7", r: "1" }
                    }
                }
                RailItem { route: Route::Streams { tab: "landing".to_string() }, label: "Streams & Events", cap: Capability::Streams, badge: streams_badge,
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        path { d: "M2 5c2 0 2 2 4 2s2-2 4-2 2 2 4 2M2 10c2 0 2 2 4 2s2-2 4-2 2 2 4 2" }
                    }
                }
                RailItem { route: Route::TimeseriesDashboard {}, label: "Timeseries", cap: Capability::Timeseries,
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        path { d: "M2 12l3-4 3 2 3-5 3 3" }
                    }
                }
                RailItem { route: Route::SpatialView {}, label: "Spatial", cap: Capability::Spatial,
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        path { d: "M2 4l4-1 4 1 4-1v10l-4 1-4-1-4 1zM6 3v11M10 4v11" }
                    }
                }
                RailItem { route: Route::FtsInspector {}, label: "FTS Inspector", cap: Capability::Fts,
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        circle { cx: "7", cy: "7", r: "4" }
                        path { d: "M10 10l3 3" }
                    }
                }
            }

            div { class: "rail-section",
                div { class: "rail-section-label", "System" }
                RailItem { route: Route::Sync {}, label: "Sync", cap: Capability::Sync,
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        path { d: "M3 8a5 5 0 0 1 9-3M13 8a5 5 0 0 1-9 3M11 3v3h-3M5 13v-3h3" }
                    }
                }
                RailItem { route: Route::Admin { tab: "cluster".to_string() }, label: "Admin",
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        circle { cx: "8", cy: "6", r: "2" }
                        path { d: "M3 13c0-2.5 2-4 5-4s5 1.5 5 4" }
                    }
                }
                RailItem { route: Route::Console {}, label: "Console",
                    svg { class: "icon", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                        rect { x: "2", y: "3", width: "12", height: "10", rx: "1" }
                        path { d: "M5 7l2 1.5L5 10M9 10h3" }
                    }
                }
            }

            div { class: "rail-spacer" }
        }
    }
}

#[component]
fn RailItem(
    route: Route,
    label: String,
    cap: Option<Capability>,
    badge: Option<String>,
    children: Element,
) -> Element {
    let nav = use_navigator();
    let current = use_route::<Route>();
    let active_conn = use_context::<Signal<Option<ActiveConnection>>>();

    // Hidden when the item gates on a capability the active connection lacks.
    let hidden = match cap {
        Some(c) => active_conn
            .read()
            .as_ref()
            .map(|conn| !conn.capabilities.has(c))
            .unwrap_or(true),
        None => false,
    };
    let is_active = current.nav_key() == route.nav_key();

    let mut class = String::from("rail-item");
    if is_active {
        class.push_str(" active");
    }
    if hidden {
        class.push_str(" cap-hidden");
    }

    rsx! {
        div {
            class: "{class}",
            onclick: move |_| {
                if !hidden {
                    nav.push(route.clone());
                }
            },
            {children}
            "{label}"
            if let Some(b) = badge {
                span { class: "badge", "{b}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_render_as_badges() {
        let b = Ok(NavBadges {
            query: 3,
            streams: 6,
        });
        assert_eq!(
            badge_texts(Some(&b)),
            (Some("3".to_string()), Some("6".to_string()))
        );
    }

    #[test]
    fn zero_shows_no_badge() {
        let b = Ok(NavBadges {
            query: 0,
            streams: 2,
        });
        assert_eq!(badge_texts(Some(&b)), (None, Some("2".to_string())));
    }

    /// A failed or pending read must not invent a count.
    #[test]
    fn loading_and_error_show_no_badge() {
        assert_eq!(badge_texts(None), (None, None));
        assert_eq!(
            badge_texts(Some(&Err(StudioError::NotConnected))),
            (None, None)
        );
    }
}
