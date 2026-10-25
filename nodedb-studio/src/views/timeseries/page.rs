//! Timeseries: Explore, Dashboards, and Lifecycle over user collections.
//! Server metrics are not here: they live in Cluster → Monitoring.
//!
//! Backfill is the job review Lifecycle opens. It is a route of its own so
//! it can be addressed and returned to, and it lights the Lifecycle tab.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::TimeseriesTab;
use crate::views::timeseries::actions::ExploreActions;
use crate::views::timeseries::backfill::BackfillTab;
use crate::views::timeseries::dashboards::DashboardsTab;
use crate::views::timeseries::explore::ExploreTab;
use crate::views::timeseries::lifecycle::LifecycleTab;

/// The tabs the strip shows. Backfill belongs to Lifecycle.
const STRIP: &[TimeseriesTab] = &[
    TimeseriesTab::Explore,
    TimeseriesTab::Dashboards,
    TimeseriesTab::Lifecycle,
];

/// The strip tab a page lights.
pub fn strip_tab(tab: TimeseriesTab) -> TimeseriesTab {
    match tab {
        TimeseriesTab::Backfill => TimeseriesTab::Lifecycle,
        other @ (TimeseriesTab::Explore | TimeseriesTab::Dashboards | TimeseriesTab::Lifecycle) => {
            other
        }
    }
}

#[component]
pub fn TimeseriesPage(tab: TimeseriesTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(STRIP, |t| t.label(), |tab| Route::Timeseries { tab }),
            current: Route::Timeseries { tab: strip_tab(tab) },
            actions: rsx! {
                if tab == TimeseriesTab::Explore {
                    ExploreActions {}
                }
            },
        }
        match tab {
            TimeseriesTab::Explore => rsx! { ExploreTab {} },
            TimeseriesTab::Dashboards => rsx! { DashboardsTab {} },
            TimeseriesTab::Lifecycle => rsx! { LifecycleTab {} },
            TimeseriesTab::Backfill => rsx! { BackfillTab {} },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backfill_lights_lifecycle_and_every_other_tab_lights_itself() {
        assert_eq!(strip_tab(TimeseriesTab::Backfill), TimeseriesTab::Lifecycle);
        for t in STRIP {
            assert_eq!(strip_tab(*t), *t);
        }
    }

    #[test]
    fn the_strip_omits_only_the_review_route() {
        for t in TimeseriesTab::ALL {
            assert_eq!(STRIP.contains(t), *t != TimeseriesTab::Backfill);
        }
    }
}
