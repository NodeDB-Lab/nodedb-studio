//! Record reads and writes against a live server.
//!
//! 0.6.2 exposes no per-record revision. `Record.revision` is a fingerprint of
//! the record's content (`explorer_sql::revision`). A write reads the record
//! again, compares fingerprints, and refuses a stale one with
//! `StudioError::RevisionConflict`. Another writer can still land between that
//! read and the write: the server has no conditional write to close the gap.

use std::time::{SystemTime, UNIX_EPOCH};

use nodedb_client::{NodeDb, NodeDbError, QueryResult};
use nodedb_types::Value;
use nodedb_types::error::ErrorDetails;

use crate::models::collection::Profile;
use crate::models::records::{Record, RecordPage, RecordQuery};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::explorer::{ident, profile_of_describe, schema_of};
use crate::services::nodedb::explorer_sql::{
    Meta, Statement, count_of, delete_statement, insert_statement, page_statements, records_of,
    select_one, update_statement,
};

/// A server error from a write, as the seam names it. A refusal of the data
/// reads as `Rejected`. A missing privilege reads as `PermissionDenied`.
pub(super) fn write_error(
    e: NodeDbError,
    privilege: &'static str,
    collection: &str,
) -> StudioError {
    match e.details() {
        ErrorDetails::AuthorizationDenied { .. } => StudioError::PermissionDenied {
            privilege,
            collection: collection.to_string(),
        },
        ErrorDetails::ConstraintViolation { .. }
        | ErrorDetails::TypeMismatch { .. }
        | ErrorDetails::BadRequest
        | ErrorDetails::UndefinedColumn { .. }
        | ErrorDetails::PlanError { .. } => StudioError::Rejected {
            reason: e.message().to_string(),
        },
        // ErrorDetails is foreign and non-exhaustive: the arm is required.
        _ => StudioError::from(e),
    }
}

fn now_micros() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX))
}

impl NodeDbConnectionService {
    /// The profile and declared shape of `collection`. An unknown collection
    /// is the server's not-found error.
    pub(super) async fn meta(&self, collection: &str) -> Result<Meta, StudioError> {
        let name = ident(collection)?;
        let describe = self.table(&format!("DESCRIBE {name}")).await?;
        let profile = profile_of_describe(name, &describe)?;
        let schema = schema_of(name, profile, &describe, &[])?;
        Ok(Meta {
            collection: name.to_string(),
            profile,
            schema,
        })
    }

    async fn run(&self, statement: &Statement) -> Result<QueryResult, NodeDbError> {
        let client = self.client(None).map_err(|_| {
            NodeDbError::config("no session is open: connect before reading records")
        })?;
        client.execute_sql(&statement.sql, &statement.params).await
    }

    async fn read(&self, statement: &Statement) -> Result<QueryResult, StudioError> {
        Ok(self.run(statement).await?)
    }

    pub(super) async fn read_records(
        &self,
        collection: &str,
        query: &RecordQuery,
    ) -> Result<RecordPage, StudioError> {
        let meta = self.meta(collection).await?;
        let observed = now_micros();
        let (page, count) = page_statements(&meta, query, observed)?;
        let total = count_of(&self.read(&count).await?)?;
        let result = self.read(&page).await?;
        let offset = query.page_size.max(1).saturating_mul(query.page);
        Ok(RecordPage {
            rows: records_of(&meta, &result, offset)?,
            total,
            observed_at: observed / 1_000_000,
        })
    }

    async fn stored(&self, meta: &Meta, id: &str) -> Result<Record, StudioError> {
        let result = self.read(&select_one(meta, id)?).await?;
        records_of(meta, &result, 0)?
            .into_iter()
            .next()
            .ok_or_else(|| StudioError::RecordMissing {
                collection: meta.collection.clone(),
                id: id.to_string(),
            })
    }

