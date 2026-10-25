//! Administration: the tabs and the page each one shows.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::AdminTab;
use crate::views::admin::access::AccessTab;
use crate::views::admin::audit::AuditTab;
use crate::views::admin::backups::BackupsTab;
use crate::views::admin::config::ConfigTab;
use crate::views::admin::credentials::CredentialsTab;
use crate::views::admin::policies::PoliciesTab;

#[component]
pub fn AdminPage(tab: AdminTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(AdminTab::ALL, |t| t.label(), |tab| Route::Admin { tab }),
            current: Route::Admin { tab },
        }
        match tab {
            AdminTab::Access => rsx! { AccessTab {} },
            AdminTab::RowPolicies => rsx! { PoliciesTab {} },
            AdminTab::Audit => rsx! { AuditTab {} },
            AdminTab::Backups => rsx! { BackupsTab {} },
            AdminTab::Config => rsx! { ConfigTab {} },
            AdminTab::Credentials => rsx! { CredentialsTab {} },
        }
    }
}
