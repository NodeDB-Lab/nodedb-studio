//! A button that opens another tool. It stays disabled, with the reason,
//! when the active connection lacks the capability the tool needs.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::routes::Route;
use crate::state::connection::ActiveConnection;

#[component]
pub fn OpenLink(
    route: Route,
    label: String,
    icon: Icon,
    #[props(default = "link-btn".to_string())] class: String,
) -> Element {
    let nav = use_navigator();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let missing = route.destination().required_cap().filter(|cap| {
        !active
            .read()
            .as_ref()
            .is_some_and(|c| c.capabilities.has(*cap))
    });
    let title = missing
        .map(|cap| format!("{label} needs the {cap:?} capability, which this connection lacks"))
        .unwrap_or_default();
    rsx! {
        button {
            class: "{class}",
            disabled: missing.is_some(),
            title: "{title}",
            onclick: move |_| { nav.push(route.clone()); },
            Ico { icon, size: 16 }
            "{label}"
        }
    }
}