    pub(super) async fn read_record(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Record, StudioError> {
        let meta = self.meta(collection).await?;
        self.stored(&meta, id).await
    }

    pub(super) async fn write_record(
        &self,
        collection: &str,
        id: Option<&str>,
        value: Value,
    ) -> Result<Record, StudioError> {
        let meta = self.meta(collection).await?;
        let statement = insert_statement(&meta, id, &value)?;
        let result = self
            .run(&statement)
            .await
            .map_err(|e| write_error(e, "INSERT", &meta.collection))?;
        match records_of(&meta, &result, 0)?.into_iter().next() {
            Some(record) => Ok(record),
            // A server that returns no row for RETURNING: read the record back.
            None => match id {
                Some(id) => self.stored(&meta, id).await,
                None => Err(StudioError::MalformedRows {
                    reason: "INSERT ... RETURNING * returned no row".to_string(),
                }),
            },
        }
    }

    /// The stored record, after a check that nobody changed it since the
    /// caller read revision `expected`.
    async fn current(&self, meta: &Meta, id: &str, expected: u64) -> Result<Record, StudioError> {
        let stored = self.stored(meta, id).await?;
        if stored.revision == expected {
            Ok(stored)
        } else {
            Err(StudioError::RevisionConflict {
                collection: meta.collection.clone(),
                id: id.to_string(),
                expected,
                stored: stored.revision,
            })
        }
    }

    pub(super) async fn replace_record(
        &self,
        collection: &str,
        id: &str,
        expected_revision: u64,
        value: Value,
    ) -> Result<Record, StudioError> {
        let meta = self.meta(collection).await?;
        if meta.profile == Profile::Timeseries {
            return Err(StudioError::Rejected {
                reason: "timeseries data is append-only: a record cannot change".to_string(),
            });
        }
        let stored = self.current(&meta, id, expected_revision).await?;
        let Some(statement) = update_statement(&meta, id, &stored.value, &value)? else {
            return Ok(stored);
        };
        let result = self
            .run(&statement)
            .await
            .map_err(|e| write_error(e, "UPDATE", &meta.collection))?;
        if result.rows_affected == 0 {
            return Err(StudioError::RecordMissing {
                collection: meta.collection.clone(),
                id: id.to_string(),
            });
        }
        self.stored(&meta, id).await
    }

    pub(super) async fn remove_record(
        &self,
        collection: &str,
        id: &str,
        expected_revision: u64,
    ) -> Result<(), StudioError> {
        let meta = self.meta(collection).await?;
        if meta.profile == Profile::Timeseries {
            return Err(StudioError::Rejected {
                reason: "timeseries data is append-only: expire it with a retention policy"
                    .to_string(),
            });
        }
        self.current(&meta, id, expected_revision).await?;
        let result = self
            .run(&delete_statement(&meta, id)?)
            .await
            .map_err(|e| write_error(e, "DELETE", &meta.collection))?;
        if result.rows_affected == 0 {
            return Err(StudioError::RecordMissing {
                collection: meta.collection.clone(),
                id: id.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::records::{Bbox, Filter};
    use crate::services::explorer_data::ExplorerData;
    use crate::services::nodedb::service::live;

    fn obj(fields: &[(&str, Value)]) -> Value {
        Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    fn s(text: &str) -> Value {
        Value::String(text.into())
    }

    /// Drop and create `name`, so a second run starts from the same state.
    async fn fresh(svc: &NodeDbConnectionService, name: &str, create: &str) {
        let c = svc.client(None).expect("a client");
        c.query(&format!("DROP COLLECTION IF EXISTS {name}"))
            .await
            .expect("drops");
        c.query(create).await.expect("creates");
    }

    fn ids(page: &RecordPage) -> Vec<&str> {
        page.rows.iter().map(|r| r.id.as_str()).collect()
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_records_page_sort_filter_and_search() {
        let svc = live().await;
        let name = "studio_probe_x_rec_strict";
        fresh(
            &svc,
            name,
            &format!("CREATE COLLECTION {name} (id TEXT PRIMARY KEY, name TEXT NOT NULL, qty BIGINT) WITH (engine='document_strict')"),
        )
        .await;
        for (id, n, q) in [
            ("a", "alpha", 1),
            ("b", "50% off", 2),
            ("c", "gamma", 2),
            ("d", "delta_x", 3),
            ("e", "echo", 3),
        ] {
            svc.insert_record(
                name,
                None,
                obj(&[("id", s(id)), ("name", s(n)), ("qty", Value::Integer(q))]),
            )
            .await
            .expect("inserts");
        }
        let mut q = RecordQuery::new(2);
        let first = svc.records(name, &q).await.expect("page 0");
        assert_eq!((first.total, ids(&first)), (5, vec!["a", "b"]));
        q.page = 2;
        assert_eq!(ids(&svc.records(name, &q).await.expect("page 2")), ["e"]);
        q.page = 0;
        q.sort = Some(("qty".into(), true));
        assert_eq!(
            ids(&svc.records(name, &q).await.expect("sorted")),
            ["d", "e"]
        );
        q.sort = None;
        q.filter = Some(Filter {
            field: "qty".into(),
            value: "2".into(),
        });
        let filtered = svc.records(name, &q).await.expect("filtered");
        assert_eq!((filtered.total, ids(&filtered)), (2, vec!["b", "c"]));
        q.filter = None;
        q.text = "50%".into();
        let hit = svc.records(name, &q).await.expect("text");
        assert_eq!(
            (hit.total, ids(&hit)),
            (1, vec!["b"]),
            "a percent sign matches itself"
        );
        q.text = "elta_".into();
        assert_eq!(svc.records(name, &q).await.expect("text").total, 1);
        q.text = "elta?".into();
        assert_eq!(svc.records(name, &q).await.expect("text").total, 0);
        q.text = String::new();
        q.sort = Some(("nope".into(), false));
        assert!(matches!(
            svc.records(name, &q).await,
            Err(StudioError::Rejected { .. })
        ));
        assert!(matches!(
            svc.records("studio_probe_x_no_such", &RecordQuery::new(2))
                .await,
            Err(StudioError::NotFound(_))
        ));
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_strict_record_writes_check_the_revision() {
        let svc = live().await;
        let name = "studio_probe_x_rec_rev";
        fresh(
            &svc,
            name,
            &format!("CREATE COLLECTION {name} (id TEXT PRIMARY KEY, name TEXT NOT NULL, qty BIGINT, meta JSON) WITH (engine='document_strict')"),
        )
        .await;
        let made = svc
            .insert_record(
                name,
                Some("r1"),
                obj(&[
                    ("name", s("one")),
                    ("qty", Value::Integer(1)),
                    ("meta", obj(&[("k", Value::Integer(7))])),
                ]),
            )
            .await
            .expect("insert");
        assert_eq!(made.id, "r1");
        assert_eq!(
            made.get("meta"),
            Some(&obj(&[("k", Value::Integer(7))])),
            "a JSON column reads back structured"
        );
        let read = svc.record(name, "r1").await.expect("record");
        assert_eq!(read, made, "insert and read agree, revision included");

        let mut next = read.value.clone();
        if let Value::Object(m) = &mut next {
            m.insert("name".into(), s("uno"));
            m.insert("qty".into(), Value::Null);
        }
        let saved = svc
            .update_record(name, "r1", read.revision, next.clone())
            .await
            .expect("update");
        assert_eq!(saved.get("name"), Some(&s("uno")));
        assert_eq!(saved.get("qty"), Some(&Value::Null));
        assert_ne!(saved.revision, read.revision);
        // unchanged content writes nothing and keeps the revision
        let same = svc
            .update_record(name, "r1", saved.revision, saved.value.clone())
            .await
            .expect("no-op");
        assert_eq!(same.revision, saved.revision);

        match svc.update_record(name, "r1", read.revision, next).await {
            Err(StudioError::RevisionConflict {
                expected, stored, ..
            }) => {
                assert_eq!((expected, stored), (read.revision, saved.revision));
            }
            other => panic!("expected a conflict, got {other:?}"),
        }
        assert!(matches!(
            svc.delete_record(name, "r1", read.revision).await,
            Err(StudioError::RevisionConflict { .. })
        ));
        assert!(matches!(
            svc.insert_record(name, Some("r1"), obj(&[("name", s("dup"))]))
                .await,
            Err(StudioError::Rejected { .. })
        ));
        assert!(
            matches!(
                svc.insert_record(name, Some("r2"), obj(&[("qty", Value::Integer(1))]))
                    .await,
                Err(StudioError::Rejected { .. })
            ),
            "a NOT NULL column without a value is refused"
        );
        assert!(matches!(
            svc.update_record(
                name,
                "r1",
                saved.revision,
                obj(&[("id", s("r1")), ("zzz", s("x"))])
            )
            .await,
            Err(StudioError::Rejected { .. })
        ));
        svc.delete_record(name, "r1", saved.revision)
            .await
            .expect("delete");
        assert!(matches!(
            svc.record(name, "r1").await,
            Err(StudioError::RecordMissing { .. })
        ));
        assert!(matches!(
            svc.delete_record(name, "r1", saved.revision).await,
            Err(StudioError::RecordMissing { .. })
        ));
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_schemaless_update_drops_a_removed_field() {
        let svc = live().await;
        let name = "studio_probe_x_rec_doc";
        fresh(&svc, name, &format!("CREATE COLLECTION {name}")).await;
        let made = svc
            .insert_record(
                name,
                None,
                obj(&[
                    ("id", s("d1")),
                    ("name", s("dee")),
                    ("n", Value::Integer(1)),
                    ("tags", Value::Array(vec![s("x")])),
                    ("o", obj(&[("k", Value::Integer(1))])),
                ]),
            )
            .await
            .expect("insert");
        assert_eq!(made.get("tags"), Some(&Value::Array(vec![s("x")])));
        assert_eq!(
            made.get("o"),
            Some(&s("{\"k\": 1}")),
            "the server stores an object as JSON text"
        );
        let auto = svc
            .insert_record(name, None, obj(&[("name", s("auto"))]))
            .await
            .expect("server id");
        assert!(!auto.id.is_empty());
        let next = obj(&[("id", s("d1")), ("name", s("dee2"))]);
        let saved = svc
            .update_record(name, "d1", made.revision, next)
            .await
            .expect("update");
        assert_eq!(saved.get("n"), None, "a removed field reads as absent");
        assert_eq!(saved.get("name"), Some(&s("dee2")));
        let page = svc
            .records(name, &RecordQuery::new(10))
            .await
            .expect("page");
        let row = page.rows.iter().find(|r| r.id == "d1").expect("d1 listed");
        assert_eq!(row, &saved, "a list read and a single read agree");
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_kv_and_columnar_records() {
        let svc = live().await;
        let kv = "studio_probe_x_rec_kv";
        fresh(
            &svc,
            kv,
            &format!(
                "CREATE COLLECTION {kv} (key TEXT PRIMARY KEY, value TEXT) WITH (engine='kv')"
            ),
        )
        .await;
        let made = svc
            .insert_record(kv, None, obj(&[("key", s("k1")), ("value", s("v1"))]))
            .await
            .expect("insert");
        let saved = svc
            .update_record(
                kv,
                "k1",
                made.revision,
                obj(&[("key", s("k1")), ("value", s("v2"))]),
            )
            .await
            .expect("upsert");
        assert_eq!(saved.get("value"), Some(&s("v2")));
        svc.delete_record(kv, "k1", saved.revision)
            .await
            .expect("delete");
        assert_eq!(
            svc.records(kv, &RecordQuery::new(5))
                .await
                .expect("page")
                .total,
            0
        );

        let col = "studio_probe_x_rec_col";
        fresh(&svc, col, &format!("CREATE COLLECTION {col} (id BIGINT PRIMARY KEY, name TEXT) WITH (engine='columnar')")).await;
        let row = svc
            .insert_record(col, Some("20"), obj(&[("name", s("n"))]))
            .await
            .expect("insert");
        assert_eq!(row.id, "20");
        assert_eq!(
            svc.record(col, "20").await.expect("record").get("id"),
            Some(&Value::Integer(20))
        );
        assert!(matches!(
            svc.record(col, "x").await,
            Err(StudioError::Rejected { .. })
        ));
        svc.delete_record(col, "20", row.revision)
            .await
            .expect("delete");
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_timeseries_reads_inside_a_range_and_refuses_changes() {
        let svc = live().await;
        let name = "studio_probe_x_rec_ts";
        fresh(&svc, name, &format!("CREATE COLLECTION {name} (ts TIMESTAMP TIME_KEY, cpu FLOAT) WITH (engine='timeseries')")).await;
        let now = chrono_free_now();
        for (at, cpu) in [
            (now - 3_600, 1.0),
            (now - 3 * 86_400, 2.0),
            (now - 40 * 86_400, 3.0),
        ] {
            svc.insert_record(
                name,
                None,
                obj(&[
                    (
                        "ts",
                        Value::DateTime(nodedb_types::NdbDateTime::from_micros(at * 1_000_000)),
                    ),
                    ("cpu", Value::Float(cpu)),
                ]),
            )
            .await
            .expect("insert");
        }
        let mut q = RecordQuery::new(10);
        q.range = crate::models::records::TimeRange::Last24Hours;
        assert_eq!(svc.records(name, &q).await.expect("24h").total, 1);
        q.range = crate::models::records::TimeRange::Last7Days;
        let week = svc.records(name, &q).await.expect("7d");
        assert_eq!(week.total, 2);
        assert_eq!(
            week.rows[0].get("cpu"),
            Some(&Value::Float(1.0)),
            "newest first"
        );
        assert!(week.rows[0].id.starts_with("row:"));
        q.range = crate::models::records::TimeRange::Last30Days;
        assert_eq!(svc.records(name, &q).await.expect("30d").total, 2);
        assert!(matches!(
            svc.record(name, "row:0").await,
            Err(StudioError::Rejected { .. })
        ));
        assert!(matches!(
            svc.update_record(name, "row:0", 0, obj(&[("cpu", Value::Float(9.0))]))
                .await,
            Err(StudioError::Rejected { .. })
        ));
        assert!(matches!(
            svc.delete_record(name, "row:0", 0).await,
            Err(StudioError::Rejected { .. })
        ));
    }

    fn chrono_free_now() -> i64 {
        i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs(),
        )
        .expect("fits")
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_spatial_bbox_filters_and_counts() {
        let svc = live().await;
        let name = "studio_probe_x_rec_sp";
        fresh(&svc, name, &format!("CREATE COLLECTION {name} (id BIGINT PRIMARY KEY, geom GEOMETRY SPATIAL_INDEX) WITH (engine='spatial')")).await;
        for (id, lon, lat) in [(1, -73.98, 40.74), (2, -74.0, 40.7), (3, 2.35, 48.85)] {
            let geo = format!("{{\"type\":\"Point\",\"coordinates\":[{lon},{lat}]}}");
            svc.insert_record(
                name,
                None,
                obj(&[("id", Value::Integer(id)), ("geom", s(&geo))]),
            )
            .await
            .expect("insert");
        }
        let mut q = RecordQuery::new(10);
        assert_eq!(svc.records(name, &q).await.expect("all").total, 3);
        q.bbox = Bbox::parse("-74.5, 40, -73.5, 41").ok();
        let hit = svc.records(name, &q).await.expect("bbox");
        assert_eq!(hit.total, 2);
        assert_eq!(hit.rows.len(), 2);
    }
}
