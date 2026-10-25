//! Spatial reads over the Explorer's records and the `climate_cube` array.
//! A record the Explorer edits or deletes changes here too: both read the
//! service's overlay.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::geo_ops::Validity;
use crate::models::spatial::{
    Feature, FeatureRef, RasterRequest, RasterSlice, SpatialSetup, Viewport, ViewportRequest,
};
use crate::models::spatial_analyze::{AnalysisRequest, AnalysisResult};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one_result};
use crate::services::spatial_data::SpatialData;

#[async_trait(?Send)]
impl SpatialData for MockConnectionService {
    async fn spatial_setup(&self) -> Result<SpatialSetup, StudioError> {
        let mut setup = mock::spatial_setup(&self.records.borrow());
        if self.behavior == MockBehavior::Empty {
            setup.sources.clear();
            setup.rasters.clear();
        }
        apply_one_result(self.behavior, Ok(setup)).await
    }

    async fn spatial_viewport(&self, request: &ViewportRequest) -> Result<Viewport, StudioError> {
        let mut read = mock::spatial_viewport(&self.records.borrow(), request);
        if self.behavior == MockBehavior::Empty
            && let Ok(v) = &mut read
        {
            for page in &mut v.layers {
                page.features.clear();
                page.matching = 0;
            }
        }
        apply_one_result(self.behavior, read).await
    }

    async fn spatial_feature(&self, feature: &FeatureRef) -> Result<Feature, StudioError> {
        let read = mock::spatial_feature(&self.records.borrow(), feature);
        apply_one_result(self.behavior, read).await
    }

    async fn spatial_validity(&self, feature: &FeatureRef) -> Result<Validity, StudioError> {
        let read = mock::spatial_validity(&self.records.borrow(), feature);
        apply_one_result(self.behavior, read).await
    }

    async fn spatial_analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisResult, StudioError> {
        let mut read = mock::spatial_analyze(&self.records.borrow(), request);
        if self.behavior == MockBehavior::Empty
            && let Ok(r) = &mut read
        {
            r.rows.clear();
            r.matching = 0;
        }
        apply_one_result(self.behavior, read).await
    }

    async fn raster_slice(&self, request: &RasterRequest) -> Result<RasterSlice, StudioError> {
        apply_one_result(self.behavior, mock::raster_slice(request)).await
    }
}
