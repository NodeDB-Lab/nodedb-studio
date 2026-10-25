//! A sheet: a drawer that slides over the right of a workbench to hold a form
//! or a review. It has the panel header (grip, title, close), a scrolling
//! body, and a footer for the actions.
//!
//! A sheet is not a closable panel and the View menu does not list it. It
//! holds work in progress, so its owner decides when it closes.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};

/// How much of the workbench the sheet covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetWidth {
    /// 520px: a short form.
    #[default]
    Narrow,
    /// 760px: a form with a review table.
    Medium,
    /// Up to 1120px: a form beside its preview.
    Wide,
}

impl SheetWidth {
    fn class(self) -> &'static str {
        match self {
            SheetWidth::Narrow => "narrow",
            SheetWidth::Medium => "medium",
            SheetWidth::Wide => "wide",
        }
    }
}

#[component]
pub fn Sheet(
    title: String,
    on_close: EventHandler<()>,
    #[props(default)] width: SheetWidth,
    #[props(default)] class: String,
    /// The actions at the foot of the sheet. Empty omits the footer.
    #[props(default = VNode::empty())]
    footer: Element,
    children: Element,
) -> Element {
    let w = width.class();
    rsx! {
        aside {
            class: "sheet events-sheet {w} {class}",
            "aria-label": "{title}",
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    e.stop_propagation();
                    on_close.call(());
                }
            },
            header { class: "panel-header",
                span { class: "panel-grip", "aria-hidden": "true",
                    Ico { icon: Icon::GripVertical, size: 16 }
                }
                h2 { class: "panel-title", "{title}" }
                div { class: "panel-actions" }
                button {
                    class: "icon-btn panel-close",
                    title: "Close",
                    "aria-label": "Close {title}",
                    onclick: move |_| on_close.call(()),
                    Ico { icon: Icon::X, size: 16 }
                }
            }
            div { class: "sheet-body", {children} }
            {footer}
        }
    }
}

/// The footer row of a sheet: a hint or an error on the left, the actions on
/// the right.
#[component]
pub fn SheetFooter(
    /// Something the person must read before acting, shown in red.
    error: Option<String>,
    /// What the actions will do, shown muted.
    hint: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        footer { class: "sheet-footer",
            if let Some(e) = error {
                div { class: "field-error", role: "alert", "{e}" }
            } else if let Some(h) = hint {
                span { class: "sheet-hint", "{h}" }
            }
            div { class: "toolbar-spacer" }
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn a_sheet_names_itself_and_holds_its_footer() {
        fn app() -> Element {
            rsx! {
                Sheet {
                    title: "Create topic".to_string(),
                    on_close: |_| {},
                    footer: rsx! {
                        SheetFooter { error: None, hint: Some("Nothing is created yet.".to_string()),
                            button { "Create" }
                        }
                    },
                    div { "body" }
                }
            }
        }
        let html = render(app);
        assert!(html.contains("aria-label=\"Create topic\""), "{html}");
        assert!(html.contains("Close Create topic"), "{html}");
        assert!(html.contains("Nothing is created yet."), "{html}");
        assert!(html.contains("sheet-footer"), "{html}");
    }

    #[test]
    fn an_error_replaces_the_hint() {
        fn app() -> Element {
            rsx! {
                SheetFooter { error: Some("Name the topic.".to_string()), hint: Some("hint".to_string()) }
            }
        }
        let html = render(app);
        assert!(html.contains("Name the topic."), "{html}");
        assert!(!html.contains(">hint<"), "{html}");
    }
}
