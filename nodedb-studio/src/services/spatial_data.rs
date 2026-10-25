//! Spatial reads at the backend seam: the geometry sources and rasters a
//! connection holds, a bounded viewport read, one feature, a validity check,
//! an analysis, and a raster band.
//!
//! A read names its source: a geometry field of a collection, or an array.
//! Geometry keeps the SRID it was stored with. A server measures on the
//! source CRS's ellipsoid, and a read that cannot run says why.

use async_trait::async_trait;

use crate::models::geo_ops::Validity;
use crate::models::spatial::{
    Feature, FeatureRef, RasterRequest, RasterSlice, SpatialSetup, Viewport, ViewportRequest,
};
use crate::models::spatial_analyze::{AnalysisRequest, AnalysisResult};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait SpatialData {
    /// The geometry sources, the georeferenced arrays, and the operations
    /// the connection runs.
    async fn spatial_setup(&self) -> Result<SpatialSetup, StudioError>;

    /// The features of each source that touch the box, at most `limit` per
    /// source. A page also counts every feature the box holds, so a cut
    /// result never reads as complete.
    async fn spatial_viewport(&self, request: &ViewportRequest) -> Result<Viewport, StudioError>;

    /// One feature with all its values. `RecordMissing` when it is gone.
    async fn spatial_feature(&self, feature: &FeatureRef) -> Result<Feature, StudioError>;

    /// `ST_IsValid` and `ST_IsValidReason` on one feature's stored geometry.
    async fn spatial_validity(&self, feature: &FeatureRef) -> Result<Validity, StudioError>;

    /// Run a checked analysis. Rows are ordered by the declared measure, and
    /// `matching` counts every match before the limit.
    async fn spatial_analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisResult, StudioError>;

    /// One band of a georeferenced array at one slice, as raw values.
    async fn raster_slice(&self, request: &RasterRequest) -> Result<RasterSlice, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::geo_ops::Bbox;
    use crate::models::spatial::SourceRef;
    use crate::models::spatial_analyze::{AnalysisForm, Availability};
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    fn places() -> SourceRef {
        SourceRef::new("places", "geometry")
    }

    fn viewport(limit: u32) -> ViewportRequest {
        ViewportRequest {
            sources: vec![places()],
            bbox: Bbox::new(-74.05, 40.68, -73.89, 40.81),
            limit,
        }
    }

    async fn nearby(svc: &MockConnectionService) -> AnalysisRequest {
        let setup = svc.spatial_setup().await.expect("setup");
        let source = setup.source(&places()).expect("source").clone();
        AnalysisForm::default()
            .check(&source, &Availability::Supported)
            .expect("valid")
    }

    #[tokio::test]
    async fn setup_lists_the_source_and_the_raster_apart() {
        let svc = MockConnectionService::ready();
        let setup = svc.spatial_setup().await.expect("setup");
        let mut keys: Vec<String> = setup.sources.iter().map(|s| s.source.key()).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n, "every source needs a stable key");
        assert_eq!(setup.rasters.len(), 1);
        assert!(
            setup
                .sources
                .iter()
                .all(|s| s.source.collection != "climate_cube")
        );
    }

    #[tokio::test]
    async fn setup_empty_has_no_sources() {
        let svc = MockConnectionService::empty();
        let setup = svc.spatial_setup().await.expect("setup");
        assert!(setup.sources.is_empty() && setup.rasters.is_empty());
    }

    #[tokio::test]
    async fn setup_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.spatial_setup().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn viewport_ready_has_unique_feature_ids() {
        let svc = MockConnectionService::ready();
        let v = svc
            .spatial_viewport(&viewport(500))
            .await
            .expect("viewport");
        let page = v.page(&places()).expect("page");
        let mut ids: Vec<&str> = page.features.iter().map(|f| f.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "every feature needs a stable key");
        assert_eq!(n, 104);
        assert_eq!(v.request, viewport(500));
    }

    #[tokio::test]
    async fn viewport_limit_cuts_features_and_keeps_the_count() {
        let svc = MockConnectionService::ready();
        let v = svc.spatial_viewport(&viewport(10)).await.expect("viewport");
        let page = v.page(&places()).expect("page");
        assert_eq!((page.features.len(), page.matching), (10, 104));
        assert!(page.capped());
    }

    #[tokio::test]
    async fn viewport_empty_returns_no_features() {
        let svc = MockConnectionService::empty();
        let v = svc
            .spatial_viewport(&viewport(100))
            .await
            .expect("viewport");
        let page = v.page(&places()).expect("page");
        assert!(page.features.is_empty() && page.matching == 0);
    }

    #[tokio::test]
    async fn viewport_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.spatial_viewport(&viewport(100)).await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn feature_ready_reads_the_explorers_record() {
        let svc = MockConnectionService::ready();
        let f = svc
            .spatial_feature(&FeatureRef {
                source: places(),
                id: "place104".into(),
            })
            .await
            .expect("feature");
        assert_eq!(f.title, "Cafe North Point");
        let err = svc
            .spatial_feature(&FeatureRef {
                source: places(),
                id: "nope".into(),
            })
            .await
            .expect_err("missing");
        assert!(matches!(err, StudioError::RecordMissing { .. }));
    }

    #[tokio::test]
    async fn feature_and_validity_erroring_are_err() {
        let svc = MockConnectionService::erroring();
        let r = FeatureRef {
            source: places(),
            id: "place104".into(),
        };
        let a = AsyncState::from_value(Some(svc.spatial_feature(&r).await));
        let b = AsyncState::from_value(Some(svc.spatial_validity(&r).await));
        assert!(a.error_message().is_some() && b.error_message().is_some());
    }

    #[tokio::test]
    async fn validity_ready_checks_the_stored_geometry() {
        let svc = MockConnectionService::ready();
        let v = svc
            .spatial_validity(&FeatureRef {
                source: places(),
                id: "place104".into(),
            })
            .await
            .expect("validity");
        assert_eq!(v, Validity::Valid);
    }

    #[tokio::test]
    async fn analyze_ready_ranks_with_unique_ids() {
        let svc = MockConnectionService::ready();
        let req = nearby(&svc).await;
        let r = svc.spatial_analyze(&req).await.expect("result");
        let mut ids: Vec<&str> = r.rows.iter().map(|row| row.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "every row needs a stable key");
        assert_eq!(r.request, req, "the result names the request it answers");
        assert_eq!(r.columns.len(), r.rows[0].cells.len());
    }

    #[tokio::test]
    async fn analyze_empty_returns_no_rows() {
        let svc = MockConnectionService::empty();
        let req = nearby(&MockConnectionService::ready()).await;
        let r = svc.spatial_analyze(&req).await.expect("result");
        assert!(r.rows.is_empty() && r.matching == 0);
    }

    #[tokio::test]
    async fn analyze_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let req = nearby(&MockConnectionService::ready()).await;
        let s = AsyncState::from_value(Some(svc.spatial_analyze(&req).await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn raster_ready_reads_a_full_grid() {
        let svc = MockConnectionService::ready();
        let s = svc
            .raster_slice(&RasterRequest {
                array: "climate_cube".into(),
                slice_index: 12,
                band: "temperature_c".into(),
            })
            .await
            .expect("slice");
        assert_eq!(s.raw.len(), (s.rows * s.cols) as usize);
    }

    #[tokio::test]
    async fn raster_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(
            svc.raster_slice(&RasterRequest {
                array: "climate_cube".into(),
                slice_index: 12,
                band: "temperature_c".into(),
            })
            .await,
        ));
        assert!(s.error_message().is_some());
    }
}
