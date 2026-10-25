//! Pieces the Administration tabs share.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::feeds::Tone;
use crate::state::admin::ReviewState;
use crate::views::events::common::StatusText;

pub fn dash() -> Element {
    rsx! { span { class: "muted", "—" } }
}

/// A value nobody reported. It is never filled from another field.
pub fn unknown() -> Element {
    rsx! { span { class: "muted", "Unknown" } }
}

/// A value whose full text is its tooltip, cut with an ellipsis in a tight cell.
#[component]
pub fn Cut(text: String, #[props(default)] class: String) -> Element {
    rsx! { span { class: "ad-cut {class}", title: "{text}", "{text}" } }
}

/// One yes-or-no fact with its tone dot.
#[component]
pub fn Check(ok: bool, yes: String, no: String) -> Element {
    let tone = if ok { Tone::Ok } else { Tone::Err };
    rsx! { StatusText { tone, text: if ok { yes } else { no } } }
}

/// The review of one change: what changes, what to weigh, and the button
/// that applies it. Nothing applies until that button.
#[component]
pub fn ReviewSheet(
    state: ReviewState,
    on_back: EventHandler<()>,
    on_apply: EventHandler<()>,
    #[props(default = "Back".to_string())] back_label: String,
) -> Element {
    let r = &state.review;
    let (granted, required) = (r.permission.granted, r.permission.required.clone());
    rsx! {
        Sheet {
            title: r.title.clone(),
            width: SheetWidth::Medium,
            on_close: move |_| on_back.call(()),
            footer: rsx! {
                SheetFooter {
                    error: state.error.clone(),
                    hint: Some("Nothing changes until you apply. Applying records an audit event.".to_string()),
                    button { class: "btn", disabled: state.busy, onclick: move |_| on_back.call(()), "{back_label}" }
                    button {
                        class: "btn primary",
                        disabled: state.busy || !r.can_apply(),
                        onclick: move |_| on_apply.call(()),
                        "{r.apply_label}"
                    }
                }
            },
            div { class: "review-feed",
                div { class: "review-block",
                    div { class: "kv",
                        div { class: "kv-row", span { "Target" } span { class: "mono", "{r.target}" } }
                    }
                    if !r.lines.is_empty() {
                        div { class: "dt-wrap",
                            table { class: "dt cl-table ad-review-table",
                                thead {
                                    tr {
                                        th { "Change" }
                                        th { "Before" }
                                        th { "After" }
                                    }
                                }
                                tbody {
                                    for (i , l) in r.lines.iter().enumerate() {
                                        tr { key: "{i}",
                                            td { class: "key", title: "{l.label}", "{l.label}" }
                                            td {
                                                match l.before.as_ref() {
                                                    Some(b) => rsx! { span { title: "{b}", "{b}" } },
                                                    None => dash(),
                                                }
                                            }
                                            td { title: "{l.after}", "{l.after}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "kv",
                        div { class: "kv-row",
                            span { "Required permission" }
                            span { class: "status",
                                span { class: if granted { "status-dot ok" } else { "status-dot err" } }
                                span { class: "mono", "{required}" }
                                if granted { " granted" } else { " not granted" }
                            }
                        }
                        div { class: "kv-row",
                            span { "Revision read" }
                            span { class: "num", "{r.revision}" }
                        }
                    }
                }
                for n in r.notes.iter() {
                    div { key: "{n}", class: "callout info",
                        Ico { icon: Icon::Info, size: 16 }
                        span { "{n}" }
                    }
                }
                for w in r.warnings.iter() {
                    div { key: "{w}", class: "callout warn",
                        Ico { icon: Icon::TriangleAlert, size: 16 }
                        span { "{w}" }
                    }
                }
                for e in r.errors.iter() {
                    div { key: "{e}", class: "callout err", role: "alert",
                        Ico { icon: Icon::CircleAlert, size: 16 }
                        span { "{e}" }
                    }
                }
                if !granted {
                    div { class: "callout err", role: "alert",
                        Ico { icon: Icon::Lock, size: 16 }
                        span { "This connection lacks {required}. Ask an administrator for it. Nothing can be applied here." }
                    }
                }
            }
        }
    }
}
