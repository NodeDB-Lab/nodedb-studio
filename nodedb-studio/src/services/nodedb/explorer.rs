//! Catalog, record, and array access.

use async_trait::async_trait;
use nodedb_types::Value;

use crate::models::array_data::{ArraySchema, SliceRequest, SliceResult};
use crate::models::collection::Collection;
use crate::models::graph_records::{EdgePage, EdgeQuery};
use crate::models::records::{CollectionAccess, Record, RecordPage, RecordQuery};
use crate::models::schema::CollectionSchema;
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl ExplorerData for NodeDbConnectionService {
    async fn catalog(&self) -> Result<Vec<Collection>, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn collection_schema(&self, _collection: &str) -> Result<CollectionSchema, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn collection_access(&self, _collection: &str) -> Result<CollectionAccess, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn records(
        &self,
        _collection: &str,
        _query: &RecordQuery,
    ) -> Result<RecordPage, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn record(&self, _collection: &str, _id: &str) -> Result<Record, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn insert_record(
        &self,
        _collection: &str,
        _id: Option<&str>,
        _value: Value,
    ) -> Result<Record, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn update_record(
        &self,
        _collection: &str,
        _id: &str,
        _expected_revision: u64,
        _value: Value,
    ) -> Result<Record, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn delete_record(
        &self,
        _collection: &str,
        _id: &str,
        _expected_revision: u64,
    ) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn edges(&self, _graph: &str, _query: &EdgeQuery) -> Result<EdgePage, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn delete_edge(&self, _graph: &str, _id: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn array_schema(&self, _array: &str) -> Result<ArraySchema, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn array_slice(
        &self,
        _array: &str,
        _request: &SliceRequest,
    ) -> Result<SliceResult, StudioError> {
        Err(StudioError::NotConnected)
    }
}
