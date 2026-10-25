//! One Monitoring chart: an SVG line of one metric over a fixed time window.
//!
//! The SVG is sized to its box in pixels, so axis text stays 12px at any
//! panel size. Time runs from `start` to `end`: a node that stopped reporting
//! draws a line that ends at its last sample, with a dashed marker and the
//! time of that sample. The value axis starts at zero and carries the unit.

use dioxus::prelude::*;

use crate::models::cluster_chart::{nearest, time_ticks, value_axis};
use crate::models::cluster_monitor::{MetricSeries, STALE_AFTER_SECS};
use crate::models::format::clock_second;
use crate::models::timeseries_axis::{axis_number, fixed};
use crate::models::timeseries_chart::Frame;

#[derive(Clone, PartialEq, Props)]
pub struct MetricChartProps {
    pub series: MetricSeries,
    pub start: i64,
    pub end: i64,
}

#[component]
pub fn MetricChart(props: MetricChartProps) -> Element {
    let mut size = use_signal(|| (480.0_f64, 180.0_f64));
    let mut hover = use_signal(|| None::<usize>);
    let s = &props.series;
    let metric = s.metric;
    let unit = metric.unit();
    let (w, h) = size();
    let frame = Frame {
        width: w,
        height: h,
        left: 56.0,
        right: 14.0,
        top: 22.0,
        bottom: 26.0,
    };
    let max = s.max().unwrap_or(0.0);
    let ticks = value_axis(metric, max);
    let top_value = ticks.last().copied().unwrap_or(1.0).max(f64::MIN_POSITIVE);
    let span = (props.end - props.start).max(1) as f64;
    let x_of = |t: i64| {
        frame.left + frame.plot_width() * ((t - props.start) as f64 / span).clamp(0.0, 1.0)
    };
    let y_of = |v: f64| frame.top + frame.plot_height() * (1.0 - (v / top_value).clamp(0.0, 1.0));
    let base = frame.top + frame.plot_height();
    let x_ticks = time_ticks(
        props.start,
        props.end,
        ((frame.plot_width() / 70.0) as usize).max(2),
    );
    let points: Vec<String> = s
        .samples
        .iter()
        .map(|p| format!("{:.1} {:.1}", x_of(p.at), y_of(p.value)))
        .collect();
    let line = format!("M{}", points.join("L"));
    let area = match (s.samples.first(), s.samples.last()) {
        (Some(a), Some(b)) => format!(
            "M{:.1} {:.1}L{}L{:.1} {:.1}z",
            x_of(a.at),
            base,
            points.join("L"),
            x_of(b.at),
            base
        ),
        _ => String::new(),
    };
    let last = s.samples.last().map(|p| p.at);
    let ended = last.filter(|t| props.end - *t > STALE_AFTER_SECS);
    let empty = s.samples.is_empty();
    let tip = hover().and_then(|i| s.samples.get(i).copied());
    let hit_style = format!(
        "left:{}px; top:{}px; width:{}px; height:{}px;",
        frame.left,
        frame.top,
        frame.plot_width(),
        frame.plot_height()
    );
    let (start, plot_w) = (props.start, frame.plot_width());
    let samples = s.samples.clone();
    let n = s.samples.len();
    let color = metric.color();
    rsx! {
        div { class: "ts-chart cl-chart",
            div {
                class: "ts-plot",
                onresize: move |e| {
                    if let Ok(sz) = e.get_border_box_size() {
                        let next = (sz.width.max(160.0), sz.height.max(90.0));
                        let now = *size.peek();
                        if (next.0 - now.0).abs() > 0.5 || (next.1 - now.1).abs() > 0.5 {
                            size.set(next);
                        }
                    }
                },
                svg {
                    class: "ts-svg",
                    role: "img",
                    "aria-label": "{metric.title()} in {unit} over the time window",
                    view_box: "0 0 {w} {h}",
                    text { class: "ts-tick title", x: "4", y: "12", text_anchor: "start", "{unit}" }
                    for t in ticks.iter() {
                        {
                            let y = y_of(*t);
                            rsx! {
                                line { key: "gy{t}", class: "ts-grid", x1: "{frame.left}", x2: "{w - frame.right}", y1: "{y}", y2: "{y}" }
                                text { class: "ts-tick", x: "{frame.left - 8.0}", y: "{y + 4.0}", text_anchor: "end", "{axis_number(*t)}" }
                            }
                        }
                    }
                    for (t , label) in x_ticks.iter() {
                        {
                            let x = x_of(*t);
                            rsx! {
                                line { key: "gx{t}", class: "ts-grid vert", x1: "{x}", x2: "{x}", y1: "{frame.top}", y2: "{base}" }
                                text { class: "ts-tick", x: "{x}", y: "{base + 18.0}", text_anchor: "middle", "{label}" }
                            }
                        }
                    }
                    line { class: "ts-axis", x1: "{frame.left}", x2: "{w - frame.right}", y1: "{base}", y2: "{base}" }
                    if !empty {
                        g { class: "ts-s{color}",
                            path { class: "ts-area", d: "{area}" }
                            path { class: "ts-line", d: "{line}" }
                        }
                    }
                    if let Some(t) = ended {
                        {
                            let x = x_of(t);
                            let label = format!("Last report {} UTC", clock_second(t, 0));
                            let anchor = if x > w * 0.6 { "end" } else { "start" };
                            let tx = if x > w * 0.6 { x - 6.0 } else { x + 6.0 };
                            rsx! {
                                line { class: "cl-ended", x1: "{x}", x2: "{x}", y1: "{frame.top}", y2: "{base}" }
                                text { class: "ts-tick", x: "{tx}", y: "{frame.top + 12.0}", text_anchor: "{anchor}", "{label}" }
                            }
                        }
                    }
                    if let Some(p) = tip {
                        {
                            let x = x_of(p.at);
                            let y = y_of(p.value);
                            let head = format!("{} UTC", clock_second(p.at, 0));
                            let value = format!("{} {unit}", fixed(p.value, metric.decimals()));
                            let tw = (head.chars().count().max(value.chars().count()) as f64) * 7.2 + 20.0;
                            let tx = if x + 12.0 + tw > w - 4.0 { x - 12.0 - tw } else { x + 12.0 };
                            let ty = frame.top + 4.0;
                            rsx! {
                                line { class: "ts-cursor", x1: "{x}", x2: "{x}", y1: "{frame.top}", y2: "{base}" }
                                g { class: "ts-s{color}", circle { class: "ts-mark", cx: "{x}", cy: "{y}", r: "4" } }
                                g { class: "ts-tip",
                                    rect { x: "{tx}", y: "{ty}", width: "{tw}", height: "40", rx: "6" }
                                    text { class: "ts-tip-head", x: "{tx + 10.0}", y: "{ty + 16.0}", "{head}" }
                                    text { x: "{tx + 10.0}", y: "{ty + 33.0}", "{value}" }
                                }
                            }
                        }
                    }
                }
                if empty {
                    div { class: "ts-empty", "No sample in this window." }
                }
                div {
                    class: "ts-hit",
                    style: "{hit_style}",
                    tabindex: "0",
                    role: "application",
                    "aria-label": "{metric.title()} values. Arrow keys move between samples.",
                    onmousemove: {
                        let samples = samples.clone();
                        move |e| {
                            let t = start + ((e.element_coordinates().x / plot_w) * span) as i64;
                            let i = nearest(&samples, t);
                            if *hover.peek() != i {
                                hover.set(i);
                            }
                        }
                    },
                    onmouseleave: move |_| hover.set(None),
                    onkeydown: move |e| {
                        let now = *hover.peek();
                        let next = match e.key() {
                            Key::ArrowRight => Some(now.map_or(0, |i| (i + 1).min(n.saturating_sub(1)))),
                            Key::ArrowLeft => Some(now.map_or(0, |i| i.saturating_sub(1))),
                            Key::Escape => None,
                            _ => return,
                        };
                        e.prevent_default();
                        hover.set(next.filter(|_| n > 0));
                    },
                    onblur: move |_| hover.set(None),
                }
            }
        }
    }
}
