//! The Timeseries states the design shows. Each seed names every tab's
//! state, so one scene's open document or open drawer never leaks into the
//! next image.
//!
//! A seed holds what a person did, not a result: the pages read their data
//! through the backend, so the screens show computed values.

use crate::state::timeseries::{AfterOpen, DashUi, ExploreUi, LifeUi, TimeseriesSeed};

/// Explore with the Collection picker open, as the design shows.
pub fn explore() -> TimeseriesSeed {
    TimeseriesSeed {
        explore: Some(ExploreUi {
            picker_open: true,
            ..ExploreUi::default()
        }),
        ..TimeseriesSeed::default()
    }
}

/// Dashboards with Sales analysis selected and open. Its second panel was
/// resized, so the document is modified.
pub fn dashboards() -> TimeseriesSeed {
    TimeseriesSeed {
        dash: Some(DashUi {
            selected: Some("dash-1".into()),
            open_id: Some("dash-1".into()),
            after_open: Some(AfterOpen {
                resize: Some(("p2".into(), 262)),
                edit: None,
            }),
            ..DashUi::default()
        }),
        ..TimeseriesSeed::default()
    }
}

/// The panel editor drawer, open on the first panel of Sales analysis.
pub fn dashboard_editor() -> TimeseriesSeed {
    TimeseriesSeed {
        dash: Some(DashUi {
            selected: Some("dash-1".into()),
            open_id: Some("dash-1".into()),
            after_open: Some(AfterOpen {
                resize: Some(("p2".into(), 262)),
                edit: Some("p1".into()),
            }),
            ..DashUi::default()
        }),
        ..TimeseriesSeed::default()
    }
}

/// Lifecycle of `sales` with `sales_hourly` selected.
pub fn lifecycle() -> TimeseriesSeed {
    TimeseriesSeed {
        life: Some(LifeUi {
            selected: Some("sales_hourly".into()),
            ..LifeUi::default()
        }),
        ..TimeseriesSeed::default()
    }
}

/// The backfill review for `sales_daily`, June 1 to 8: the page runs the dry
/// run when it opens.
pub fn backfill() -> TimeseriesSeed {
    TimeseriesSeed::default()
}
