//! The scenes a run captures, one PNG each per theme.
//!
//! Every studio scene opens `STUDIO_CONNECTION` through the backend seam. The
//! data on screen is whatever that backend serves, so the scene list does
//! not change when the backend does.

use crate::data::mock;
use crate::models::collection::Profile;
use crate::models::connections::ConnectionTest;
use crate::models::records::Filter;
use crate::routes::tabs::{AdminTab, ClusterTab, EventsTab};
use crate::routes::{Destination, Route};
use crate::state::connection_form::{ConnectionForm, TestState};
use crate::state::explorer::{
    CollectionTab, DetailsTab, GraphSub, RecordView, Selected, TransferTab,
};
use crate::state::record_editor::RecordEditor;
use crate::state::ui::{ModalKind, Popover, PrefsPane};

/// The saved connection every studio scene opens. It holds every capability,
/// so every destination renders.
pub const STUDIO_CONNECTION: &str = "staging-cluster";

/// The saved connection whose credentials cannot write.
pub const READ_ONLY_CONNECTION: &str = "Production";

/// How a scene leaves the Explorer: the collection, and how it is viewed.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplorerSeed {
    pub collection: &'static str,
    pub tab: CollectionTab,
    /// A `field = value` filter on the Data tab.
    pub filter: Option<(&'static str, &'static str)>,
    pub details_tab: Option<DetailsTab>,
    /// The relation filter of the Graph tab.
    pub relation: Option<&'static str>,
    /// The Data Transfer tab. `None` is Export.
    pub transfer: Option<TransferTab>,
}

impl ExplorerSeed {
    /// The selection and view the Explorer starts the scene with.
    pub fn target(&self) -> (Selected, RecordView) {
        let view = RecordView {
            tab: self.tab,
            filter: self.filter.map(|(field, value)| Filter {
                field: field.to_string(),
                value: value.to_string(),
            }),
            details_tab: self.details_tab,
            relation: self.relation.map(str::to_string),
            graph_sub: GraphSub::Edges,
            transfer_tab: self.transfer.unwrap_or_default(),
            ..RecordView::default()
        };
        (
            Selected {
                name: self.collection.to_string(),
            },
            view,
        )
    }
}

/// What the window shows under any overlay.
#[derive(Clone, PartialEq)]
pub enum Screen {
    /// The Connection Manager.
    Disconnected,
    /// The studio at `route`. `explorer` sets the Explorer's selection.
    Studio {
        route: Route,
        explorer: Option<ExplorerSeed>,
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
    /// The record editor, open on this edit.
    RecordEditor(Box<RecordEditor>),
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
    /// The saved connection the scene runs on.
    pub connection: &'static str,
    pub overlay: Overlay,
    /// Panels of the scene's page to hide, by `PanelSpec` id. Every other
    /// panel shows at its default size.
    pub hidden: &'static [&'static str],
}

fn scene(name: &'static str, screen: Screen, overlay: Overlay) -> Scene {
    Scene {
        name,
        screen,
        connection: STUDIO_CONNECTION,
        overlay,
        hidden: &[],
    }
}

fn at(route: Route) -> Screen {
    Screen::Studio {
        route,
        explorer: None,
    }
}

fn seed(collection: &'static str) -> ExplorerSeed {
    ExplorerSeed {
        collection,
        tab: CollectionTab::Data,
        filter: None,
        details_tab: None,
        relation: None,
        transfer: None,
    }
}

/// The Explorer on `collection`, with its default view.
fn explorer(collection: &'static str) -> Screen {
    explorer_with(seed(collection))
}

fn explorer_with(seed: ExplorerSeed) -> Screen {
    Screen::Studio {
        route: Route::Explorer {},
        explorer: Some(seed),
    }
}

/// The edit the record editor scene shows: `users/u44182` at revision 18,
/// stored as pending, with the status changed to active.
fn editor_seed() -> Option<RecordEditor> {
    let schema = mock::collection_schema("users")?;
    let mut stored = mock::stored_record("users", "u44182", &mock::Overlay::default())?;
    if let nodedb_types::Value::Object(map) = &mut stored.value {
        map.insert(
            "status".into(),
            nodedb_types::Value::String("pending".into()),
        );
    }
    let mut editor = RecordEditor::edit("users", Profile::Document, schema, stored);
    editor.text = editor.text.replace("\"pending\"", "\"active\"");
    Some(editor)
}

