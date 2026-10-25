//! The Spatial states the design shows. Each seed names every tab's state,
//! so one scene's selection or draft never leaks into the next image.
//!
//! A seed holds what a person did, not a result: the pages read their data
//! through the backend, so the screens show computed values.

use crate::data::mock;
use crate::models::spatial::{FeatureRef, SourceRef};
use crate::models::spatial_analyze::{AnalysisForm, Availability};
use crate::state::spatial::{AnalyzeDraft, AnalyzeUi, MapUi, SpatialSeed};

const COLLECTION: &str = "places";
const FIELD: &str = "geometry";
/// Cafe North Point, the feature the Explorer scene selects.
const SELECTED: &str = "place104";

fn source() -> SourceRef {
    SourceRef::new(COLLECTION, FIELD)
}

/// Map: `place104` selected, as the Explorer shows it.
pub fn map() -> SpatialSeed {
    SpatialSeed {
        map: Some(MapUi {
            selected: Some(FeatureRef {
                source: source(),
                id: SELECTED.into(),
            }),
            ..MapUi::default()
        }),
        ..SpatialSeed::default()
    }
}

/// Analyze: Nearby `place104` within 1,000 m, run.
pub fn analyze() -> SpatialSeed {
    let setup = mock::spatial_setup(&Default::default());
    let draft = setup.source(&source()).and_then(|s| {
        let form = AnalysisForm::default();
        let request = form.check(s, &Availability::Supported).ok()?;
        Some(AnalyzeDraft {
            form,
            tried: true,
            pending: Some((1, request)),
            seq: 1,
            ..AnalyzeDraft::default()
        })
    });
    let mut analyze = AnalyzeUi::default();
    if let Some(d) = draft {
        analyze.drafts.insert(source().key(), d);
    }
    SpatialSeed {
        analyze: Some(analyze),
        ..SpatialSeed::default()
    }
}

/// Raster: `climate_cube` at 12:00 UTC, the cell the Explorer reads as 22.4.
pub fn raster() -> SpatialSeed {
    SpatialSeed::default()
}
