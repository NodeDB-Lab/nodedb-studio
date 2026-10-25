//! Pieces every Events & Jobs tab uses.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::console::Protocol;
use crate::models::feeds::Tone;
use crate::models::sql_text::tokenize;
use crate::state::events::feeds::Notice;

/// A status word with its color dot. The word always shows.
#[component]
pub fn StatusText(tone: Tone, text: String) -> Element {
    rsx! {
        span { class: "status",
            span { class: tone.dot() }
            "{text}"
        }
    }
}

/// A status badge: the word on its tone.
#[component]
pub fn StatusBadge(tone: Tone, text: String) -> Element {
    rsx! {
        span { class: tone.badge(),
            span { class: tone.dot() }
            "{text}"
        }
    }
}

/// One line that says what the last action did. Errors announce themselves.
#[component]
pub fn NoticeLine(notice: Option<Notice>, on_dismiss: EventHandler<()>) -> Element {
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

/// SQL text with its keywords colored, read-only.
#[component]
pub fn SqlBlock(sql: String, #[props(default)] class: String) -> Element {
    rsx! {
        pre { class: "code-block sql-block {class}",
            for (i, t) in tokenize(&sql, Protocol::Sql).iter().enumerate() {
                span { key: "{i}", class: "{t.kind.class()}", "{t.text}" }
            }
        }
    }
}

/// A labeled form control. `error` shows under it when set.
#[component]
pub fn FormField(
    label: String,
    id: String,
    #[props(default)] hint: Option<String>,
    #[props(default)] error: Option<String>,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    rsx! {
        div { class: "field {class}",
            label { r#for: "{id}", "{label}" }
            {children}
            if let Some(e) = error {
                div { class: "error", role: "alert", "{e}" }
            } else if let Some(h) = hint {
                div { class: "hint", "{h}" }
            }
        }
    }
}

/// An info icon with its explanation on hover and for assistive technology.
#[component]
pub fn InfoTip(text: String) -> Element {
    rsx! {
        span { class: "info-tip", title: "{text}", role: "img", "aria-label": "{text}",
            Ico { icon: Icon::Info, size: 16 }
        }
    }
}
