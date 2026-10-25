//! Sync: devices, subscriptions, rejected writes, and activity. Scope is the
//! whole Origin cluster, never the database in the header.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::SyncTab;
use crate::views::sync::activity::ActivityTab;
use crate::views::sync::devices::DevicesTab;
use crate::views::sync::rejects::RejectsTab;
use crate::views::sync::subs::SubsTab;

#[component]
pub fn SyncPage(tab: SyncTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(SyncTab::ALL, |t| t.label(), |tab| Route::Sync { tab }),
            current: Route::Sync { tab },
        }
        match tab {
            SyncTab::Devices => rsx! { DevicesTab {} },
            SyncTab::Subscriptions => rsx! { SubsTab {} },
            SyncTab::RejectedWrites => rsx! { RejectsTab {} },
            SyncTab::Activity => rsx! { ActivityTab {} },
        }
    }
}
