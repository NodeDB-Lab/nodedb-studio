//! The scenes a run captures, one PNG each per theme.
//!
//! Every studio scene opens `STUDIO_CONNECTION` through the backend seam. The
//! data on screen is whatever that backend serves, so the scene list does
//! not change when the backend does.

use crate::data::mock;
use crate::models::collection::Profile;
use crate::models::connections::ConnectionTest;
use crate::routes::Route;
use crate::routes::tabs::GraphTab;
use crate::screenshots::area_scenes::{
    admin, cluster, designer, events, search, spatial, sync, timeseries, vector, workbench,
};
use crate::screenshots::scene::{
    ExplorerSeed, Overlay, READ_ONLY_CONNECTION, Scene, Screen, at, explorer, explorer_with, scene,
    seed,
};
use crate::state::connection_form::{ConnectionForm, TestState};
use crate::state::explorer::{CollectionTab, DetailsTab, TransferTab};
use crate::state::record_editor::RecordEditor;
use crate::state::ui::{ModalKind, Popover, PrefsPane};

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
    ]);
    all.extend(designer());
    all.extend(workbench());
    all.extend([
        scene(
            "graph",
            at(Route::Graph {
                tab: GraphTab::Cypher,
            }),
            O::None,
        ),
        scene(
            "graph-builder",
            at(Route::Graph {
                tab: GraphTab::Builder,
            }),
            O::None,
        ),
        scene(
            "graph-rag",
            at(Route::Graph {
                tab: GraphTab::GraphRag,
            }),
            O::None,
        ),
    ]);
    all.extend(vector());
    all.extend(timeseries());
    all.extend(spatial());
    all.extend(search());
    all.extend(events());
    all.extend(cluster());
    all.extend(admin());
    all.extend(sync());
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::data::mock;
    use crate::screenshots::search_scenes;

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

    /// The Query and Console views the design names, each once.
    #[test]
    fn every_workbench_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in [
            "query",
            "query-view-menu",
            "query-plan",
            "query-library",
            "console-session",
        ] {
            assert!(names.contains(&expected), "no scene `{expected}`");
        }
    }

    /// The sidebar badge and the page count the same tabs.
    #[test]
    fn every_query_scene_opens_the_three_tabs_the_badge_counts() {
        let badge = mock::nav_badges().query as usize;
        for s in scenes() {
            if let Some(q) = &s.query {
                assert_eq!(q.count(), badge, "scene `{}`", s.name);
            }
        }
    }

    /// A hidden panel must exist on the page, or the scene hides nothing.
    #[test]
    fn hidden_panels_exist_on_their_page() {
        for s in scenes() {
            if let Screen::Studio { route, .. } = &s.screen {
                let ids: Vec<&str> = route.surface().panels().iter().map(|p| p.id).collect();
                for h in s.hidden {
                    assert!(ids.contains(h), "scene `{}` hides unknown `{h}`", s.name);
                }
            }
        }
    }

    /// The three Vector Space views the design names, each once.
    #[test]
    fn every_vector_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in ["vector-search", "vector-projection", "vector-index"] {
            assert!(names.contains(&expected), "no scene `{expected}`");
        }
        assert!(!names.contains(&"vector-space"));
    }

    /// The three Search views the design names, each once.
    #[test]
    fn every_search_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in ["search-results", "search-analyze", "search-indexes"] {
            assert!(names.contains(&expected), "no scene `{expected}`");
        }
        assert!(!names.contains(&"search"), "the placeholder scene is gone");
    }

    /// The Search seeds hold a request each page runs.
    #[test]
    fn the_search_scenes_seed_a_run_to_show() {
        let seed = search_scenes::results();
        let state = seed.results.expect("the results scene seeds a query");
        assert!(state.pending.is_some());
        assert!(
            search_scenes::analyze()
                .analyze
                .expect("analyze")
                .pending
                .is_some()
        );
    }

    /// The three Spatial views the design names, each once.
    #[test]
    fn every_spatial_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in ["spatial-map", "spatial-analyze", "spatial-raster"] {
            assert!(names.contains(&expected), "no scene `{expected}`");
        }
        assert!(!names.contains(&"spatial"), "the placeholder scene is gone");
    }

    #[test]
    fn every_events_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in [
            "events-changefeeds",
            "events-feed-editor",
            "events-topics",
            "events-materialized-views",
            "events-notifications",
            "events-scheduled-jobs",
        ] {
            assert!(names.contains(&expected), "no scene `{expected}`");
        }
    }

    #[test]
    fn every_timeseries_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in [
            "timeseries-explore",
            "timeseries-dashboards",
            "timeseries-dashboard-editor",
            "timeseries-lifecycle",
            "timeseries-backfill",
        ] {
            assert!(names.contains(&expected), "no scene `{expected}`");
        }
        assert!(
            !names.contains(&"timeseries"),
            "the placeholder scene is gone"
        );
    }

    #[test]
    fn every_graph_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in ["graph", "graph-builder", "graph-rag"] {
            assert!(names.contains(&expected), "no scene `{expected}`");
        }
        assert!(
            !names.contains(&"graph-explorer"),
            "the placeholder scene is gone"
        );
    }

    #[test]
    fn every_designer_view_has_a_scene() {
        let names: Vec<&str> = scenes().iter().map(|s| s.name).collect();
        for expected in ["designer", "designer-create", "designer-review"] {
            assert!(names.contains(&expected), "no scene `{expected}`");
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
