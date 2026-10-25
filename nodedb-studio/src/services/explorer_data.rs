//! Explorer reads and writes at the backend seam.
//!
//! Reads return typed models: the catalog, a collection's schema, a page of
//! records. Writes name the revision they read, and the server refuses one
//! whose revision moved (`StudioError::RevisionConflict`). Every method
//! scopes to the active session's database.

use async_trait::async_trait;
use nodedb_types::Value;

use crate::models::array_data::{ArraySchema, SliceRequest, SliceResult};
use crate::models::collection::Collection;
use crate::models::graph_records::{EdgePage, EdgeQuery};
use crate::models::records::{CollectionAccess, Record, RecordPage, RecordQuery};
use crate::models::schema::CollectionSchema;
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait ExplorerData {
    /// Every collection of the active database, in display order.
    async fn catalog(&self) -> Result<Vec<Collection>, StudioError>;

    /// The declared fields, columns, and indexes of a record collection.
    async fn collection_schema(&self, collection: &str) -> Result<CollectionSchema, StudioError>;

    /// What the session may do to a collection.
    async fn collection_access(&self, collection: &str) -> Result<CollectionAccess, StudioError>;

    /// One page of records matching `query`.
    async fn records(
        &self,
        collection: &str,
        query: &RecordQuery,
    ) -> Result<RecordPage, StudioError>;

    /// One record as the server holds it now.
    /// `StudioError::RecordMissing` when it does not exist.
    async fn record(&self, collection: &str, id: &str) -> Result<Record, StudioError>;

    /// Insert a record. `id` names it, or the server assigns one. The
    /// session needs INSERT.
    async fn insert_record(
        &self,
        collection: &str,
        id: Option<&str>,
        value: Value,
    ) -> Result<Record, StudioError>;

    /// Replace a record. `expected_revision` is the revision the caller
    /// read: a stale one is `StudioError::RevisionConflict`.
    async fn update_record(
        &self,
        collection: &str,
        id: &str,
        expected_revision: u64,
        value: Value,
    ) -> Result<Record, StudioError>;

    /// Remove a record, under the same revision check as an update.
    async fn delete_record(
        &self,
        collection: &str,
        id: &str,
        expected_revision: u64,
    ) -> Result<(), StudioError>;

    /// One page of a graph's edges.
    async fn edges(&self, graph: &str, query: &EdgeQuery) -> Result<EdgePage, StudioError>;

    /// Remove one edge.
    async fn delete_edge(&self, graph: &str, id: &str) -> Result<(), StudioError>;

    /// The dimensions and attributes of an array.
    async fn array_schema(&self, array: &str) -> Result<ArraySchema, StudioError>;

    /// The cells of one slice. A range outside the extents, or a slice over
    /// the cell limit, is `StudioError::Rejected`.
    async fn array_slice(
        &self,
        array: &str,
        request: &SliceRequest,
    ) -> Result<SliceResult, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::collection::Profile;
    use crate::models::record_json::{normalize, pretty};
    use crate::models::records::Filter;
    use crate::services::async_state::AsyncState;
    use crate::services::connection_service::ConnectionService;
    use crate::services::mock::MockConnectionService;
    use crate::state::connections_registry::Credentials;

    async fn connected(name: &str) -> MockConnectionService {
        let svc = MockConnectionService::ready();
        let creds = Credentials {
            username: "tester".into(),
            password: None,
        };
        svc.connect(name, &creds).await.expect("saved connection");
        svc
    }

    #[tokio::test]
    async fn the_catalog_names_every_profile_once_per_collection() {
        let svc = MockConnectionService::ready();
        let catalog = svc.catalog().await.expect("catalog");
        let mut names: Vec<&str> = catalog.iter().map(|c| c.name.as_str()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(total, names.len(), "collection names are unique keys");
        for profile in [
            Profile::Document,
            Profile::DocumentStrict,
            Profile::KeyValue,
            Profile::Columnar,
            Profile::Timeseries,
            Profile::Spatial,
            Profile::Array,
        ] {
            assert!(
                catalog.iter().any(|c| c.profile == profile),
                "{profile:?} has no collection in the catalog"
            );
        }
    }

    #[tokio::test]
    async fn every_collection_reads_a_first_page_with_unique_record_ids() {
        let svc = MockConnectionService::ready();
        for c in svc.catalog().await.expect("catalog") {
            if c.profile == Profile::Array {
                continue;
            }
            let schema = svc.collection_schema(&c.name).await.expect("schema");
            let page = svc
                .records(&c.name, &RecordQuery::new(schema.default_page_size))
                .await
                .expect("page");
            assert!(!page.rows.is_empty(), "{} reads empty", c.name);
            let mut ids: Vec<&str> = page.rows.iter().map(|r| r.id.as_str()).collect();
            let n = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(n, ids.len(), "{} repeats an id", c.name);
        }
    }

    #[tokio::test]
    async fn the_catalog_count_matches_the_total_a_read_reports() {
        let svc = MockConnectionService::ready();
        for c in svc.catalog().await.expect("catalog") {
            let Some(rows) = c.rows.filter(|_| c.profile != Profile::Timeseries) else {
                continue;
            };
            let page = svc
                .records(&c.name, &RecordQuery::new(1))
                .await
                .expect("page");
            assert_eq!(page.total, rows, "{}", c.name);
        }
    }

    #[tokio::test]
    async fn empty_behaviour_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.catalog().await));
        assert!(s.is_empty());
        let page = svc
            .records("users", &RecordQuery::new(5))
            .await
            .expect("page");
        assert_eq!((page.total, page.rows.len()), (0, 0));
    }

    #[tokio::test]
    async fn erroring_behaviour_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.catalog().await));
        assert!(s.error_message().is_some());
        assert!(svc.records("users", &RecordQuery::new(5)).await.is_err());
        assert!(svc.record("users", "u44182").await.is_err());
        assert!(
            svc.update_record("users", "u44182", 18, Value::Null)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn an_unknown_collection_or_record_is_a_typed_error() {
        let svc = MockConnectionService::ready();
        assert!(matches!(
            svc.collection_schema("nope").await,
            Err(StudioError::NotFound(_))
        ));
        assert!(matches!(
            svc.record("users", "nope").await,
            Err(StudioError::RecordMissing { .. })
        ));
    }

    #[tokio::test]
    async fn an_update_bumps_the_revision_and_later_reads_see_it() {
        let svc = MockConnectionService::ready();
        let stored = svc.record("users", "u44182").await.expect("record");
        assert_eq!(stored.revision, 18);
        let mut value = stored.value.clone();
        if let Value::Object(m) = &mut value {
            m.insert("status".into(), Value::String("suspended".into()));
        }
        let saved = svc
            .update_record("users", "u44182", 18, value)
            .await
            .expect("update");
        assert_eq!(saved.revision, 19);
        let again = svc.record("users", "u44182").await.expect("record");
        assert_eq!(
            again.get("status"),
            Some(&Value::String("suspended".into()))
        );
        let filtered = svc
            .records(
                "users",
                &RecordQuery {
                    filter: Some(Filter {
                        field: "status".into(),
                        value: "suspended".into(),
                    }),
                    ..RecordQuery::new(5)
                },
            )
            .await
            .expect("page");
        assert!(filtered.rows.iter().any(|r| r.id == "u44182"));
    }

    #[tokio::test]
    async fn a_stale_revision_is_a_conflict_and_changes_nothing() {
        let svc = MockConnectionService::ready();
        let stored = svc.record("users", "u44182").await.expect("record");
        svc.update_record("users", "u44182", 18, stored.value.clone())
            .await
            .expect("first writer");
        let stale = svc
            .update_record("users", "u44182", 18, stored.value.clone())
            .await;
        match stale {
            Err(StudioError::RevisionConflict {
                expected, stored, ..
            }) => assert_eq!((expected, stored), (18, 19)),
            other => panic!("expected a conflict, got {other:?}"),
        }
        assert_eq!(
            svc.record("users", "u44182")
                .await
                .expect("record")
                .revision,
            19
        );
        // A delete under the stale revision conflicts too.
        assert!(matches!(
            svc.delete_record("users", "u44182", 18).await,
            Err(StudioError::RevisionConflict { .. })
        ));
    }

    #[tokio::test]
    async fn a_delete_removes_the_record_from_reads_and_counts() {
        let svc = MockConnectionService::ready();
        svc.delete_record("orders", "o100", 6)
            .await
            .expect("delete");
        assert!(matches!(
            svc.record("orders", "o100").await,
            Err(StudioError::RecordMissing { .. })
        ));
        let page = svc
            .records("orders", &RecordQuery::new(50))
            .await
            .expect("page");
        assert_eq!(page.total, 4);
    }

    #[tokio::test]
    async fn an_insert_lands_first_and_a_duplicate_id_is_rejected() {
        let svc = MockConnectionService::ready();
        let value = svc.record("users", "u44182").await.expect("record").value;
        let mut fresh = normalize(&value, &[]);
        if let Value::Object(m) = &mut fresh {
            m.insert("id".into(), Value::String("u99999".into()));
        }
        let made = svc
            .insert_record("users", None, fresh.clone())
            .await
            .expect("insert");
        assert_eq!((made.id.as_str(), made.revision), ("u99999", 1));
        let page = svc
            .records("users", &RecordQuery::new(5))
            .await
            .expect("page");
        assert_eq!(page.rows[0].id, "u99999");
        assert_eq!(page.total, 1285);
        assert!(matches!(
            svc.insert_record("users", None, fresh).await,
            Err(StudioError::Rejected { .. })
        ));
    }

    #[tokio::test]
    async fn a_strict_collection_rejects_a_record_outside_its_schema() {
        let svc = MockConnectionService::ready();
        let mut value = svc.record("orders", "o104").await.expect("record").value;
        if let Value::Object(m) = &mut value {
            m.insert("surprise".into(), Value::Bool(true));
        }
        match svc.update_record("orders", "o104", 18, value).await {
            Err(StudioError::Rejected { reason }) => {
                assert!(reason.contains("surprise"), "{reason}")
            }
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_read_only_session_cannot_write() {
        let svc = connected("Production").await;
        let access = svc.collection_access("shipments").await.expect("access");
        assert!(access.read.allowed() && !access.create.allowed());
        assert_eq!(access.role, "analyst");
        assert!(matches!(
            svc.insert_record("shipments", None, Value::Object(Default::default()))
                .await,
            Err(StudioError::PermissionDenied {
                privilege: "INSERT",
                ..
            })
        ));
        let admin = connected("staging-cluster").await;
        assert!(
            admin
                .collection_access("shipments")
                .await
                .expect("access")
                .create
                .allowed()
        );
    }

    #[tokio::test]
    async fn the_pretty_form_of_a_stored_record_parses_back_unchanged() {
        let svc = MockConnectionService::ready();
        for (collection, id) in [
            ("users", "u44182"),
            ("orders", "o104"),
            ("sessions", "session:u_44182"),
        ] {
            let schema = svc.collection_schema(collection).await.expect("schema");
            let record = svc.record(collection, id).await.expect("record");
            let text = pretty(&record.value, &schema.fields);
            let back =
                crate::models::record_json::parse_object(&text, &schema.fields).expect("parses");
            let diff = crate::models::record_check::changes(&record.value, &back, &schema.fields);
            assert!(diff.is_empty(), "{collection}/{id}: {diff:?}");
        }
    }

    #[tokio::test]
    async fn edges_filter_by_relation_and_a_delete_removes_one() {
        let svc = MockConnectionService::ready();
        let q = EdgeQuery {
            relation: Some("PURCHASED".into()),
            ..EdgeQuery::new(10)
        };
        let page = svc.edges("social_graph", &q).await.expect("edges");
        assert_eq!((page.total, page.edges.len()), (54, 10));
        svc.delete_edge("social_graph", "e_7f3a2d59")
            .await
            .expect("delete");
        assert_eq!(
            svc.edges("social_graph", &q).await.expect("edges").total,
            53
        );
        assert!(matches!(
            svc.delete_edge("social_graph", "e_7f3a2d59").await,
            Err(StudioError::RecordMissing { .. })
        ));
    }

    #[tokio::test]
    async fn an_array_slice_reads_and_a_bad_range_is_rejected() {
        let svc = MockConnectionService::ready();
        let schema = svc.array_schema("climate_cube").await.expect("schema");
        assert_eq!(schema.shape(), vec![24, 181, 360]);
        let ok = SliceRequest {
            time_index: 12,
            lat: (40.0, 41.0),
            lon: (-74.0, -73.0),
            attribute: "temperature_c".into(),
        };
        assert_eq!(
            svc.array_slice("climate_cube", &ok)
                .await
                .expect("slice")
                .cell(5, 5),
            Some(22.4)
        );
        let bad = SliceRequest {
            lat: (10.0, 11.0),
            ..ok
        };
        assert!(matches!(
            svc.array_slice("climate_cube", &bad).await,
            Err(StudioError::Rejected { .. })
        ));
    }
}
