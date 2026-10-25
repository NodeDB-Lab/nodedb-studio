//! The Timeseries chart: an SVG drawn from one `ExploreResult`.
//!
//! The SVG is sized to its box in pixels, so axis text stays 12px at any
//! panel size. Marks sit at bucket centers and a line breaks wherever a
//! bucket holds no value: nothing is drawn between observations. Each series
//! has its own marker shape as well as its own color. A filled point, a
//! null bucket, and a provisional bucket each draw differently from an
//! observed one, and the tooltip says which it is.

use std::collections::BTreeSet;

use dioxus::prelude::*;

use crate::models::timeseries::{
    Aggregate, ChartKind, ExploreResult, PointState, Series, TimeSpan,
};
use crate::models::timeseries_axis::{axis_number, fixed, long_stamp};
use crate::models::timeseries_chart::{Frame, Geometry, marker, runs, y_title};

/// How a value reads: counts whole, everything else to two places.
pub fn value_text(aggregate: Aggregate, v: f64) -> String {
    match aggregate {
        Aggregate::Count => fixed(v, 0),
        Aggregate::Sum | Aggregate::Avg | Aggregate::Min | Aggregate::Max => fixed(v, 2),
    }
}

/// The name a series goes by: its group, or the measure when it has none.
pub fn series_name(s: &Series, result: &ExploreResult) -> String {
    if s.key.is_empty() {
        result.query.measure.label()
    } else {
        s.key.clone()
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct TsChartProps {
    pub result: ExploreResult,
    #[props(default)]
    pub kind: ChartKind,
    /// Series keys the legend hides.
    #[props(default)]
    pub hidden: BTreeSet<String>,
    /// The most series drawn, heaviest first. The table lists the rest.
    #[props(default = 6)]
    pub limit: usize,
    /// A small chart, as in a preview: fewer ticks and tighter margins.
    #[props(default)]
    pub compact: bool,
    /// The shown result is older than the query: draw it dimmed.
    #[props(default)]
    pub stale: bool,
    /// Whether a drag over the plot selects a range.
    #[props(default)]
    pub brushable: bool,
    #[props(default)]
    pub on_brush: EventHandler<TimeSpan>,
    /// A click on a legend item, with the series key.
    #[props(default)]
    pub on_toggle: EventHandler<String>,
    /// Whether to print the query summary under the legend.
    #[props(default = true)]
    pub caption: bool,
}

#[component]
pub fn TsChart(props: TsChartProps) -> Element {
    let mut size = use_signal(|| (640.0_f64, 300.0_f64));
    let mut hover = use_signal(|| None::<usize>);
    let mut drag = use_signal(|| None::<(usize, usize)>);

    let r = &props.result;
    let agg = r.query.measure.aggregate;
    let drawn: Vec<&Series> = r.series.iter().take(props.limit).collect();
    let visible: Vec<(usize, &Series)> = drawn
        .iter()
        .enumerate()
        .filter(|(_, s)| !props.hidden.contains(&s.key))
        .map(|(i, s)| (i, *s))
        .collect();
    let (w, h) = size();
    let (left, right, top, bottom) = if props.compact {
        (58.0, 10.0, 8.0, 40.0)
    } else {
        (70.0, 16.0, 12.0, 46.0)
    };
    let frame = Frame {
        width: w,
        height: h,
        left,
        right,
        top,
        bottom,
    };
    let only: Vec<&Series> = visible.iter().map(|(_, s)| *s).collect();
    let g = Geometry::new(frame, r, &only, if props.compact { 3 } else { 4 });
    let title = y_title(r);
    let empty = r.is_empty();
    let hidden_count = r.series.len().saturating_sub(props.limit);
    let hit_style = format!(
        "left:{left}px; top:{top}px; width:{}px; height:{}px;",
        frame.plot_width(),
        frame.plot_height()
    );
    let brush_band = drag().map(|(a, b)| (a.min(b), a.max(b)));
    let tip = hover().filter(|i| *i < r.buckets.len());
    let hit = g.hit_map();
    let (brushable, on_brush, on_toggle) = (props.brushable, props.on_brush, props.on_toggle);

    rsx! {
        div { class: if props.stale { "ts-chart stale" } else { "ts-chart" },
            div {
                class: "ts-plot",
                onresize: move |e| {
                    if let Ok(sz) = e.get_border_box_size() {
                        let next = (sz.width.max(120.0), sz.height.max(80.0));
                        let now = *size.peek();
                        if (next.0 - now.0).abs() > 0.5 || (next.1 - now.1).abs() > 0.5 {
                            size.set(next);
                        }
                    }
                },
                svg {
                    class: "ts-svg",
                    role: "img",
                    "aria-label": "{title} over {r.buckets.len()} buckets, {drawn.len()} series",
                    view_box: "0 0 {w} {h}",
                    // Gridlines and value labels.
                    for t in g.ticks.iter() {
                        {
                            let y = g.y(*t);
                            rsx! {
                                line { key: "gy{t}", class: "ts-grid", x1: "{left}", x2: "{w - right}", y1: "{y}", y2: "{y}" }
                                text { class: "ts-tick", x: "{left - 8.0}", y: "{y + 4.0}", text_anchor: "end", "{axis_number(*t)}" }
                            }
                        }
                    }
                    for (i , label) in g.x_labels.iter() {
                        {
                            let x = g.x(*i);
                            rsx! {
                                line { key: "gx{i}", class: "ts-grid vert", x1: "{x}", x2: "{x}", y1: "{top}", y2: "{g.baseline()}" }
                                text { class: "ts-tick", x: "{x}", y: "{g.baseline() + 18.0}", text_anchor: "middle", "{label}" }
                            }
                        }
                    }
                    line { class: "ts-axis", x1: "{left}", x2: "{w - right}", y1: "{g.baseline()}", y2: "{g.baseline()}" }
                    text {
                        class: "ts-tick title",
                        x: "{left + frame.plot_width() / 2.0}",
                        y: "{h - 6.0}",
                        text_anchor: "middle",
                        "UTC"
                    }
                    text {
                        class: "ts-tick title",
                        x: "14",
                        y: "{top + frame.plot_height() / 2.0}",
                        text_anchor: "middle",
                        transform: "rotate(-90 14 {top + frame.plot_height() / 2.0})",
                        "{title}"
                    }
                    if let Some((a , b)) = brush_band {
                        rect {
                            class: "ts-brush",
                            x: "{left + g.band * a as f64}",
                            y: "{top}",
                            width: "{g.band * (b - a + 1) as f64}",
                            height: "{frame.plot_height()}",
                        }
                    }
                    // Bars, areas, then lines, so a marker is never covered.
                    for (c , s) in visible.iter() {
                        match props.kind {
                            ChartKind::Bar => {
                                let m = visible.len().max(1);
                                let slot = visible.iter().position(|(ci, _)| ci == c).unwrap_or(0);
                                let group = g.band * 0.78;
                                let bar = group / m as f64;
                                rsx! {
                                    g { key: "bars{c}", class: "ts-s{c % 6}",
                                        for (i , start) in r.buckets.iter().enumerate() {
                                            if let Some(p) = s.point_at(*start) {
                                                if let Some(v) = p.value {
                                                    rect {
                                                        key: "b{c}-{i}",
                                                        class: if p.provisional { "ts-bar provisional" } else if matches!(p.state, PointState::Filled(_)) { "ts-bar filled" } else { "ts-bar" },
                                                        x: "{g.x(i) - group / 2.0 + bar * slot as f64}",
                                                        y: "{g.y(v)}",
                                                        width: "{(bar - 1.0).max(1.0)}",
                                                        height: "{(g.baseline() - g.y(v)).max(0.0)}",
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ChartKind::Line | ChartKind::Area => {
                                let paths: Vec<(String, String)> = runs(s, &r.buckets)
                                    .into_iter()
                                    .map(|run| {
                                        let pts: Vec<String> = run
                                            .iter()
                                            .map(|(i, v)| format!("{:.1} {:.1}", g.x(*i), g.y(*v)))
                                            .collect();
                                        let line = format!("M{}", pts.join("L"));
                                        let first = run.first().map_or(0, |p| p.0);
                                        let last = run.last().map_or(0, |p| p.0);
                                        let area = format!(
                                            "M{:.1} {:.1}L{}L{:.1} {:.1}z",
                                            g.x(first),
                                            g.baseline(),
                                            pts.join("L"),
                                            g.x(last),
                                            g.baseline()
                                        );
                                        (line, area)
                                    })
                                    .collect();
                                rsx! {
                                    g { key: "line{c}", class: "ts-s{c % 6}",
                                        if props.kind == ChartKind::Area {
                                            for (n , (_ , area)) in paths.iter().enumerate() {
                                                path { key: "a{c}-{n}", class: "ts-area", d: "{area}" }
                                            }
                                        }
                                        for (n , (line , _)) in paths.iter().enumerate() {
                                            path { key: "l{c}-{n}", class: "ts-line", d: "{line}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if props.kind != ChartKind::Bar {
                        for (c , s) in visible.iter() {
                            g { key: "marks{c}", class: "ts-s{c % 6}",
                                for (i , start) in r.buckets.iter().enumerate() {
                                    if let Some(p) = s.point_at(*start) {
                                        if let Some(v) = p.value {
                                            path {
                                                key: "m{c}-{i}",
                                                class: if p.provisional { "ts-mark hollow" } else if matches!(p.state, PointState::Filled(_)) { "ts-mark filled" } else { "ts-mark" },
                                                d: "{marker(*c, g.x(i), g.y(v), 4.0)}",
                                            }
                                        } else {
                                            path {
                                                key: "m{c}-{i}",
                                                class: "ts-mark hollow null",
                                                d: "{marker(*c, g.x(i), g.baseline() - 5.0, 3.5)}",
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(i) = tip {
                        {
                            let x = g.x(i);
                            let lines: Vec<(usize, String, String)> = visible
                                .iter()
                                .map(|(c, s)| {
                                    let name = series_name(s, r);
                                    match s.point_at(r.buckets[i]) {
                                        None => (*c, name, "no rows".to_string()),
                                        Some(p) => {
                                            let mut t = match p.value {
                                                Some(v) => value_text(agg, v),
                                                None => "null".to_string(),
                                            };
                                            match p.state {
                                                PointState::Observed => {}
                                                PointState::NullOnly => t.push_str(" (only nulls)"),
                                                PointState::Filled(f) => {
                                                    t.push_str(&format!(" ({})", f.label()));
                                                }
                                            }
                                            if p.provisional {
                                                t.push_str(" (provisional)");
                                            }
                                            (*c, name, t)
                                        }
                                    }
                                })
                                .collect();
                            let head = long_stamp(r.buckets[i]);
                            let chars = lines
                                .iter()
                                .map(|(_, n, t)| n.chars().count() + t.chars().count() + 3)
                                .chain([head.chars().count()])
                                .max()
                                .unwrap_or(10);
                            let tw = chars as f64 * 7.0 + 30.0;
                            let th = 14.0 + (lines.len() + 1) as f64 * 18.0;
                            let tx = if x + 14.0 + tw > w - 4.0 { x - 14.0 - tw } else { x + 14.0 };
                            let ty = top + 6.0;
                            rsx! {
                                line { class: "ts-cursor", x1: "{x}", x2: "{x}", y1: "{top}", y2: "{g.baseline()}" }
                                g { class: "ts-tip",
                                    rect { x: "{tx}", y: "{ty}", width: "{tw}", height: "{th}", rx: "6" }
                                    text { class: "ts-tip-head", x: "{tx + 10.0}", y: "{ty + 20.0}", "{head}" }
                                    for (n , (c , name , t)) in lines.iter().enumerate() {
                                        g { key: "tip{n}", class: "ts-s{c % 6}",
                                            path { class: "ts-mark", d: "{marker(*c, tx + 16.0, ty + 20.0 + 18.0 * (n as f64 + 1.0) - 4.0, 4.0)}" }
                                            text { x: "{tx + 28.0}", y: "{ty + 20.0 + 18.0 * (n as f64 + 1.0)}", "{name}: {t}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if empty {
                    div { class: "ts-empty",
                        "No rows matched in this range. "
                        "{r.rows_scanned} rows were read before the filters."
                    }
                }
                div {
                    class: "ts-hit",
                    style: "{hit_style}",
                    tabindex: "0",
                    role: "application",
                    "aria-label": "Chart values. Arrow keys move between buckets.",
                    onmousemove: move |e| {
                        let i = hit.bucket_at(e.element_coordinates().x);
                        if hover.peek().as_ref() != i.as_ref() {
                            hover.set(i);
                        }
                        let current = *drag.peek();
                        if let (Some((a, _)), Some(i)) = (current, i) {
                            drag.set(Some((a, i)));
                        }
                    },
                    onmouseleave: move |_| {
                        hover.set(None);
                        drag.set(None);
                    },
                    onmousedown: move |e| {
                        if brushable
                            && let Some(i) = hit.bucket_at(e.element_coordinates().x)
                        {
                            drag.set(Some((i, i)));
                        }
                    },
                    onmouseup: {
                        let starts = r.buckets.clone();
                        let size_s = r.query.bucket.seconds();
                        move |_| {
                            let Some((a, b)) = drag.peek().as_ref().copied() else { return };
                            drag.set(None);
                            if a != b {
                                let (lo, hi) = (a.min(b), a.max(b));
                                if let (Some(s), Some(e)) = (starts.get(lo), starts.get(hi)) {
                                    on_brush.call(TimeSpan::new(*s, *e + size_s));
                                }
                            }
                        }
                    },
                    onkeydown: {
                        let n = r.buckets.len();
                        move |e| {
                            let now = *hover.peek();
                            let next = match e.key() {
                                Key::ArrowRight => Some(now.map_or(0, |i| (i + 1).min(n.saturating_sub(1)))),
                                Key::ArrowLeft => Some(now.map_or(0, |i| i.saturating_sub(1))),
                                Key::Escape => None,
                                _ => return,
                            };
                            e.prevent_default();
                            hover.set(next.filter(|_| n > 0));
                        }
                    },
                    onblur: move |_| hover.set(None),
                }
            }
            div { class: "ts-legend",
                for (c , s) in drawn.iter().enumerate() {
                    {
                        let key = s.key.clone();
                        let off = props.hidden.contains(&s.key);
                        let name = series_name(s, r);
                        rsx! {
                            button {
                                key: "{c}-{key}",
                                class: if off { "ts-legend-item off" } else { "ts-legend-item" },
                                "aria-pressed": "{!off}",
                                title: if off { "Show {name}" } else { "Hide {name}" },
                                onclick: move |_| on_toggle.call(key.clone()),
                                svg { class: "ts-s{c % 6}", width: "14", height: "14", view_box: "0 0 14 14",
                                    path { class: "ts-mark", d: "{marker(c, 7.0, 7.0, 4.5)}" }
                                }
                                span { "{name}" }
                            }
                        }
                    }
                }
                if hidden_count > 0 {
                    span { class: "ts-more", "{hidden_count} more series are not drawn. The table lists them." }
                }
            }
            if props.caption {
                div { class: "ts-caption", "{r.query.summary()}" }
            }
        }
    }
}
