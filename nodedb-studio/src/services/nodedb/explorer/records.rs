//! Record reads and writes against a live server.
//!
//! 0.6.2 exposes no per-record revision. `Record.revision` is a fingerprint of
//! the record's content (`explorer::sql::revision`). A write reads the record
//! again, compares fingerprints, and refuses a stale one with
//! `StudioError::RevisionConflict`. The write itself then names, in its
//! `WHERE`, the stored value of each field it depends on
//! (`explorer::write`): a change another writer lands in between makes it
//! match no row, read as a conflict. A field whose value the server cannot
//! compare exactly, and a bare key-value entry (`UPSERT` takes no `WHERE`),
//! keep only the fingerprint check. A write that matches no row while the
//! record still reads as the caller read it met a guard the server does not
//! match: that is a refusal, not a conflict.
//!
//! Writes run on the pool. While the workbench holds a transaction open,
//! they are refused: run outside it, a write would survive its ROLLBACK.

use std::time::{SystemTime, UNIX_EPOCH};

use nodedb_client::{NodeDb, NodeDbError, QueryResult};
use nodedb_types::Value;
use nodedb_types::error::ErrorDetails;

use crate::models::collection::Profile;
use crate::models::records::{Record, RecordPage, RecordQuery};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::explorer::sql::{
    Meta, Statement, count_of, page_statements, records_of, select_one,
};
use crate::services::nodedb::explorer::write::{
    Guarded, delete_statement, insert_statement, update_statement,
};
use crate::services::nodedb::explorer::{ident, profile_of_describe, schema_of};

