//! The scenes a run captures, one PNG each per theme.
//!
//! Every studio scene opens `STUDIO_CONNECTION` through the backend seam. The
//! data on screen is whatever that backend serves, so the scene list does
//! not change when the backend does.

use crate::models::collection::StorageMode;
use crate::routes::Route;
use crate::state::ui::{ModalKind, Popover, PrefsPane};

/// The saved connection every studio scene opens. It holds every capability,
/// so every route and every Admin tab renders.
pub const STUDIO_CONNECTION: &str = "staging-cluster";

/// What the window shows under any overlay.
#[derive(Clone, PartialEq)]
pub enum Screen {
    /// The Connection Manager.
    Disconnected,
    /// The studio at `route`. `collection` sets the Explorer selection.
    Studio {
        route: Route,
        collection: Option<(&'static str, StorageMode)>,
    },
}

/// What opens on top of the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overlay {
    None,
    Modal(ModalKind),
    Popover(Popover),
    Palette,
}

#[derive(Clone, PartialEq)]
pub struct Scene {
    /// File stem of the PNG. Lowercase ASCII, digits and `-`.
    pub name: &'static str,
    pub screen: Screen,
    pub overlay: Overlay,
}

fn scene(name: &'static str, screen: Screen, overlay: Overlay) -> Scene {
    Scene {
        name,
        screen,
        overlay,
    }
}

fn at(route: Route) -> Screen {
    Screen::Studio {
        route,
        collection: None,
    }
}

fn collection(name: &'static str, mode: StorageMode) -> Screen {
    Screen::Studio {
        route: Route::Explorer {},
        collection: Some((name, mode)),
    }
}

fn streams(tab: &str) -> Screen {
    at(Route::Streams {
        tab: tab.to_string(),
    })
}

fn admin(tab: &str) -> Screen {
    at(Route::Admin {
        tab: tab.to_string(),
    })
}

/// Capture order. Scenes on the same connection run back to back, so the run
/// connects once.
pub fn scenes() -> Vec<Scene> {
    use Overlay as O;
    use StorageMode as M;
    vec![
        scene("connection-manager", Screen::Disconnected, O::None),
        scene(
            "new-connection",
            Screen::Disconnected,
            O::Modal(ModalKind::NewConnection),
        ),
        scene(
            "explorer-document",
            collection("users", M::Document),
            O::None,
        ),
        scene(
            "explorer-strict",
            collection("accounts", M::Strict),
            O::None,
        ),
        scene(
            "explorer-vector",
            collection("embeddings", M::Vector),
            O::None,
        ),
        scene("explorer-graph", collection("social", M::Graph), O::None),
        scene(
            "explorer-timeseries",
            collection("metrics", M::Timeseries),
            O::None,
        ),
        scene("explorer-kv", collection("sessions", M::Kv), O::None),
        scene(
            "explorer-spatial",
            collection("places", M::Spatial),
            O::None,
        ),
        scene("explorer-fts", collection("articles", M::Fts), O::None),
        scene(
            "insert-document",
            collection("users", M::Document),
            O::Modal(ModalKind::DocForm),
        ),
        scene("query", at(Route::Query {}), O::None),
        scene("designer", at(Route::Designer {}), O::None),
        scene("graph-explorer", at(Route::GraphExplorer {}), O::None),
        scene("vector-space", at(Route::VectorSpace {}), O::None),
        scene(
            "timeseries-dashboard",
            at(Route::TimeseriesDashboard {}),
            O::None,
        ),
        scene("spatial-map", at(Route::SpatialView {}), O::None),
        scene("fts-inspector", at(Route::FtsInspector {}), O::None),
        scene("sync", at(Route::Sync {}), O::None),
        scene("console", at(Route::Console {}), O::None),
        scene("streams-landing", streams("landing"), O::None),
        scene("streams-cdc", streams("cdc"), O::None),
        scene("streams-mv", streams("mv"), O::None),
        scene("streams-topics", streams("topics"), O::None),
        scene("streams-notify", streams("notify"), O::None),
        scene("streams-cron", streams("cron"), O::None),
        scene("admin-rbac", admin("rbac"), O::None),
        scene("admin-rls", admin("rls"), O::None),
        scene("admin-audit", admin("audit"), O::None),
        scene("admin-cluster", admin("cluster"), O::None),
        scene("admin-shards", admin("shards"), O::None),
        scene("admin-nodes", admin("nodes"), O::None),
        scene("admin-raft", admin("raft"), O::None),
        scene(
            "popover-connection",
            at(Route::Explorer {}),
            O::Popover(Popover::Connection),
        ),
        scene(
            "popover-database",
            at(Route::Explorer {}),
            O::Popover(Popover::Database),
        ),
        scene(
            "popover-notifications",
            at(Route::Explorer {}),
            O::Popover(Popover::Notifications),
        ),
        scene(
            "popover-avatar",
            at(Route::Explorer {}),
            O::Popover(Popover::Avatar),
        ),
        scene("command-palette", at(Route::Explorer {}), O::Palette),
        scene(
            "preferences",
            at(Route::Explorer {}),
            O::Modal(ModalKind::Preferences(PrefsPane::Appearance)),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::data::mock;

    #[test]
    fn names_are_unique_file_stems() {
        let all = scenes();
        let mut seen = HashSet::new();
        for s in &all {
            assert!(seen.insert(s.name), "duplicate scene name `{}`", s.name);
            assert!(
                !s.name.is_empty()
                    && s.name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "scene name `{}` is not a lowercase file stem",
                s.name
            );
        }
    }

    /// A route the connection cannot serve redirects to the Explorer, which
    /// would write an Explorer image under the wrong name.
    #[test]
    fn studio_connection_serves_every_scene_route() {
        let caps = mock::connections()
            .into_iter()
            .find(|c| c.name == STUDIO_CONNECTION)
            .and_then(|c| c.profile)
            .map(|p| p.capabilities)
            .expect("the studio connection must be a reachable mock entry");
        for s in scenes() {
            if let Screen::Studio { route, .. } = &s.screen
                && let Some(cap) = route.required_cap()
            {
                assert!(caps.has(cap), "scene `{}` needs {cap:?}", s.name);
            }
        }
        assert!(caps.cluster, "the Admin cluster tabs need a cluster");
    }

    /// A selection absent from the collection list is replaced by the
    /// Explorer's default, which would write the wrong viewer.
    #[test]
    fn every_selected_collection_exists_with_its_mode() {
        let groups = mock::collection_groups();
        for s in scenes() {
            if let Screen::Studio {
                collection: Some((name, mode)),
                ..
            } = s.screen
            {
                let found = groups
                    .iter()
                    .flat_map(|g| &g.collections)
                    .any(|c| c.name == name && c.mode == mode);
                assert!(found, "scene `{}` selects missing `{name}`", s.name);
            }
        }
    }
}
