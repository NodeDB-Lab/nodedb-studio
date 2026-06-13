//! Streams & Events. The `tab` route segment selects the screen: `landing`
//! shows the capability cards; the others render the shared lateral nav
//! (`Snav`) plus that sub-screen's content.

use dioxus::prelude::*;

use crate::components::snav::Snav;
use crate::views::streams::cdc::StreamsCdc;
use crate::views::streams::cron::StreamsCron;
use crate::views::streams::landing::StreamsLanding;
use crate::views::streams::mv::StreamsMv;
use crate::views::streams::notify::StreamsNotify;
use crate::views::streams::topics::StreamsTopics;

#[component]
pub fn Streams(tab: String) -> Element {
    if tab == "landing" {
        return rsx! {
            div { class: "view active", StreamsLanding {} }
        };
    }

    rsx! {
        div { class: "view active",
            div { class: "streams-sub-shell",
                Snav { active: tab.clone() }
                match tab.as_str() {
                    "mv" => rsx! { StreamsMv {} },
                    "topics" => rsx! { StreamsTopics {} },
                    "notify" => rsx! { StreamsNotify {} },
                    "cron" => rsx! { StreamsCron {} },
                    // "cdc" and any unknown sub-tab land on CDC.
                    _ => rsx! { StreamsCdc {} },
                }
            }
        }
    }
}
