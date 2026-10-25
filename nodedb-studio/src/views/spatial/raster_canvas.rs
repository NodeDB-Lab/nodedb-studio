//! The raster canvas: the band as one bitmap in an SVG, placed by the
//! GEOREF, with the selected cell, the legend, and the pointer tools.
//!
//! The bitmap holds one pixel per cell, colored from the physical values.
//! Picking inverts the GEOREF at the pointer, so it reads the stored cell,
//! never a screen color.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::geo_proj::PlaneView;
use crate::models::geo_raster::{Palette, RasterCrs, RasterFrame, ValueMode};
use crate::models::img_encode::png_data_uri;
use crate::models::spatial::{RasterSlice, RasterSource};
use crate::models::spatial_text::{raw_text, trimmed};
use crate::state::spatial::SpatialUi;
use crate::views::spatial::map_draw::text_width;

const PAD: f64 = 28.0;
const CLICK_PX: f64 = 4.0;
const ZOOM_BUTTON: f64 = 1.5;
const ZOOM_WHEEL: f64 = 1.25;
const KEY_PAN_PX: f64 = 80.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    client: (f64, f64),
    travelled: f64,
}

/// The rendered bitmap and the range its colors cover.
#[derive(Debug, Clone, PartialEq)]
struct Rendered {
    uri: String,
    width: u32,
    height: u32,
    range: Option<(f64, f64)>,
}

fn render(
    slice: &RasterSlice,
    frame: RasterFrame,
    palette: Palette,
    values: ValueMode,
) -> Rendered {
    let physical = values == ValueMode::Physical;
    let range = slice.range(physical);
    let (lo, hi) = range.unwrap_or((0.0, 1.0));
    let span = if hi > lo { hi - lo } else { 1.0 };
    let (width, height, rgba) = frame.render(|row, col| {
        let raw = slice.raw_at(row, col)?;
        let p = slice.meta.physical(raw)?;
        let v = if physical { p } else { raw };
        Some(palette.color((v - lo) / span))
    });
    Rendered {
        uri: png_data_uri(width, height, &rgba).unwrap_or_default(),
        width,
        height,
        range,
    }
}

/// The SVG matrix that places the bitmap in a window onto the display plane.
fn matrix(frame: RasterFrame, view: PlaneView, (w, h): (f64, f64)) -> String {
    let a = frame.image_affine();
    let r = view.res;
    format!(
        "matrix({} {} {} {} {} {})",
        a.a / r,
        -a.c / r,
        a.b / r,
        -a.d / r,
        (a.x0 - view.cx) / r + w / 2.0,
        (view.cy - a.y0) / r + h / 2.0
    )
}