/// Capture order. Scenes on the same connection run back to back, so the run
/// connects once.
pub fn scenes() -> Vec<Scene> {
    use Overlay as O;
    let mut all = vec![
        scene("connections-manager", Screen::Disconnected, O::None),
        scene(
            "connections-create",
            Screen::Disconnected,
            O::NewConnection(Box::new(staging_form())),
        ),
        scene(
            "explorer-documents",
            explorer_with(ExplorerSeed {
                filter: Some(("status", "active")),
                ..seed("users")
            }),
            O::None,
        ),
        scene(
            "explorer-strict-records",
            explorer_with(ExplorerSeed {
                filter: Some(("currency", "USD")),
                ..seed("orders")
            }),
            O::None,
        ),
        scene(
            "explorer-graph-records",
            explorer_with(ExplorerSeed {
                tab: CollectionTab::Graph,
                relation: Some("PURCHASED"),
                ..seed("users")
            }),
            O::None,
        ),
        scene(
            "explorer-vector-records",
            explorer_with(ExplorerSeed {
                details_tab: Some(DetailsTab::Vector),
                ..seed("articles")
            }),
            O::None,
        ),
        scene("explorer-timeseries-records", explorer("sales"), O::None),
        scene("explorer-spatial-records", explorer("places"), O::None),
        scene("explorer-search-records", explorer("articles"), O::None),
        scene(
            "explorer-columnar-records",
            explorer_with(ExplorerSeed {
                filter: Some(("warehouse", "nyc")),
                ..seed("inventory")
            }),
            O::None,
        ),
        scene("explorer-array-data", explorer("climate_cube"), O::None),
        scene("explorer", explorer("sessions"), O::None),
    ];
    if let Some(editor) = editor_seed() {
        all.push(scene(
            "explorer-record-editor",
            explorer("users"),
            O::RecordEditor(Box::new(editor)),
        ));
    }
    all.extend([
        scene(
            "data-import-export",
            Screen::Studio {
                route: Route::DataTransfer {},
                explorer: Some(ExplorerSeed {
                    filter: Some(("status", "active")),
                    ..seed("users")
                }),
            },
            O::None,
        ),
        scene(
            "data-transfer-import",
            Screen::Studio {
                route: Route::DataTransfer {},
                explorer: Some(ExplorerSeed {
                    transfer: Some(TransferTab::Import),
                    ..seed("users")
                }),
            },
            O::None,
        ),
        scene(
            "data-transfer-history",
            Screen::Studio {
                route: Route::DataTransfer {},
                explorer: Some(ExplorerSeed {
                    transfer: Some(TransferTab::History),
                    ..seed("users")
                }),
            },
            O::None,
        ),
        scene("query", at(Route::Query {}), O::None),
        scene(
            "query-view-menu",
            at(Route::Query {}),
            O::Popover(Popover::View),
        ),
        scene("designer", at(Route::Designer {}), O::None),
        scene("console", at(Route::Console {}), O::None),
    ]);
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
            explorer("users"),
            O::Popover(Popover::Connection),
        ),
        scene(
            "shell-database-selector",
            explorer("users"),
            O::Popover(Popover::Database),
        ),
        scene(
            "shell-account",
            explorer("users"),
            O::Popover(Popover::Avatar),
        ),
        scene(
            "shell-notifications",
            explorer("users"),
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
    // The only scene on another connection, so it runs last: a read-only
    // analyst session, an empty filtered result, and the View menu open with
    // Record details unchecked.
    all.push(Scene {
        connection: READ_ONLY_CONNECTION,
        hidden: &["details"],
        ..scene(
            "state-access-empty",
            explorer_with(ExplorerSeed {
                filter: Some(("status", "delayed")),
                ..seed("shipments")
            }),
            O::Popover(Popover::View),
        )
    });
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
    fn each_scene_connection_serves_its_route() {
        for s in scenes() {
            let caps = mock::connections()
                .into_iter()
                .find(|c| c.name == s.connection)
                .and_then(|c| c.profile)
                .map(|p| p.capabilities)
                .expect("a scene connection must be a reachable mock entry");
            if let Screen::Studio { route, .. } = &s.screen
                && let Some(cap) = route.destination().required_cap()
            {
                assert!(caps.has(cap), "scene `{}` needs {cap:?}", s.name);
            }
        }
    }

    /// A selection absent from the catalog is replaced by the Explorer's
    /// default, which would write the wrong view.
    #[test]
    fn every_selected_collection_exists_in_the_catalog() {
        let catalog = mock::collections();
        for s in scenes() {
            if let Screen::Studio {
                explorer: Some(seed),
                ..
            } = s.screen
            {
                assert!(
                    catalog.iter().any(|c| c.name == seed.collection),
                    "scene `{}` selects missing `{}`",
                    s.name,
                    seed.collection
                );
            }
        }
    }

    /// A tab the collection does not offer falls back to Data, which would
    /// write the wrong view.
    #[test]
    fn every_seeded_tab_exists_for_its_collection() {
        let catalog = mock::collections();
        for s in scenes() {
            if let Screen::Studio {
                explorer: Some(seed),
                ..
            } = s.screen
                && let Some(c) = catalog.iter().find(|c| c.name == seed.collection)
            {
                assert!(
                    CollectionTab::for_collection(c).contains(&seed.tab),
                    "scene `{}`: `{}` has no {:?} tab",
                    s.name,
                    seed.collection,
                    seed.tab
                );
                if let Some(d) = seed.details_tab {
                    assert!(
                        DetailsTab::for_collection(c).contains(&d),
                        "scene `{}`: `{}` has no {d:?} details tab",
                        s.name,
                        seed.collection
                    );
                }
            }
        }
    }

    /// The scenes the design names, each once.
    #[test]
    fn every_explorer_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in [
            "explorer-documents",
            "explorer-strict-records",
            "explorer-graph-records",
            "explorer-vector-records",
            "explorer-timeseries-records",
            "explorer-spatial-records",
            "explorer-search-records",
            "explorer-columnar-records",
            "explorer-array-data",
            "explorer",
            "explorer-record-editor",
            "data-import-export",
            "state-access-empty",
        ] {
            assert!(names.contains(&expected), "no scene `{expected}`");
        }
    }

    /// The state scene hides Record details and runs on the read-only session.
    #[test]
    fn the_access_scene_is_read_only_with_the_inspector_hidden() {
        let all = scenes();
        let s = all
            .iter()
            .find(|s| s.name == "state-access-empty")
            .expect("scene");
        assert_eq!(s.connection, READ_ONLY_CONNECTION);
        assert_eq!(s.hidden, ["details"]);
    }
}
