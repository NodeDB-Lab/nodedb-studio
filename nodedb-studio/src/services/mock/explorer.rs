//! Catalog, record reads, and record writes. Writes land in the service's
//! in-memory overlay and every later read sees them.

use async_trait::async_trait;
use nodedb_client::NodeDbError;
use nodedb_types::Value;

use crate::data::mock;
use crate::models::array_data::{ArraySchema, SliceRequest, SliceResult};
use crate::models::collection::{Collection, Profile};
use crate::models::graph_records::{EdgePage, EdgeQuery};
use crate::models::record_check::violations;
use crate::models::records::{CollectionAccess, Record, RecordPage, RecordQuery};
use crate::models::schema::CollectionSchema;
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one, apply_one_or_empty};

fn not_found(collection: &str) -> StudioError {
    StudioError::NotFound(NodeDbError::collection_not_found(collection))
}

fn missing(collection: &str, id: &str) -> StudioError {
    StudioError::RecordMissing {
        collection: collection.to_string(),
        id: id.to_string(),
    }
}

impl MockConnectionService {
    /// The user name and read-only flag of the session the last `connect`
    /// opened. Before any connect, an administrator.
    fn identity(&self) -> (String, bool) {
        let name = self.session.borrow().clone();
        let saved = self.saved.borrow();
        name.and_then(|n| saved.iter().find(|c| c.name == n))
            .and_then(|c| c.profile.as_ref())
            .map(|p| (p.user.clone(), p.capabilities.readonly))
            .unwrap_or_else(|| ("admin".to_string(), false))
    }

    fn access(&self, collection: &str) -> Result<CollectionAccess, StudioError> {
        let (user, read_only) = self.identity();
        mock::collection_access(collection, &user, read_only).ok_or_else(|| not_found(collection))
    }

    fn profile(&self, collection: &str) -> Result<Profile, StudioError> {
        mock::collections()
            .into_iter()
            .find(|c| c.name == collection)
            .map(|c| c.profile)
            .ok_or_else(|| not_found(collection))
    }

    /// A write is refused while the behavior is `Erroring`, like a read.
    async fn gate(&self) -> Result<(), StudioError> {
        apply_one(self.behavior, || ()).await
    }
}

/// The key-value entry fields the server derives from a value.
fn derive_kv(mut record: Record) -> Record {
    if let Value::Object(map) = &mut record.value {
        let value = map.get("value").cloned().unwrap_or(Value::Null);
        let kind = match &value {
            Value::Bool(_) => "bool",
            Value::Integer(_) => "counter",
            Value::Bytes(_) => "bytes",
            _ => "json",
        };
        let size = crate::models::record_json::compact(&value, &[]).len() as i64;
        map.insert("type".to_string(), Value::String(kind.to_string()));
        map.insert("size".to_string(), Value::Integer(size));
    }
    record
}

#[async_trait(?Send)]
impl ExplorerData for MockConnectionService {
    async fn catalog(&self) -> Result<Vec<Collection>, StudioError> {
        apply(self.behavior, mock::collections).await
    }

    async fn collection_schema(&self, collection: &str) -> Result<CollectionSchema, StudioError> {
        let c = collection.to_string();
        apply_one(self.behavior, move || {
            mock::collection_schema(&c).ok_or_else(|| not_found(&c))
        })
        .await?
    }

    async fn collection_access(&self, collection: &str) -> Result<CollectionAccess, StudioError> {
        let access = self.access(collection);
        apply_one(self.behavior, move || access).await?
    }

    async fn records(
        &self,
        collection: &str,
        query: &RecordQuery,
    ) -> Result<RecordPage, StudioError> {
        let overlay = self.records.borrow().clone();
        let (c, q) = (collection.to_string(), query.clone());
        let page = move || mock::record_page(&c, &q, &overlay).ok_or_else(|| not_found(&c));
        let empty = {
            let c = collection.to_string();
            move || {
                if mock::collection_schema(&c).is_some() {
                    Ok(RecordPage {
                        rows: Vec::new(),
                        total: 0,
                        observed_at: mock::observed_at(),
                    })
                } else {
                    Err(not_found(&c))
                }
            }
        };
        apply_one_or_empty(self.behavior, page, empty).await?
    }

    async fn record(&self, collection: &str, id: &str) -> Result<Record, StudioError> {
        let overlay = self.records.borrow().clone();
        let (c, i) = (collection.to_string(), id.to_string());
        apply_one(self.behavior, move || {
            mock::stored_record(&c, &i, &overlay).ok_or_else(|| missing(&c, &i))
        })
        .await?
    }