/// A server error from a write, as the seam names it. A refusal of the data
/// reads as `Rejected`. A missing privilege reads as `PermissionDenied`.
pub(in crate::services::nodedb) fn write_error(
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

/// An insert ran, but the server returned no row for it and the id is the
/// server's: Studio cannot read the record back. A retry would insert a
/// second record, so the error says the first one landed.
fn inserted_unseen(collection: &str) -> StudioError {
    StudioError::MalformedRows {
        reason: format!(
            "INSERT ... RETURNING * returned no row, but the record was inserted into \
             {collection} under an id the server assigned. Refresh the list to see it: \
             saving again inserts a second record."
        ),
    }
}

/// A guarded write matched no row, yet the record reads as the caller read
/// it: the server did not match a guard, so a retry fails the same way.
fn unmatched(collection: &str, id: &str, guards: &[String]) -> StudioError {
    let what = if guards.is_empty() {
        "its key".to_string()
    } else {
        let names: Vec<String> = guards.iter().map(|g| format!("`{g}`")).collect();
        format!(
            "the stored value of {}, which the write compares",
            names.join(", ")
        )
    };
    StudioError::Rejected {
        reason: format!(
            "the server matched no row for record {id} of {collection}, although the record \
             still reads as you opened it: the server did not match {what}. Nothing was \
             written. Edit or remove the record in the Query workbench instead."
        ),
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
    pub(in crate::services::nodedb) async fn meta(
        &self,
        collection: &str,
    ) -> Result<Meta, StudioError> {
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

    pub(in crate::services::nodedb) async fn read_records(
        &self,
        collection: &str,
        query: &RecordQuery,
    ) -> Result<RecordPage, StudioError> {
        let meta = self.meta(collection).await?;
        let observed = now_micros();
        let (page, count) = page_statements(&meta, query, observed)?;
        let total = count_of(&self.read(&count).await?)?;
        let result = self.read(&page).await?;
        Ok(RecordPage {
            rows: records_of(&meta, &result)?,
            total,
            observed_at: observed / 1_000_000,
        })
    }

    async fn stored(&self, meta: &Meta, id: &str) -> Result<Record, StudioError> {
        let result = self.read(&select_one(meta, id)?).await?;
        // A keyed read returns the one row. A keyless read returns every row
        // at the id's time: the one `records_of` names `id` is the record.
        let keyed = meta.schema.id_field.is_some();
        records_of(meta, &result)?
            .into_iter()
            .find(|r| keyed || r.id == id)
            .ok_or_else(|| StudioError::RecordMissing {
                collection: meta.collection.clone(),
                id: id.to_string(),
            })
    }

    pub(in crate::services::nodedb) async fn read_record(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Record, StudioError> {
        let meta = self.meta(collection).await?;
        self.stored(&meta, id).await
    }

    /// Refuses an Explorer write while the workbench holds a transaction
    /// open: it would run on the pool and survive the transaction's ROLLBACK.
    pub(in crate::services::nodedb) fn outside_tx(&self) -> Result<(), StudioError> {
        if self.tx_open() {
            Err(StudioError::TransactionOpen)
        } else {
            Ok(())
        }
    }

    pub(in crate::services::nodedb) async fn write_record(
        &self,
        collection: &str,
        id: Option<&str>,
        value: Value,
    ) -> Result<Record, StudioError> {
        self.outside_tx()?;
        let meta = self.meta(collection).await?;
        let statement = insert_statement(&meta, id, &value)?;
        let result = self
            .run(&statement)
            .await
            .map_err(|e| write_error(e, "INSERT", &meta.collection))?;
        match records_of(&meta, &result)?.into_iter().next() {
            Some(record) => Ok(record),
            // A server that returns no row for RETURNING: read the record back.
            None => match id {
                Some(id) => self.stored(&meta, id).await,
                None => Err(inserted_unseen(&meta.collection)),
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

    pub(in crate::services::nodedb) async fn replace_record(
        &self,
        collection: &str,
        id: &str,
        expected_revision: u64,
        value: Value,
    ) -> Result<Record, StudioError> {
        self.outside_tx()?;
        let meta = self.meta(collection).await?;
        if meta.profile == Profile::Timeseries {
            return Err(StudioError::Rejected {
                reason: "timeseries data is append-only: a record cannot change".to_string(),
            });
        }
        let stored = self.current(&meta, id, expected_revision).await?;
        self.apply_update(&meta, &stored, &value).await
    }

    /// Writes `value` over `stored`, read just before. A write that matches
    /// no row met a change made since that read, or a removal.
    async fn apply_update(
        &self,
        meta: &Meta,
        stored: &Record,
        value: &Value,
    ) -> Result<Record, StudioError> {
        let Some(write) = update_statement(meta, &stored.id, &stored.value, value)? else {
            return Ok(stored.clone());
        };
        let result = self
            .run(&write.statement)
            .await
            .map_err(|e| write_error(e, "UPDATE", &meta.collection))?;
        if result.rows_affected == 0 {
            return Err(self.lost(meta, stored, &write).await);
        }
        self.stored(meta, &stored.id).await
    }

    /// Why `write`, guarded by `stored`, matched no row: the record changed
    /// since that read, or it is gone, or it reads unchanged and the server
    /// did not match a guard.
    async fn lost(&self, meta: &Meta, stored: &Record, write: &Guarded) -> StudioError {
        match self.stored(meta, &stored.id).await {
            Ok(now) if now.revision == stored.revision => {
                unmatched(&meta.collection, &stored.id, &write.guards)
            }
            Ok(now) => StudioError::RevisionConflict {
                collection: meta.collection.clone(),
                id: stored.id.clone(),
                expected: stored.revision,
                stored: now.revision,
            },
            Err(e) => e,
        }
    }

    pub(in crate::services::nodedb) async fn remove_record(
        &self,
        collection: &str,
        id: &str,
        expected_revision: u64,
    ) -> Result<(), StudioError> {
        self.outside_tx()?;
        let meta = self.meta(collection).await?;
        if meta.profile == Profile::Timeseries {
            return Err(StudioError::Rejected {
                reason: "timeseries data is append-only: expire it with a retention policy"
                    .to_string(),
            });
        }
        let stored = self.current(&meta, id, expected_revision).await?;
        self.apply_delete(&meta, &stored).await
    }

    /// Removes `stored`, read just before, while it still reads that way.
    async fn apply_delete(&self, meta: &Meta, stored: &Record) -> Result<(), StudioError> {
        let write = delete_statement(meta, &stored.id, &stored.value)?;
        let result = self
            .run(&write.statement)
            .await
            .map_err(|e| write_error(e, "DELETE", &meta.collection))?;
        if result.rows_affected == 0 {
            return Err(self.lost(meta, stored, &write).await);
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
        let search = RecordQuery {
            text: "dee".into(),
            ..RecordQuery::new(10)
        };
        match svc.records(name, &search).await {
            Err(StudioError::Rejected { reason }) => assert!(reason.contains("0.6.2"), "{reason}"),
            other => panic!("text search on a schemaless collection must refuse, got {other:?}"),
        }
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
        q.range = crate::models::records::TimeRange::Last30Days;
        assert_eq!(svc.records(name, &q).await.expect("30d").total, 2);

        // The inspector opens the first row: it reads back by its id.
        let first = &week.rows[0];
        assert_eq!(
            &svc.record(name, &first.id).await.expect("first row"),
            first
        );
        // An identical point gets its own id. A newer point renames nothing.
        let point = |at: i64, cpu: f64| {
            obj(&[
                (
                    "ts",
                    Value::DateTime(nodedb_types::NdbDateTime::from_micros(at * 1_000_000)),
                ),
                ("cpu", Value::Float(cpu)),
            ])
        };
        svc.insert_record(name, None, point(now - 3_600, 1.0))
            .await
            .expect("insert a repeat");
        q.range = crate::models::records::TimeRange::Last7Days;
        let before = svc.records(name, &q).await.expect("7d").rows;
        svc.insert_record(name, None, point(now - 60, 9.0))
            .await
            .expect("insert a newer point");
        let after = svc.records(name, &q).await.expect("7d").rows;
        let ids = |rows: &[Record]| rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&after[1..]), ids(&before), "{:?}", ids(&after));
        assert_ne!(before[0].id, before[1].id, "a repeat has its own id");
        assert_eq!(
            &svc.record(name, &before[1].id).await.expect("repeat"),
            &before[1]
        );

        assert!(matches!(
            svc.record(name, "row:0").await,
            Err(StudioError::Rejected { .. })
        ));
        assert!(matches!(
            svc.update_record(
                name,
                &first.id,
                first.revision,
                obj(&[("cpu", Value::Float(9.0))])
            )
            .await,
            Err(StudioError::Rejected { .. })
        ));
        assert!(matches!(
            svc.delete_record(name, &first.id, first.revision).await,
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
    async fn live_explorer_kv_edit_keeps_the_fields_it_does_not_change() {
        let svc = live().await;
        let kv = "studio_probe_x_rec_kv2";
        fresh(
            &svc,
            kv,
            &format!(
                "CREATE COLLECTION {kv} (key TEXT PRIMARY KEY, name TEXT, email TEXT) WITH (engine='kv')"
            ),
        )
        .await;
        let made = svc
            .insert_record(
                kv,
                None,
                obj(&[("key", s("k1")), ("name", s("n1")), ("email", s("e1"))]),
            )
            .await
            .expect("insert");
        let saved = svc
            .update_record(
                kv,
                "k1",
                made.revision,
                obj(&[("key", s("k1")), ("name", s("n2")), ("email", s("e1"))]),
            )
            .await
            .expect("update");
        assert_eq!(saved.get("name"), Some(&s("n2")));
        assert_eq!(
            saved.get("email"),
            Some(&s("e1")),
            "an edit of name keeps email"
        );
        let cleared = svc
            .update_record(
                kv,
                "k1",
                saved.revision,
                obj(&[("key", s("k1")), ("name", s("n2"))]),
            )
            .await
            .expect("update");
        assert!(matches!(cleared.get("email"), None | Some(Value::Null)));
        assert_eq!(cleared.get("name"), Some(&s("n2")));
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_an_edit_leaves_untouched_fields_byte_identical() {
        use crate::models::records::check::set_field;
        use crate::state::record_editor::RecordEditor;
        let svc = live().await;
        let name = "studio_probe_x_rec_bytes";
        fresh(
            &svc,
            name,
            &format!(
                "CREATE COLLECTION {name} (id TEXT PRIMARY KEY, name TEXT, b BYTES, v VECTOR(5), \
                 at TIMESTAMP) WITH (engine='document_strict')"
            ),
        )
        .await;
        svc.client(None)
            .expect("a client")
            .execute_sql(
                &format!("INSERT INTO {name} (id, name, b, v, at) VALUES ($1, $2, $3, $4, $5)"),
                &[
                    s("r1"),
                    s("one"),
                    Value::Bytes((0u8..16).collect()),
                    Value::Vector(vec![0.1, 0.2, 0.3, 0.4, 0.5].into()),
                    Value::DateTime(nodedb_types::NdbDateTime::from_micros(
                        1_781_356_860_000_001,
                    )),
                ],
            )
            .await
            .expect("seeds");
        let before = svc.record(name, "r1").await.expect("record");
        assert!(
            matches!(before.get("b"), Some(Value::Bytes(_))),
            "{before:?}"
        );
        let schema = svc.collection_schema(name).await.expect("schema");
        let mut editor = RecordEditor::edit(
            name,
            Profile::DocumentStrict,
            schema.clone(),
            before.clone(),
        );
        assert!(editor.text.contains(" bytes>\""), "{}", editor.text);
        editor.text = set_field(&editor.text, &schema.fields, "name", "uno").expect("edits");
        assert_eq!(editor.changes().len(), 1, "the review shows one change");
        let saved = svc
            .update_record(name, "r1", before.revision, editor.draft().expect("parses"))
            .await
            .expect("update");
        assert_eq!(saved.get("name"), Some(&s("uno")));
        for f in ["b", "v", "at"] {
            assert_eq!(saved.get(f), before.get(f), "{f} is untouched");
        }
        let mut placeholder = saved.value.clone();
        if let Value::Object(m) = &mut placeholder {
            m.insert("b".into(), s("<3 bytes>"));
        }
        assert!(matches!(
            svc.update_record(name, "r1", saved.revision, placeholder)
                .await,
            Err(StudioError::Rejected { .. })
        ));
    }

    /// Each profile, with every scalar type 0.6.2 has: DATE is not one of
    /// them (`unknown column type: 'DATE'`).
    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_a_write_after_another_writer_conflicts() {
        use crate::services::nodedb::explorer::write::delete_statement;
        let svc = live().await;
        let typed = "d DECIMAL(10,2), dn DECIMAL, u UUID, tz TIMESTAMPTZ, s TEXT";
        for (name, columns, key) in [
            (
                "studio_probe_x_rec_g_st",
                format!("(id TEXT PRIMARY KEY, name TEXT, at TIMESTAMP, f FLOAT, {typed}) WITH (engine='document_strict')"),
                "id",
            ),
            ("studio_probe_x_rec_g_doc", String::new(), "id"),
            (
                "studio_probe_x_rec_g_kv",
                format!("(key TEXT PRIMARY KEY, name TEXT, f FLOAT, {typed}) WITH (engine='kv')"),
                "key",
            ),
            (
                "studio_probe_x_rec_g_col",
                format!("(id BIGINT PRIMARY KEY, name TEXT, at TIMESTAMP, f FLOAT, {typed}) WITH (engine='columnar')"),
                "id",
            ),
            (
                "studio_probe_x_rec_g_sp",
                "(id BIGINT PRIMARY KEY, name TEXT, geom GEOMETRY SPATIAL_INDEX) WITH (engine='spatial')".to_string(),
                "id",
            ),
        ] {
            fresh(&svc, name, &format!("CREATE COLLECTION {name} {columns}")).await;
            let mut fields = vec![("name", s("a")), ("f", Value::Float(0.1))];
            if columns.contains("at TIMESTAMP") || columns.is_empty() {
                fields.push((
                    "at",
                    Value::DateTime(nodedb_types::NdbDateTime::from_micros(
                        1_781_356_860_000_001,
                    )),
                ));
            }
            if columns.contains("UUID") || columns.is_empty() {
                fields.extend([
                    ("d", s("89.5")),
                    ("dn", s("3.14159")),
                    ("u", s("6f1c2a3e-1b2c-4d5e-8f90-1234567890ab")),
                    (
                        "tz",
                        Value::DateTime(nodedb_types::NdbDateTime::from_micros(
                            1_781_356_860_000_002,
                        )),
                    ),
                    ("s", s("it's \"q\" é 漢字 🙂")),
                ]);
            }
            if columns.contains("GEOMETRY") {
                fields.retain(|(f, _)| *f == "name");
            }
            let read = svc
                .insert_record(name, Some("1"), obj(&fields))
                .await
                .unwrap_or_else(|e| panic!("{name}: insert: {e}"));
            let meta = svc.meta(name).await.expect("meta");
            if columns.contains("dn DECIMAL") {
                // A bare DECIMAL keeps its places and the editor sets no limit.
                assert_eq!(read.get("dn"), Some(&s("3.14159")), "{name}");
                assert_eq!(
                    meta.schema.field("dn").map(|f| f.ty),
                    Some(crate::models::schema::FieldType::Decimal {
                        precision: None,
                        scale: None
                    }),
                    "{name}"
                );
                let found =
                    crate::models::records::check::violations(true, &meta.schema.fields, &read.value);
                assert!(found.iter().all(|v| v.field != "dn"), "{name}: {found:?}");
            }
            // A guard the server does not match on an unchanged record is a
            // refusal that names it, never a conflict.
            let write = delete_statement(&meta, &read.id, &read.value).expect("builds");
            // A declared UUID stays out. Schemaless `u` is text, and guards.
            assert_eq!(
                write.guards.iter().any(|g| g == "u"),
                columns.is_empty(),
                "{name}: {:?}",
                write.guards
            );
            match svc.lost(&meta, &read, &write).await {
                StudioError::Rejected { reason } => {
                    assert!(reason.contains("`name`"), "{name}: {reason}")
                }
                other => panic!("{name}: expected a refusal, got {other:?}"),
            }
            let mut edit = read.value.clone();
            if let Value::Object(m) = &mut edit {
                m.insert("name".into(), s("mine"));
            }
            // An unchanged record takes the guarded write.
            let mine = svc
                .apply_update(&meta, &read, &edit)
                .await
                .unwrap_or_else(|e| panic!("{name}: guarded update: {e}"));
            assert_eq!(mine.get("name"), Some(&s("mine")), "{name}");
            // Another writer lands after the read: the write matches no row.
            svc.client(None)
                .expect("a client")
                .execute_sql(
                    &format!("UPDATE {name} SET \"name\" = 'theirs' WHERE \"{key}\" = $1"),
                    &[meta.id_value("1").expect("id")],
                )
                .await
                .expect("their write");
            let mut again = mine.value.clone();
            if let Value::Object(m) = &mut again {
                m.insert("name".into(), s("mine again"));
            }
            match svc.apply_update(&meta, &mine, &again).await {
                Err(StudioError::RevisionConflict {
                    expected, stored, ..
                }) => {
                    assert_eq!(expected, mine.revision, "{name}");
                    assert_ne!(stored, mine.revision, "{name}");
                }
                other => panic!("{name}: expected a conflict, got {other:?}"),
            }
            assert!(
                matches!(
                    svc.apply_delete(&meta, &mine).await,
                    Err(StudioError::RevisionConflict { .. })
                ),
                "{name}"
            );
            let theirs = svc.record(name, "1").await.expect("their record");
            assert_eq!(
                theirs.get("name"),
                Some(&s("theirs")),
                "{name}: nothing overwritten"
            );
            // Every guarded field matches its stored value, so the record
            // can go.
            svc.apply_delete(&meta, &theirs)
                .await
                .unwrap_or_else(|e| panic!("{name}: guarded delete: {e}"));
            assert!(
                matches!(
                    svc.apply_update(&meta, &theirs, &again).await,
                    Err(StudioError::RecordMissing { .. })
                ),
                "{name}"
            );
        }
    }

    /// Documents a 0.6.2 defect: bytes written over the native protocol
    /// read back as other bytes or as text. Studio refuses to write bytes
    /// because of it. When this test fails, the server stores bytes as
    /// written: lift `NO_BYTES` in `explorer::write`.
    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_bytes_do_not_round_trip() {
        let svc = live().await;
        let written = Value::Bytes((0u8..16).collect());
        let st = "studio_probe_x_rec_bin_st";
        let doc = "studio_probe_x_rec_bin_doc";
        let kv = "studio_probe_x_rec_bin_kv";
        fresh(&svc, st, &format!("CREATE COLLECTION {st} (id TEXT PRIMARY KEY, name TEXT, b BYTES) WITH (engine='document_strict')")).await;
        fresh(&svc, doc, &format!("CREATE COLLECTION {doc}")).await;
        fresh(
            &svc,
            kv,
            &format!(
                "CREATE COLLECTION {kv} (key TEXT PRIMARY KEY, value BYTES) WITH (engine='kv')"
            ),
        )
        .await;
        let refused = |r: Result<Record, StudioError>, why: &str| match r {
            Err(StudioError::Rejected { reason }) => assert!(reason.contains(why), "{reason}"),
            other => panic!("expected a refusal naming {why}, got {other:?}"),
        };
        for (name, key, field) in [(st, "id", "b"), (doc, "id", "b"), (kv, "key", "value")] {
            svc.client(None)
                .expect("a client")
                .execute_sql(
                    &format!("INSERT INTO {name} (\"{key}\", \"{field}\") VALUES ($1, $2)"),
                    &[s("r1"), written.clone()],
                )
                .await
                .expect("seeds");
            let back = svc.record(name, "r1").await.expect("reads");
            assert!(back.get(field).is_some(), "{name}: {back:?}");
            assert_ne!(
                back.get(field),
                Some(&written),
                "{name}: bytes now round-trip: lift NO_BYTES in explorer_write"
            );
            refused(
                svc.insert_record(name, Some("r2"), obj(&[(field, written.clone())]))
                    .await,
                "0.6.2",
            );
            refused(
                svc.insert_record(name, Some("r2"), obj(&[(field, s("<16 bytes>"))]))
                    .await,
                &format!("`{field}`"),
            );
            for value in [written.clone(), s("<16 bytes>")] {
                let mut edit = back.value.clone();
                if let Value::Object(m) = &mut edit {
                    m.insert(field.into(), value);
                }
                refused(
                    svc.update_record(name, "r1", back.revision, edit).await,
                    &format!("`{field}`"),
                );
            }
            assert_eq!(
                svc.record(name, "r1").await.expect("reads"),
                back,
                "{name}: untouched"
            );
        }
        // An edit that leaves the bytes alone still saves.
        let back = svc.record(st, "r1").await.expect("reads");
        let mut edit = back.value.clone();
        if let Value::Object(m) = &mut edit {
            m.insert("name".into(), s("n"));
        }
        let saved = svc
            .update_record(st, "r1", back.revision, edit)
            .await
            .expect("an edit of name");
        assert_eq!(saved.get("b"), back.get("b"));
    }

    /// A field name 0.6.2 takes in double quotes is written like any other.
    /// One holding `"` (another client can insert it) blocks only an edit of
    /// that field.
    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_field_names_outside_bare_identifiers() {
        let svc = live().await;
        let doc = "studio_probe_x_rec_names_doc";
        fresh(&svc, doc, &format!("CREATE COLLECTION {doc}")).await;
        svc.client(None)
            .expect("a client")
            .execute_sql(
                &format!("INSERT INTO {doc} (\"id\", \"name\", \"e-mail\", \"a\"\"q\") VALUES ($1, $2, $3, $4)"),
                &[s("d1"), s("n"), s("x@y"), s("q")],
            )
            .await
            .expect("seeds");
        let read = svc.record(doc, "d1").await.expect("reads");
        let with = |r: &Record, field: &str, value: &str| {
            let mut v = r.value.clone();
            if let Value::Object(m) = &mut v {
                m.insert(field.into(), s(value));
            }
            v
        };
        let saved = svc
            .update_record(doc, "d1", read.revision, with(&read, "name", "n2"))
            .await
            .expect("an edit of name");
        assert_eq!(saved.get("name"), Some(&s("n2")));
        assert_eq!(saved.get("a\"q"), Some(&s("q")), "kept");
        let saved = svc
            .update_record(doc, "d1", saved.revision, with(&saved, "e-mail", "z@y"))
            .await
            .expect("an edit of e-mail");
        assert_eq!(saved.get("e-mail"), Some(&s("z@y")));
        match svc
            .update_record(doc, "d1", saved.revision, with(&saved, "a\"q", "q2"))
            .await
        {
            Err(StudioError::Rejected { reason }) => assert!(reason.contains("a\"q"), "{reason}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        let odd = obj(&[
            ("id", s("d2")),
            ("first name", s("f")),
            ("o'k", s("o")),
            ("名前", s("m")),
        ]);
        let made = svc
            .insert_record(doc, None, odd.clone())
            .await
            .expect("inserts");
        assert_eq!(made.value, odd);
        for r in [saved, made] {
            svc.delete_record(doc, &r.id, r.revision)
                .await
                .expect("deletes");
        }

        let st = "studio_probe_x_rec_names_st";
        fresh(&svc, st, &format!("CREATE COLLECTION {st} (id TEXT PRIMARY KEY, \"e-mail\" TEXT) WITH (engine='document_strict')")).await;
        let made = svc
            .insert_record(st, Some("s1"), obj(&[("e-mail", s("a@b"))]))
            .await
            .expect("inserts");
        let saved = svc
            .update_record(st, "s1", made.revision, with(&made, "e-mail", "c@d"))
            .await
            .expect("updates");
        assert_eq!(saved.get("e-mail"), Some(&s("c@d")));
        svc.delete_record(st, "s1", saved.revision)
            .await
            .expect("deletes");
    }

    /// While the workbench holds a transaction open, Explorer writes would
    /// run outside it and survive its ROLLBACK, so they are refused.
    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_writes_wait_for_an_open_transaction() {
        use crate::models::workbench::TxAccess;
        use crate::services::workbench_data::WorkbenchData;
        let svc = live().await;
        let name = "studio_probe_x_rec_tx";
        fresh(&svc, name, &format!("CREATE COLLECTION {name}")).await;
        let made = svc
            .insert_record(name, None, obj(&[("id", s("x1")), ("name", s("a"))]))
            .await
            .expect("inserts");
        svc.begin_transaction(TxAccess::ReadWrite)
            .await
            .expect("begins");
        let open = |e: Result<(), StudioError>| {
            assert!(matches!(e, Err(StudioError::TransactionOpen)), "{e:?}");
        };
        open(
            svc.insert_record(name, None, obj(&[("id", s("x2"))]))
                .await
                .map(|_| ()),
        );
        open(
            svc.update_record(
                name,
                "x1",
                made.revision,
                obj(&[("id", s("x1")), ("name", s("b"))]),
            )
            .await
            .map(|_| ()),
        );
        open(svc.delete_record(name, "x1", made.revision).await);
        let text = StudioError::TransactionOpen.to_string();
        assert!(text.contains("commit") && text.contains("roll"), "{text}");
        svc.rollback_transaction().await.expect("rolls back");
        assert_eq!(svc.record(name, "x1").await.expect("reads"), made);
        svc.delete_record(name, "x1", made.revision)
            .await
            .expect("deletes");
    }

    #[test]
    fn a_guard_the_server_does_not_match_is_named() {
        let text = unmatched("c", "r1", &["u".into(), "name".into()]).to_string();
        assert!(
            text.contains("`u`, `name`") && text.contains("Nothing was written"),
            "{text}"
        );
        assert!(unmatched("c", "r1", &[]).to_string().contains("its key"));
    }

    #[test]
    fn an_insert_the_server_does_not_return_says_it_landed() {
        let text = inserted_unseen("c").to_string();
        assert!(
            text.contains("was inserted") && text.contains("Refresh"),
            "{text}"
        );
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
