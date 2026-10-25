//! Route entry points while the redesign replaces each area.
//!
//! Every route renders a `*Page` component named here. An area's redesign
//! moves its page into the area's own module and removes it from this file.
//! When this file is empty it goes.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::AdminTab;
use crate::views::admin::audit::AuditView;
use crate::views::admin::rbac::RbacView;
use crate::views::admin::rls::RlsView;

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
