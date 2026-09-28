//! Stats-pane building blocks shared by the Explorer's engine viewers: a
//! `StatCard` tile (label, value, sub-line) for the `.stats-grid`, and a
//! `StatSection` that frames one chart or breakdown under a heading. The
//! chart itself (area sparkline, cardinality bars, …) differs per viewer and
//! is passed in as the section's body.

use dioxus::prelude::*;

/// One figure in a `.stats-grid`. Values arrive pre-formatted for display.
#[component]
pub fn StatCard(label: String, value: String, sub: String) -> Element {
    rsx! {
        div { class: "stat-card",
            div { class: "lbl", "{label}" }
            div { class: "val", "{value}" }
            div { class: "sub", "{sub}" }
        }
    }
}

/// A titled `.stat-spark` frame around one chart or breakdown.
#[component]
pub fn StatSection(title: String, body: Element) -> Element {
    rsx! {
        div { class: "stat-section",
            h3 { "{title}" }
            div { class: "stat-spark", {body} }
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
    fn card_renders_label_value_sub_in_order() {
        fn app() -> Element {
            rsx! { StatCard { label: "Documents", value: "2,400,182", sub: "+820/min" } }
        }
        let html = render(app);
        assert!(
            html.contains(
                r#"<div class="lbl">Documents</div><div class="val">2,400,182</div><div class="sub">+820/min</div>"#
            ),
            "{html}"
        );
    }

    #[test]
    fn section_frames_its_body_under_the_title() {
        fn app() -> Element {
            rsx! { StatSection { title: "Write throughput · last 1h", body: rsx! { svg {} } } }
        }
        let html = render(app);
        assert!(
            html.contains(
                r#"<h3>Write throughput · last 1h</h3><div class="stat-spark"><svg></svg></div>"#
            ),
            "{html}"
        );
    }
}
