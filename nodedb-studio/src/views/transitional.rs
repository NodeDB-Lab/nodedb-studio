//! Route entry points while the redesign replaces each area.
//!
//! Every route renders a `*Page` component named here. An area's redesign
//! moves its page into the area's own module and removes it from this file.
//! When this file is empty it goes.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::{
    AdminTab, EventsTab, GraphTab, SearchTab, SpatialTab, SyncTab, TimeseriesTab, VectorTab,
};
use crate::views::admin::audit::AuditView;
use crate::views::admin::rbac::RbacView;
use crate::views::admin::rls::RlsView;
use crate::views::console::Console;
use crate::views::designer::Designer;
use crate::views::explorer::Explorer;
use crate::views::fts_inspector::FtsInspector;
use crate::views::graph_explorer::GraphExplorer;
use crate::views::query::Query;
use crate::views::spatial_view::SpatialView;
use crate::views::streams::cdc::StreamsCdc;
use crate::views::streams::cron::StreamsCron;
use crate::views::streams::mv::StreamsMv;
use crate::views::streams::notify::StreamsNotify;
use crate::views::streams::topics::StreamsTopics;
use crate::views::sync::Sync;
use crate::views::timeseries_dashboard::TimeseriesDashboard;
use crate::views::vector_space::VectorSpace;

/// A page that has not been rebuilt yet.
#[component]
fn Unbuilt(title: &'static str) -> Element {
    rsx! {
        div { class: "page-body",
            div { class: "empty-state",
                h2 { "{title}" }
                p { "This page is being rebuilt for the new workspace." }
            }
        }
    }
}

#[component]
pub fn ExplorerPage() -> Element {
    rsx! { div { class: "legacy-page", Explorer {} } }
}

#[component]
pub fn DataTransferPage() -> Element {
    rsx! { Unbuilt { title: "Import and export" } }
}

#[component]
pub fn QueryPage() -> Element {
    rsx! { div { class: "legacy-page", Query {} } }
}

#[component]
pub fn DesignerPage() -> Element {
    rsx! { div { class: "legacy-page", Designer {} } }
}

#[component]
pub fn EventsPage(tab: EventsTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(EventsTab::ALL, |t| t.label(), |tab| Route::Events { tab }),
            current: Route::Events { tab },
        }
        div { class: "legacy-page",
            match tab {
                EventsTab::ChangeFeeds => rsx! { StreamsCdc {} },
                EventsTab::MaterializedViews => rsx! { StreamsMv {} },
                EventsTab::Topics => rsx! { StreamsTopics {} },
                EventsTab::Notifications => rsx! { StreamsNotify {} },
                EventsTab::ScheduledJobs => rsx! { StreamsCron {} },
            }
        }
    }
}

#[component]
pub fn GraphPage(tab: GraphTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(GraphTab::ALL, |t| t.label(), |tab| Route::Graph { tab }),
            current: Route::Graph { tab },
        }
        div { class: "legacy-page", GraphExplorer {} }
    }
}

#[component]
pub fn VectorPage(tab: VectorTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(VectorTab::ALL, |t| t.label(), |tab| Route::Vector { tab }),
            current: Route::Vector { tab },
        }
        div { class: "legacy-page", VectorSpace {} }
    }
}

#[component]
pub fn TimeseriesPage(tab: TimeseriesTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(TimeseriesTab::ALL, |t| t.label(), |tab| Route::Timeseries { tab }),
            current: Route::Timeseries { tab },
        }
        div { class: "legacy-page", TimeseriesDashboard {} }
    }
}

#[component]
pub fn SpatialPage(tab: SpatialTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(SpatialTab::ALL, |t| t.label(), |tab| Route::Spatial { tab }),
            current: Route::Spatial { tab },
        }
        div { class: "legacy-page", SpatialView {} }
    }
}

#[component]
pub fn SearchPage(tab: SearchTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(SearchTab::ALL, |t| t.label(), |tab| Route::Search { tab }),
            current: Route::Search { tab },
        }
        div { class: "legacy-page", FtsInspector {} }
    }
}

#[component]
pub fn SyncPage(tab: SyncTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(SyncTab::ALL, |t| t.label(), |tab| Route::Sync { tab }),
            current: Route::Sync { tab },
        }
        div { class: "legacy-page", Sync {} }
    }
}

#[component]
pub fn AdminPage(tab: AdminTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(AdminTab::ALL, |t| t.label(), |tab| Route::Admin { tab }),
            current: Route::Admin { tab },
        }
        div { class: "legacy-page",
            match tab {
                AdminTab::Access => rsx! { RbacView {} },
                AdminTab::RowPolicies => rsx! { RlsView {} },
                AdminTab::Audit => rsx! { AuditView {} },
                AdminTab::Backups => rsx! { Unbuilt { title: "Backups" } },
                AdminTab::Config => rsx! { Unbuilt { title: "Server config" } },
                AdminTab::Credentials => rsx! { Unbuilt { title: "Credentials" } },
            }
        }
    }
}

#[component]
pub fn ConsolePage() -> Element {
    rsx! { div { class: "legacy-page", Console {} } }
}