    async fn insert_record(
        &self,
        collection: &str,
        id: Option<&str>,
        value: Value,
    ) -> Result<Record, StudioError> {
        self.gate().await?;
        if !self.access(collection)?.create.allowed() {
            return Err(StudioError::PermissionDenied {
                privilege: "INSERT",
                collection: collection.to_string(),
            });
        }
        let profile = self.profile(collection)?;
        let schema = mock::collection_schema(collection).ok_or_else(|| not_found(collection))?;
        if let Some(v) = violations(profile.enforces_schema(), &schema.fields, &value).first() {
            return Err(StudioError::Rejected {
                reason: format!("`{}` {}", v.field, v.message),
            });
        }
        let from_field = schema.id_field.as_deref().and_then(|f| match &value {
            Value::Object(m) => m.get(f).and_then(|v| v.as_str().map(str::to_string)),
            _ => None,
        });
        let mut overlay = self.records.borrow_mut();
        let id = id.map(str::to_string).or(from_field).unwrap_or_else(|| {
            let n = overlay.inserted.get(collection).map_or(0, Vec::len) + 1;
            format!("new_{n:04}")
        });
        if mock::stored_record(collection, &id, &overlay).is_some() {
            return Err(StudioError::Rejected {
                reason: format!("{collection}/{id} already exists"),
            });
        }
        let mut record = Record {
            id,
            revision: 1,
            value,
        };
        if profile == Profile::KeyValue {
            record = derive_kv(record);
        }
        overlay
            .inserted
            .entry(collection.to_string())
            .or_default()
            .insert(0, record.clone());
        Ok(record)
    }

    async fn update_record(
        &self,
        collection: &str,
        id: &str,
        expected_revision: u64,
        value: Value,
    ) -> Result<Record, StudioError> {
        self.gate().await?;
        let access = self.access(collection)?;
        if !access.update.allowed() {
            return Err(StudioError::PermissionDenied {
                privilege: "UPDATE",
                collection: collection.to_string(),
            });
        }
        let profile = self.profile(collection)?;
        let schema = mock::collection_schema(collection).ok_or_else(|| not_found(collection))?;
        let mut overlay = self.records.borrow_mut();
        let stored =
            mock::stored_record(collection, id, &overlay).ok_or_else(|| missing(collection, id))?;
        if stored.revision != expected_revision {
            return Err(StudioError::RevisionConflict {
                collection: collection.to_string(),
                id: id.to_string(),
                expected: expected_revision,
                stored: stored.revision,
            });
        }
        if let Some(v) = violations(profile.enforces_schema(), &schema.fields, &value).first() {
            return Err(StudioError::Rejected {
                reason: format!("`{}` {}", v.field, v.message),
            });
        }
        let mut record = Record {
            id: id.to_string(),
            revision: stored.revision + 1,
            value,
        };
        if profile == Profile::KeyValue {
            record = derive_kv(record);
        }
        let key = (collection.to_string(), id.to_string());
        if let Some(rows) = overlay.inserted.get_mut(collection)
            && let Some(slot) = rows.iter_mut().find(|r| r.id == id)
        {
            *slot = record.clone();
        } else {
            overlay.replaced.insert(key, record.clone());
        }
        Ok(record)
    }

    async fn delete_record(
        &self,
        collection: &str,
        id: &str,
        expected_revision: u64,
    ) -> Result<(), StudioError> {
        self.gate().await?;
        if !self.access(collection)?.delete.allowed() {
            return Err(StudioError::PermissionDenied {
                privilege: "DELETE",
                collection: collection.to_string(),
            });
        }
        let mut overlay = self.records.borrow_mut();
        let stored =
            mock::stored_record(collection, id, &overlay).ok_or_else(|| missing(collection, id))?;
        if stored.revision != expected_revision {
            return Err(StudioError::RevisionConflict {
                collection: collection.to_string(),
                id: id.to_string(),
                expected: expected_revision,
                stored: stored.revision,
            });
        }
        if let Some(rows) = overlay.inserted.get_mut(collection) {
            rows.retain(|r| r.id != id);
        }
        overlay
            .replaced
            .remove(&(collection.to_string(), id.to_string()));
        overlay
            .deleted
            .insert((collection.to_string(), id.to_string()));
        Ok(())
    }

    async fn edges(&self, graph: &str, query: &EdgeQuery) -> Result<EdgePage, StudioError> {
        let deleted = self.records.borrow().edges_deleted.clone();
        let (g, q) = (graph.to_string(), query.clone());
        let empty = EdgePage {
            edges: Vec::new(),
            total: 0,
            relations: Vec::new(),
        };
        apply_one_or_empty(
            self.behavior,
            move || {
                if g == mock::GRAPH {
                    Ok(mock::edge_page(&q, &deleted))
                } else {
                    Err(not_found(&g))
                }
            },
            move || Ok(empty),
        )
        .await?
    }

    async fn delete_edge(&self, graph: &str, id: &str) -> Result<(), StudioError> {
        self.gate().await?;
        if graph != mock::GRAPH {
            return Err(not_found(graph));
        }
        if !self.access("users")?.update.allowed() {
            return Err(StudioError::PermissionDenied {
                privilege: "DELETE",
                collection: "users".to_string(),
            });
        }
        let known = mock::edge_page(&EdgeQuery::new(1000), &self.records.borrow().edges_deleted)
            .edges
            .iter()
            .any(|e| e.id == id);
        if !known {
            return Err(missing(graph, id));
        }
        self.records
            .borrow_mut()
            .edges_deleted
            .insert(id.to_string());
        Ok(())
    }

    async fn array_schema(&self, array: &str) -> Result<ArraySchema, StudioError> {
        let a = array.to_string();
        apply_one(self.behavior, move || {
            mock::array_schema(&a).ok_or_else(|| not_found(&a))
        })
        .await?
    }

    async fn array_slice(
        &self,
        array: &str,
        request: &SliceRequest,
    ) -> Result<SliceResult, StudioError> {
        let (a, r) = (array.to_string(), request.clone());
        apply_one(self.behavior, move || mock::array_slice(&a, &r)).await?
    }
}
