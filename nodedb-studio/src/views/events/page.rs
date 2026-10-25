//! Events & Jobs: application data events and scheduled database work.
//!
//! Each tab keeps its selection, filters, and open sheet in `EventsUi`, above
//! the router, so leaving the page does not lose them. A change feed, a
//! topic, a notification, and a scheduled job are different objects with
//! different delivery rules, and each has its own tab.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::EventsTab;
use crate::views::events::feeds::FeedsTab;
use crate::views::events::jobs::JobsTab;
use crate::views::events::matviews::MatViewsTab;
use crate::views::events::notify::NotifyTab;
use crate::views::events::topics::TopicsTab;

#[component]
pub fn EventsPage(tab: EventsTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(EventsTab::ALL, |t| t.label(), |tab| Route::Events { tab }),
            current: Route::Events { tab },
        }
        match tab {
            EventsTab::ChangeFeeds => rsx! { FeedsTab {} },
            EventsTab::MaterializedViews => rsx! { MatViewsTab {} },
            EventsTab::Topics => rsx! { TopicsTab {} },
            EventsTab::Notifications => rsx! { NotifyTab {} },
            EventsTab::ScheduledJobs => rsx! { JobsTab {} },
        }
    }
}
