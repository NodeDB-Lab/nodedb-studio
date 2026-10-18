//! Reusable detail-panel shell (header + body/footer slots) for the Explorer's
//! master-detail viewers. Owns the `.detail-panel` structure + CSS so every
//! engine viewer composes the same chrome. The body (JSON tree, row fields,
//! vector preview, …) and the footer actions differ per viewer and are passed
//! in as slot Elements. Place it inside `.list-content.with-detail`.

use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct DetailPanelProps {
    /// Record identifier shown in the header (e.g. a document `_id`).
    pub title: String,
    /// Age of the shown data, pre-formatted for display ("8s", "2m").
    pub freshness: String,
    /// Marks the data as old enough to warn about (`.freshness.stale`).
    #[props(default = false)]
    pub stale: bool,
    pub on_refresh: EventHandler<()>,
    pub on_close: EventHandler<()>,
    /// Scrolling body content.
    pub body: Element,
    /// Footer actions. Wrap trailing actions in `span { class: "right" }`.
    pub footer: Element,
}

#[component]
pub fn DetailPanel(props: DetailPanelProps) -> Element {
    let freshness_class = if props.stale {
        "freshness stale"
    } else {
        "freshness"
    };
    rsx! {
        aside { class: "detail-panel",
            div { class: "detail-header",
                span { class: "title", "{props.title}" }
                div { class: "detail-header-right",
                    button {
                        class: freshness_class,
                        title: "Refreshed {props.freshness} ago · click to refresh",
                        onclick: move |_| props.on_refresh.call(()),
                        "↻ "
                        span { "{props.freshness}" }
                    }
                    button {
                        class: "close-x",
                        title: "Close detail",
                        aria_label: "Close detail",
                        onclick: move |_| props.on_close.call(()),
                        "×"
                    }
                }
            }
            div { class: "detail-body", {props.body} }
            div { class: "detail-footer", {props.footer} }
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
    fn renders_header_and_both_slots() {
        fn app() -> Element {
            rsx! {
                DetailPanel {
                    title: "evt_01H8",
                    freshness: "8s",
                    on_refresh: |_| {},
                    on_close: |_| {},
                    body: rsx! { div { "BODY" } },
                    footer: rsx! { button { "Edit" } },
                }
            }
        }
        let html = render(app);
        assert!(
            html.contains(r#"<span class="title">evt_01H8</span>"#),
            "{html}"
        );
        assert!(html.contains(r#"class="freshness""#), "{html}");
        assert!(html.contains("<span>8s</span>"), "{html}");
        assert!(
            html.contains(r#"<div class="detail-body"><div>BODY</div></div>"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<div class="detail-footer"><button>Edit</button></div>"#),
            "{html}"
        );
    }

    #[test]
    fn stale_marks_the_freshness_chip() {
        fn app() -> Element {
            rsx! {
                DetailPanel {
                    title: "row",
                    freshness: "2m",
                    stale: true,
                    on_refresh: |_| {},
                    on_close: |_| {},
                    body: rsx! {},
                    footer: rsx! {},
                }
            }
        }
        let html = render(app);
        assert!(html.contains(r#"class="freshness stale""#), "{html}");
    }
}
