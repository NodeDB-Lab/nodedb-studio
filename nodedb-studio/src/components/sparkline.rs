//! SVG-fidelity spike: a minimal sparkline drawn as an inline `<svg><polyline>`
//! in pure Dioxus (no JS interop), fed from a typed `Vec<f64>`. Proves whether
//! rsx!-driven SVG gives the fidelity later chart/topology screens need.

use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct SparklineProps {
    /// Y values; X is the index. Normalized to a 100x24 viewBox.
    pub points: Vec<f64>,
}

#[component]
pub fn Sparkline(props: SparklineProps) -> Element {
    let pts = &props.points;
    if pts.len() < 2 {
        return rsx! { svg { width: "100", height: "24", "viewBox": "0 0 100 24" } };
    }
    let max = pts.iter().cloned().fold(f64::MIN, f64::max);
    let min = pts.iter().cloned().fold(f64::MAX, f64::min);
    let span = (max - min).max(f64::EPSILON);
    let n = (pts.len() - 1) as f64;
    let coords: String = pts
        .iter()
        .enumerate()
        .map(|(i, y)| {
            let x = (i as f64 / n) * 100.0;
            let yy = 24.0 - ((y - min) / span) * 24.0;
            format!("{x:.1},{yy:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    rsx! {
        svg { width: "100", height: "24", "viewBox": "0 0 100 24",
            polyline {
                points: "{coords}",
                fill: "none",
                stroke: "currentColor",
                "stroke-width": "1.5",
            }
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
    fn renders_polyline_from_points() {
        fn app() -> Element {
            rsx! { Sparkline { points: vec![1.0, 4.0, 2.0, 8.0, 5.0] } }
        }
        let html = render(app);
        assert!(html.contains("<svg"));
        assert!(html.contains("<polyline"));
        assert!(html.contains("points="));
    }

    #[test]
    fn degenerate_input_renders_empty_svg() {
        fn app() -> Element {
            rsx! { Sparkline { points: vec![1.0] } }
        }
        let html = render(app);
        assert!(html.contains("<svg"));
        assert!(!html.contains("<polyline"));
    }
}
