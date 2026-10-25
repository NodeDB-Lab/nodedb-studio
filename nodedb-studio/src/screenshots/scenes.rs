//! The scenes a run captures, one PNG each per theme.
//!
//! Every studio scene opens `STUDIO_CONNECTION` through the backend seam. The
//! data on screen is whatever that backend serves, so the scene list does
//! not change when the backend does.

use crate::models::collection::StorageMode;
use crate::models::connections::ConnectionTest;
use crate::routes::tabs::{AdminTab, ClusterTab, EventsTab};
use crate::routes::{Destination, Route};
use crate::state::connection_form::{ConnectionForm, TestState};
use crate::state::ui::{ModalKind, Popover, PrefsPane};

/// The saved connection every studio scene opens. It holds every capability,
/// so every destination renders.
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
#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    None,
    Modal(ModalKind),
    /// The connection dialog, open with these fields.
    NewConnection(Box<ConnectionForm>),
    Popover(Popover),
    /// The palette, open with this text typed.
    Palette(&'static str),
}

/// The New connection dialog as the design shows it: a staging Origin with a
/// passed test. The password is a placeholder of the right length.
fn staging_form() -> ConnectionForm {
    ConnectionForm {
        name: "Staging".into(),
        host: "db.staging.example".into(),
        database: "analytics".into(),
        username: "admin".into(),
        secret: "xxxxxxxxxxx".into(),
        server_name: "db.staging.example".into(),
        ca_file: "/etc/ssl/certs/staging-ca.pem".into(),
        test: TestState::Passed(ConnectionTest {
            latency_ms: 22,
            tls_verified: true,
            server_version: "1.4.2".into(),
            db_count: 12,
        }),
        ..ConnectionForm::blank()
    }
}

#[derive(Clone, PartialEq)]
pub struct Scene {
    /// File stem of the PNG. Lowercase ASCII, digits and `-`.
    pub name: &'static str,
    pub screen: Screen,
    pub overlay: Overlay,
    /// Panels of the scene's page to hide, by `PanelSpec` id. Every other
    /// panel shows at its default size.
    pub hidden: &'static [&'static str],
}

fn scene(name: &'static str, screen: Screen, overlay: Overlay) -> Scene {
    Scene {
        name,
        screen,
        overlay,
        hidden: &[],
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

/// Capture order. Scenes on the same connection run back to back, so the run
/// connects once.
pub fn scenes() -> Vec<Scene> {
    use Overlay as O;
    use StorageMode as M;
    let mut all = vec![
        scene("connections-manager", Screen::Disconnected, O::None),
        scene(
            "connections-create",
            Screen::Disconnected,
            O::NewConnection(Box::new(staging_form())),
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
        scene(
            "query-view-menu",
            at(Route::Query {}),
            O::Popover(Popover::View),
        ),
        scene("designer", at(Route::Designer {}), O::None),
        scene("console", at(Route::Console {}), O::None),
    ];
    all.extend([
        scene("graph-explorer", at(Destination::Graph.route()), O::None),
        scene("vector-space", at(Destination::Vector.route()), O::None),
        scene("timeseries", at(Destination::Timeseries.route()), O::None),
        scene("spatial", at(Destination::Spatial.route()), O::None),
        scene("search", at(Destination::Search.route()), O::None),
    ]);
    for tab in EventsTab::ALL {
        all.push(scene(
            events_scene(*tab),
            at(Route::Events { tab: *tab }),
            O::None,
        ));
    }
    for tab in ClusterTab::ALL {
        all.push(scene(
            cluster_scene(*tab),
            at(Route::Cluster { tab: *tab }),
            O::None,
        ));
    }
    for tab in AdminTab::ALL {
        all.push(scene(
            admin_scene(*tab),
            at(Route::Admin { tab: *tab }),
            O::None,
        ));
    }
    all.push(scene(
        "sync-devices",
        at(Destination::Sync.route()),
        O::None,
    ));
    all.extend([
        scene(
            "shell-connection-switcher",
            collection("users", M::Document),
            O::Popover(Popover::Connection),
        ),
        scene(
            "shell-database-selector",
            collection("users", M::Document),
            O::Popover(Popover::Database),
        ),
        scene(
            "shell-account",
            collection("users", M::Document),
            O::Popover(Popover::Avatar),
        ),
        scene(
            "shell-notifications",
            collection("users", M::Document),
            O::Popover(Popover::Notifications),
        ),
        scene(
            "shell-preferences",
            at(Route::Explorer {}),
            O::Modal(ModalKind::Preferences(PrefsPane::Appearance)),
        ),
        scene(
            "shell-command-palette",
            at(Route::Query {}),
            O::Palette("graph"),
        ),
    ]);
    all
}

fn events_scene(tab: EventsTab) -> &'static str {
    match tab {
        EventsTab::ChangeFeeds => "events-change-feeds",
        EventsTab::MaterializedViews => "events-materialized-views",
        EventsTab::Topics => "events-topics",
        EventsTab::Notifications => "events-notifications",
        EventsTab::ScheduledJobs => "events-scheduled-jobs",
    }
}

fn cluster_scene(tab: ClusterTab) -> &'static str {
    match tab {
        ClusterTab::Nodes => "cluster-nodes",
        ClusterTab::RaftGroups => "cluster-raft-groups",
        ClusterTab::Placement => "cluster-placement",
        ClusterTab::Monitoring => "cluster-monitoring",
        ClusterTab::Activity => "cluster-activity",
    }
}

fn admin_scene(tab: AdminTab) -> &'static str {
    match tab {
        AdminTab::Access => "admin-access",
        AdminTab::RowPolicies => "admin-row-policies",
        AdminTab::Audit => "admin-audit",
        AdminTab::Backups => "admin-backups",
        AdminTab::Config => "admin-config",
        AdminTab::Credentials => "admin-credentials",
    }
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
                && let Some(cap) = route.destination().required_cap()
            {
                assert!(caps.has(cap), "scene `{}` needs {cap:?}", s.name);
            }
        }
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
