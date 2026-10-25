//! Spatial reads. The real client does not expose them yet.

use async_trait::async_trait;

use crate::models::geo_ops::Validity;
use crate::models::spatial::{
    Feature, FeatureRef, RasterRequest, RasterSlice, SpatialSetup, Viewport, ViewportRequest,
};
use crate::models::spatial_analyze::{AnalysisRequest, AnalysisResult};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::spatial_data::SpatialData;

#[async_trait(?Send)]
impl SpatialData for NodeDbConnectionService {
    async fn spatial_setup(&self) -> Result<SpatialSetup, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn spatial_viewport(&self, _request: &ViewportRequest) -> Result<Viewport, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn spatial_feature(&self, _feature: &FeatureRef) -> Result<Feature, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn spatial_validity(&self, _feature: &FeatureRef) -> Result<Validity, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn spatial_analyze(
        &self,
        _request: &AnalysisRequest,
    ) -> Result<AnalysisResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn raster_slice(&self, _request: &RasterRequest) -> Result<RasterSlice, StudioError> {
        Err(StudioError::NotConnected)
    }
}
