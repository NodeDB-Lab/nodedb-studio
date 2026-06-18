//! Reusable live-tail layout shell (frame + toolbar/body/footer slots). Owns the
//! `.live-tail` structure + CSS so every live screen (CDC, Notify, MV, Console)
//! composes the same chrome instead of re-emitting it. Screen-specific content
//! (rows, status pills, footer stats) is passed in as slot Elements.

use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct LiveTailProps {
    /// Toolbar content (title, status pill, filter, Pause/Export buttons).
    pub toolbar: Element,
    /// Scrolling body (the rows).
    pub body: Element,
    /// Footer content (buffer/lag stats).
    pub footer: Element,
}

#[component]
pub fn LiveTail(props: LiveTailProps) -> Element {
    rsx! {
        div { class: "live-tail",
            div { class: "tail-toolbar", {props.toolbar} }
            div { class: "tail-body", {props.body} }
            div { class: "tail-footer", {props.footer} }
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
    fn renders_three_slots_in_frame() {
        fn app() -> Element {
            rsx! {
                LiveTail {
                    toolbar: rsx! { span { "TB" } },
                    body: rsx! { div { "ROWS" } },
                    footer: rsx! { span { "FT" } },
                }
            }
        }
        let html = render(app);
        assert!(html.contains("live-tail"));
        assert!(html.contains("tail-toolbar"));
        assert!(html.contains("tail-body"));
        assert!(html.contains("tail-footer"));
        assert!(html.contains("TB") && html.contains("ROWS") && html.contains("FT"));
    }
}
