//! One line that says what the last action did. Errors announce
//! themselves. An undoable action offers Undo on the same line.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::state::timeseries::Notice;

#[component]
pub fn TsNotice(
    notice: Option<Notice>,
    on_dismiss: EventHandler<()>,
    #[props(default)] undo: bool,
    #[props(default)] on_undo: EventHandler<()>,
) -> Element {
    let Some(n) = notice else {
        return rsx! {};
    };
    let (class, icon) = if n.error {
        ("callout err notice-line", Icon::CircleAlert)
    } else {
        ("callout ok notice-line", Icon::CircleCheck)
    };
    rsx! {
        div { class: "{class}", role: if n.error { "alert" } else { "status" },
            Ico { icon, size: 16 }
            span { class: "notice-text", "{n.text}" }
            if undo {
                button { class: "btn sm", onclick: move |_| on_undo.call(()), "Undo" }
            }
            button {
                class: "icon-btn sm",
                title: "Dismiss",
                "aria-label": "Dismiss message",
                onclick: move |_| on_dismiss.call(()),
                Ico { icon: Icon::X, size: 16 }
            }
        }
    }
}