#[component]
pub fn RasterCanvas(raster: RasterSource, slice: RasterSlice) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut state = ui.raster;
    let mut drag = use_signal(|| None::<Drag>);
    let mut hover = use_signal(|| None::<(f64, f64)>);
    // The last size reported, restored if a reset clears `measured`.
    let mut seen = use_signal(|| None::<(f64, f64)>);
    use_effect(move || {
        let measured = state.read().measured;
        let Some(sz) = *seen.peek() else { return };
        if !measured {
            let mut s = state.write();
            s.size = sz;
            s.measured = true;
        }
    });

    let st = state.read().clone();
    let base = RasterFrame {
        rows: slice.rows,
        cols: slice.cols,
        source: slice.affine,
        srid: slice.srid,
        crs: RasterCrs::Native,
    };
    let crs = if base.supports(st.crs).is_ok() {
        st.crs
    } else {
        RasterCrs::Native
    };
    let frame = RasterFrame { crs, ..base };
    let size = st.size;
    let physical = st.values == ValueMode::Physical;

    let (palette, values) = (st.palette, st.values);
    let rendered = use_memo(use_reactive!(|slice, crs, palette, values| {
        render(&slice, RasterFrame { crs, ..base }, palette, values)
    }));
    let rendered = rendered.read().clone();

    // Frame the grid once the canvas has a size, and again when asked.
    use_effect(move || {
        let (fit, none, measured, size) = {
            let s = state.read();
            (s.fit, s.view.is_none(), s.measured, s.size)
        };
        if !measured || !(fit || none) {
            return;
        }
        let mut s = state.write();
        s.fit = false;
        s.view = Some(PlaneView::fit(frame.bounds(), size, PAD, 1.0));
    });

    let fit_res = PlaneView::fit(frame.bounds(), size, PAD, 1.0).res;
    let limits = (fit_res / 120.0, fit_res * 3.0);
    let view = st.view;
    let cell = st.cell.filter(|(r, c)| *r < slice.rows && *c < slice.cols);

    let mut zoom = move |factor: f64| {
        let mut s = state.write();
        let size = s.size;
        if let Some(v) = s.view {
            s.view = Some(v.zoomed_about(factor, (size.0 / 2.0, size.1 / 2.0), size, limits));
        }
    };
    let mut pick = move |px: f64, py: f64| {
        let (view, size) = {
            let s = state.peek();
            (s.view, s.size)
        };
        let Some(view) = view else { return };
        let (x, y) = view.from_screen(px, py, size);
        if let Some(rc) = frame.cell_at(x, y) {
            state.write().cell = Some(rc);
        }
    };

    // The selected cell's marker, in screen pixels.
    let marker = view.zip(cell).map(|(v, (row, col))| {
        let (cx, cy) = frame.cell_centre(row, col);
        let (sx, sy) = v.to_screen(cx, cy, size);
        let corners = frame.cell_corners(row, col);
        let (p0, p1) = (
            v.to_screen(corners[0].0, corners[0].1, size),
            v.to_screen(corners[1].0, corners[1].1, size),
        );
        let side = (p1.0 - p0.0).hypot(p1.1 - p0.1).max(12.0);
        (sx, sy, side, format!("row {row} · col {col}"))
    });

    let footprint = view.map(|v| {
        let (w, h) = (rendered.width, rendered.height);
        let pts = frame.image_affine().corners(h, w);
        let mut d = String::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            let (sx, sy) = v.to_screen(*x, *y, size);
            d.push_str(&format!(
                "{}{sx:.1} {sy:.1}",
                if i == 0 { "M" } else { "L" }
            ));
        }
        d.push('Z');
        d
    });

    let meta = &slice.meta;
    let legend_title = if physical {
        if meta.units.is_empty() {
            meta.name.clone()
        } else {
            format!("{} ({})", meta.name, meta.units)
        }
    } else {
        format!("{} · raw {}", meta.name, meta.dtype)
    };
    let end = |v: f64| trimmed(v, 2);
    let nodata_text = meta.nodata.map(|n| raw_text(meta, n));

    rsx! {
        div {
            class: "sp-map sp-rmap pan",
            tabindex: "0",
            role: "application",
            "aria-label": "Raster. Arrow keys move the selected cell. Shift and arrow keys pan. Plus and minus zoom. Zero fits the grid.",
            onresize: move |e| {
                if let Ok(sz) = e.get_border_box_size() {
                    let (w, h) = (sz.width.round(), sz.height.round());
                    if w > 1.0 && h > 1.0 {
                        seen.set(Some((w, h)));
                        let mut s = state.write();
                        if s.size != (w, h) || !s.measured {
                            s.size = (w, h);
                            s.measured = true;
                        }
                    }
                }
            },
            onkeydown: {
                let (rows, cols) = (slice.rows, slice.cols);
                move |e| {
                    let shift = e.modifiers().shift();
                    let mut step = |dr: i64, dc: i64, dx: f64, dy: f64| {
                        let mut s = state.write();
                        if shift {
                            if let Some(v) = s.view {
                                s.view = Some(v.panned_px(dx, dy));
                            }
                        } else {
                            let (r, c) = s.cell.unwrap_or((rows / 2, cols / 2));
                            let r = (i64::from(r) + dr).clamp(0, i64::from(rows) - 1) as u32;
                            let c = (i64::from(c) + dc).clamp(0, i64::from(cols) - 1) as u32;
                            s.cell = Some((r, c));
                        }
                    };
                    match e.key() {
                        Key::ArrowLeft => step(0, -1, KEY_PAN_PX, 0.0),
                        Key::ArrowRight => step(0, 1, -KEY_PAN_PX, 0.0),
                        Key::ArrowUp => step(-1, 0, 0.0, KEY_PAN_PX),
                        Key::ArrowDown => step(1, 0, 0.0, -KEY_PAN_PX),
                        Key::Character(c) if c == "+" || c == "=" => zoom(ZOOM_BUTTON),
                        Key::Character(c) if c == "-" => zoom(1.0 / ZOOM_BUTTON),
                        Key::Character(c) if c == "0" => state.write().fit = true,
                        _ => return,
                    }
                    e.prevent_default();
                }
            },
            if let Some(view) = view {
                svg {
                    class: "sp-svg",
                    view_box: "0 0 {size.0} {size.1}",
                    preserve_aspect_ratio: "none",
                    role: "img",
                    "aria-label": "Raster of {slice.request.band}, {slice.rows} rows by {slice.cols} columns",
                    defs {
                        pattern { id: "sp-checker", width: "16", height: "16", pattern_units: "userSpaceOnUse",
                            rect { class: "sp-chk-a", x: "0", y: "0", width: "16", height: "16" }
                            rect { class: "sp-chk-b", x: "0", y: "0", width: "8", height: "8" }
                            rect { class: "sp-chk-b", x: "8", y: "8", width: "8", height: "8" }
                        }
                    }
                    if let Some(d) = footprint {
                        path { d: "{d}", fill: "url(#sp-checker)" }
                    }
                    if !rendered.uri.is_empty() {
                        image {
                            href: "{rendered.uri}",
                            width: "{rendered.width}",
                            height: "{rendered.height}",
                            transform: "{matrix(frame, view, size)}",
                            preserve_aspect_ratio: "none",
                            style: "image-rendering: pixelated",
                        }
                    }
                    if let Some((x, y, side, label)) = marker.clone() {
                        {
                            let w = text_width(&label);
                            let flip = x + 28.0 + w > size.0 - 8.0;
                            let (lx, ly) = if flip { (x - 28.0 - w, y - 34.0) } else { (x + 28.0, y - 34.0) };
                            let (ex, ey) = if flip { (lx + w, ly + 26.0) } else { (lx, ly + 26.0) };
                            let lead_x = if flip { x - side / 2.0 } else { x + side / 2.0 };
                            rsx! {
                                rect {
                                    class: "sp-cell-ring halo",
                                    x: "{x - side / 2.0}",
                                    y: "{y - side / 2.0}",
                                    width: "{side}",
                                    height: "{side}",
                                }
                                rect {
                                    class: "sp-cell-ring core",
                                    x: "{x - side / 2.0}",
                                    y: "{y - side / 2.0}",
                                    width: "{side}",
                                    height: "{side}",
                                }
                                line { class: "sp-leader", x1: "{lead_x}", y1: "{y - side / 2.0}", x2: "{ex}", y2: "{ey}" }
                                rect { class: "sp-label-box", x: "{lx}", y: "{ly}", width: "{w}", height: "26", rx: "4" }
                                text { class: "sp-label", x: "{lx + 9.0}", y: "{ly + 18.0}", "{label}" }
                            }
                        }
                    }
                }
                div {
                    class: "sp-hit",
                    onmousedown: move |e| {
                        let c = e.client_coordinates();
                        drag.set(Some(Drag { client: (c.x, c.y), travelled: 0.0 }));
                    },
                    onmousemove: move |e| {
                        let p = e.element_coordinates();
                        hover.set(Some((p.x, p.y)));
                        let Some(d) = *drag.peek() else { return };
                        let c = e.client_coordinates();
                        let (dx, dy) = (c.x - d.client.0, c.y - d.client.1);
                        let mut s = state.write();
                        if let Some(v) = s.view {
                            s.view = Some(v.panned_px(dx, dy));
                        }
                        drag.set(Some(Drag { client: (c.x, c.y), travelled: d.travelled + dx.abs() + dy.abs() }));
                    },
                    onmouseup: move |e| {
                        let Some(d) = *drag.peek() else { return };
                        drag.set(None);
                        if d.travelled < CLICK_PX {
                            let p = e.element_coordinates();
                            pick(p.x, p.y);
                        }
                    },
                    onmouseleave: move |_| {
                        drag.set(None);
                        hover.set(None);
                    },
                    onwheel: move |e| {
                        e.prevent_default();
                        let up = e.delta().strip_units().y < 0.0;
                        let Some((px, py)) = *hover.peek() else { return };
                        let mut s = state.write();
                        let size = s.size;
                        if let Some(v) = s.view {
                            let f = if up { ZOOM_WHEEL } else { 1.0 / ZOOM_WHEEL };
                            s.view = Some(v.zoomed_about(f, (px, py), size, limits));
                        }
                    },
                }
                div { class: "sp-controls",
                    div { class: "sp-zoom",
                        button {
                            class: "icon-btn sp-ctl",
                            title: "Zoom in",
                            "aria-label": "Zoom in",
                            onclick: move |_| zoom(ZOOM_BUTTON),
                            Ico { icon: Icon::Plus, size: 18 }
                        }
                        button {
                            class: "icon-btn sp-ctl",
                            title: "Zoom out",
                            "aria-label": "Zoom out",
                            onclick: move |_| zoom(1.0 / ZOOM_BUTTON),
                            Ico { icon: Icon::Minus, size: 18 }
                        }
                        button {
                            class: "icon-btn sp-ctl",
                            title: "Fit the grid",
                            "aria-label": "Fit the grid",
                            onclick: move |_| state.write().fit = true,
                            Ico { icon: Icon::Maximize, size: 18 }
                        }
                        span {
                            class: "sp-north",
                            title: "Up is north of the display CRS. The grid does not rotate.",
                            role: "img",
                            "aria-label": "North is up",
                            span { class: "arrow", "aria-hidden": "true" }
                            "N"
                        }
                    }
                    button {
                        class: "btn sm sp-region",
                        disabled: true,
                        title: "A zonal analysis needs a region to carry into a draft. Selecting a region has no design yet.",
                        Ico { icon: Icon::Square, size: 16 }
                        "Analyze region"
                    }
                }
                div { class: "sp-rlegend",
                    div { class: "sp-ramp-block",
                        span { class: "sp-ramp-title", "{legend_title}" }
                        if let Some((lo, hi)) = rendered.range {
                            div { class: "sp-ramp", style: "background: {st.palette.css_gradient()}" }
                            div { class: "sp-ramp-ends",
                                span { "{end(lo)}" }
                                span { "{end(hi)}" }
                            }
                        } else {
                            span { class: "sp-ramp-none", "No valid cells in this slice" }
                        }
                    }
                    if let Some(n) = nodata_text {
                        div { class: "sp-nodata",
                            span { class: "sp-chk-swatch", "aria-hidden": "true" }
                            span { title: "Raw marker {n}. NoData is missing data, never a low value.", "NoData" }
                        }
                    } else {
                        div { class: "sp-nodata none", title: "The band declares no NoData marker, so every cell is data.", "No NoData marker" }
                    }
                }
            } else {
                div { class: "sp-state", role: "status", "Framing the raster…" }
            }
        }
    }
}
