//! Array collections: slice controls, the heatmap and table of a slice, and
//! the array's dimensions.
//!
//! An array is not a table of rows. A slice fixes the time and a latitude
//! and longitude range, and returns the cells between. The server rejects a
//! range outside the extents or a slice over the cell limit, and the
//! controls show that reason.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::array_data::{ArraySchema, SliceResult};
use crate::models::format::{date_second, offset_label, thousands};
use crate::services::async_state::AsyncState;
use crate::state::explorer::{RecordView, SliceMode};
use crate::state::preferences::Preferences;

/// A blue-to-red scale, `t` in `[0, 1]`.
pub fn color(t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    let ramp = |c: f64| (255.0 * c.clamp(0.0, 1.0)).round() as u8;
    let r = ramp(1.5 - (4.0 * t - 3.0).abs());
    let g = ramp(1.5 - (4.0 * t - 2.0).abs());
    let b = ramp(1.5 - (4.0 * t - 1.0).abs());
    format!("rgb({r},{g},{b})")
}

/// The row and column of a cell given as tenths of a degree.
fn locate(result: &SliceResult, cell: (i64, i64)) -> Option<(usize, usize)> {
    let tenths = |v: f64| (v * 10.0).round() as i64;
    let row = result.lats.iter().position(|l| tenths(*l) == cell.0)?;
    let col = result.lons.iter().position(|l| tenths(*l) == cell.1)?;
    Some((row, col))
}

/// `Temperature (°C)` from `temperature_c` and its unit.
fn legend_title(attribute: &str, unit: &str) -> String {
    let name = attribute.split('_').next().unwrap_or(attribute);
    let mut chars = name.chars();
    let title = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default();
    format!("{title} ({unit})")
}

fn hour_label(schema: &ArraySchema, index: u32, offset: i16) -> String {
    let at = schema.time_origin + i64::from(index) * schema.time_step;
    format!("{} ({})", date_second(at, offset), offset_label(offset))
}

#[component]
pub fn Heatmap(result: SliceResult, cell: Option<(i64, i64)>) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let (rows, cols) = (result.lats.len(), result.lons.len());
    if rows == 0 || cols == 0 {
        return rsx! {};
    }
    let (x0, y0, w, h) = (72.0, 12.0, 440.0, 330.0);
    let (cw, ch) = (w / cols as f64, h / rows as f64);
    let span = (result.max - result.min).max(f64::EPSILON);
    let step_x = cols.div_ceil(11).max(1);
    let step_y = rows.div_ceil(11).max(1);
    let marker = cell.and_then(|c| locate(&result, c));
    let ticks: Vec<f64> = (0..5)
        .map(|i| result.min + span * f64::from(i) / 4.0)
        .collect();
    rsx! {
        svg {
            class: "heatmap",
            view_box: "0 0 640 400",
            role: "img",
            "aria-label": "Heatmap of {result.attribute}",
            for (r, row) in result.values.iter().enumerate() {
                for (c, v) in row.iter().enumerate() {
                    rect {
                        key: "{r}-{c}",
                        class: "heat-cell",
                        x: "{x0 + c as f64 * cw}",
                        y: "{y0 + r as f64 * ch}",
                        width: "{cw + 0.4}",
                        height: "{ch + 0.4}",
                        fill: "{color((v - result.min) / span)}",
                        onclick: {
                            let tenths = (
                                (result.lats[r] * 10.0).round() as i64,
                                (result.lons[c] * 10.0).round() as i64,
                            );
                            move |_| view.write().slice.cell = Some(tenths)
                        },
                        title { "{result.lats[r]}, {result.lons[c]}: {v} {result.unit}" }
                    }
                }
            }
            if let Some((r, c)) = marker {
                {
                    let cx = x0 + (c as f64 + 0.5) * cw;
                    let cy = y0 + (r as f64 + 0.5) * ch;
                    rsx! {
                        g { class: "crosshair", "aria-hidden": "true",
                            line { x1: "{cx - 8.0}", y1: "{cy}", x2: "{cx + 8.0}", y2: "{cy}" }
                            line { x1: "{cx}", y1: "{cy - 8.0}", x2: "{cx}", y2: "{cy + 8.0}" }
                        }
                    }
                }
            }
            for (c, lon) in result.lons.iter().enumerate().filter(|(c, _)| c % step_x == 0) {
                text {
                    key: "x{c}",
                    class: "axis-label",
                    x: "{x0 + (c as f64 + 0.5) * cw}",
                    y: "{y0 + h + 18.0}",
                    text_anchor: "middle",
                    "{lon}"
                }
            }
            for (r, lat) in result.lats.iter().enumerate().filter(|(r, _)| r % step_y == 0) {
                text {
                    key: "y{r}",
                    class: "axis-label",
                    x: "{x0 - 8.0}",
                    y: "{y0 + (r as f64 + 0.5) * ch + 4.0}",
                    text_anchor: "end",
                    "{lat}"
                }
            }
            text { class: "axis-title", x: "{x0 + w / 2.0}", y: "392", text_anchor: "middle", "Longitude (°E)" }
            text {
                class: "axis-title",
                transform: "translate(14 {y0 + h / 2.0}) rotate(-90)",
                text_anchor: "middle",
                "Latitude (°N)"
            }
            text { class: "axis-title", x: "528", y: "10", "{legend_title(&result.attribute, &result.unit)}" }
            for i in 0..24 {
                rect {
                    key: "k{i}",
                    x: "528",
                    y: "{y0 + 10.0 + i as f64 * (h - 12.0) / 24.0}",
                    width: "16",
                    height: "{(h - 12.0) / 24.0 + 0.4}",
                    fill: "{color(1.0 - f64::from(i) / 23.0)}",
                }
            }
            for (i, t) in ticks.iter().enumerate() {
                text {
                    key: "t{i}",
                    class: "axis-label",
                    x: "550",
                    y: "{y0 + 10.0 + (h - 12.0) * (1.0 - i as f64 / 4.0) + 4.0}",
                    "{t:.0}"
                }
            }
        }
    }
}

#[component]
fn SliceTable(result: SliceResult, cell: Option<(i64, i64)>) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let marker = cell.and_then(|c| locate(&result, c));
    rsx! {
        div { class: "dt-wrap slice-table",
            table { class: "dt",
                thead {
                    tr {
                        th { "lat \\ lon" }
                        for lon in result.lons.iter() {
                            th { key: "{lon}", class: "num", "{lon}" }
                        }
                    }
                }
                tbody {
                    for (r, row) in result.values.iter().enumerate() {
                        tr { key: "{result.lats[r]}",
                            td { class: "key", "{result.lats[r]}" }
                            for (c, v) in row.iter().enumerate() {
                                td {
                                    key: "{c}",
                                    class: if marker == Some((r, c)) { "num picked" } else { "num" },
                                    onclick: {
                                        let tenths = (
                                            (result.lats[r] * 10.0).round() as i64,
                                            (result.lons[c] * 10.0).round() as i64,
                                        );
                                        move |_| view.write().slice.cell = Some(tenths)
                                    },
                                    "{v}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The controls and the slice they read.
#[component]
pub fn SliceView(
    schema: ArraySchema,
    slice: AsyncState<SliceResult>,
    on_retry: EventHandler<()>,
) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let prefs = use_context::<Signal<Preferences>>();
    let offset = prefs.read().utc_offset_minutes;
    let form = view.read().slice.clone();
    let run = move |_| {
        let parsed = view.peek().slice.parse();
        let mut w = view.write();
        match parsed {
            Ok(req) => {
                w.slice.applied = req;
                w.slice.error = None;
            }
            Err(e) => w.slice.error = Some(e),
        }
    };
    rsx! {
        div { class: "slice-controls",
            div { class: "field",
                label { r#for: "slice-time", "Time" }
                select {
                    id: "slice-time",
                    class: "select native",
                    onchange: move |e| {
                        if let Ok(i) = e.value().parse::<u32>() {
                            view.write().slice.time_index = i;
                        }
                    },
                    for i in 0..schema.dimension("time").map_or(0, |d| d.points) {
                        option { key: "{i}", value: "{i}", selected: i == form.time_index, "{hour_label(&schema, i, offset)}" }
                    }
                }
            }
            RangeField {
                label: "Latitude range",
                from: form.lat_from.clone(),
                to: form.lat_to.clone(),
                on_from: move |t| view.write().slice.lat_from = t,
                on_to: move |t| view.write().slice.lat_to = t,
            }
            RangeField {
                label: "Longitude range",
                from: form.lon_from.clone(),
                to: form.lon_to.clone(),
                on_from: move |t| view.write().slice.lon_from = t,
                on_to: move |t| view.write().slice.lon_to = t,
            }
            div { class: "field",
                label { r#for: "slice-attr", "Attribute" }
                select {
                    id: "slice-attr",
                    class: "select native",
                    onchange: move |e| view.write().slice.attribute = e.value(),
                    for a in schema.attributes.iter() {
                        option { key: "{a.name}", value: "{a.name}", selected: a.name == form.attribute, "{a.name}" }
                    }
                }
            }
            button { class: "btn primary", onclick: run,
                Ico { icon: Icon::Play, size: 16 }
                "Run slice"
            }
            button {
                class: "btn",
                disabled: true,
                title: "Exporting an array slice is not designed yet",
                Ico { icon: Icon::Upload, size: 16 }
                "Export"
            }
        }
        if let Some(e) = form.error.clone() {
            div { class: "field-error slice-error", role: "alert", "{e}" }
        }
        div { class: "slice-result",
            div { class: "slice-head",
                h3 { class: "section-title", "Slice result" }
                div { class: "seg", role: "group", "aria-label": "Result view",
                    for (mode, label, icon) in [(SliceMode::Heatmap, "Heatmap", Icon::Grid3x3), (SliceMode::Table, "Table", Icon::Table)] {
                        button {
                            key: "{label}",
                            class: if form.mode == mode { "active" } else { "" },
                            "aria-pressed": "{form.mode == mode}",
                            onclick: move |_| view.write().slice.mode = mode,
                            Ico { icon, size: 14 }
                            " {label}"
                        }
                    }
                }
            }
            crate::components::async_view::AsyncView {
                loading: slice.is_loading(),
                empty: slice.is_empty(),
                error: slice.error_message(),
                retriable: slice.is_retriable(),
                on_retry: move |_| on_retry.call(()),
            }
            if let Some(result) = slice.loaded().cloned() {
                match form.mode {
                    SliceMode::Heatmap => rsx! { Heatmap { result, cell: form.cell } },
                    SliceMode::Table => rsx! { SliceTable { result, cell: form.cell } },
                }
            }
        }
    }
}

#[component]
fn RangeField(
    label: &'static str,
    from: String,
    to: String,
    on_from: EventHandler<String>,
    on_to: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "field range-field",
            span { class: "field-label", "{label}" }
            div { class: "range-inputs",
                input {
                    class: "input",
                    inputmode: "decimal",
                    "aria-label": "{label} from",
                    value: "{from}",
                    oninput: move |e| on_from.call(e.value()),
                }
                span { "–" }
                input {
                    class: "input",
                    inputmode: "decimal",
                    "aria-label": "{label} to",
                    value: "{to}",
                    oninput: move |e| on_to.call(e.value()),
                }
            }
        }
    }
}

#[component]
pub fn DimensionsTable(schema: ArraySchema) -> Element {
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt",
                thead { tr { th { "Dimension" } th { "From" } th { "To" } th { class: "num", "Step" } th { class: "num", "Points" } th { class: "num", "Chunk" } th { "Unit" } } }
                tbody {
                    for d in schema.dimensions.iter() {
                        tr { key: "{d.name}",
                            td { class: "key", "{d.name}" }
                            td { "{d.start}" }
                            td { "{d.end}" }
                            td { class: "num", "{d.step}" }
                            td { class: "num", "{d.points}" }
                            td { class: "num", "{d.chunk}" }
                            td { "{d.unit}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn AttributesTable(schema: ArraySchema) -> Element {
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt",
                thead { tr { th { "Attribute" } th { "Type" } th { "Unit" } } }
                tbody {
                    for a in schema.attributes.iter() {
                        tr { key: "{a.name}", td { class: "key", "{a.name}" } td { "{a.dtype}" } td { "{a.unit}" } }
                    }
                }
            }
        }
    }
}

fn join(values: &[u32]) -> String {
    values
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" × ")
}

#[component]
pub fn ArrayMetadata(schema: ArraySchema) -> Element {
    let cells: u64 = schema.shape().iter().map(|n| u64::from(*n)).product();
    rsx! {
        div { class: "kv array-meta",
            div { class: "kv-row", span { "Array" } span { "{schema.array}" } }
            div { class: "kv-row", span { "Shape" } span { "{join(&schema.shape())}" } }
            div { class: "kv-row", span { "Chunk shape" } span { "{join(&schema.chunk_shape())}" } }
            div { class: "kv-row", span { "Cells" } span { "{thousands(cells)}" } }
            div { class: "kv-row", span { "Attributes" } span { "{schema.attributes.len()}" } }
        }
    }
}

/// The details panel of an array: its extents and the selected cell.
#[component]
pub fn ArrayDetails(schema: ArraySchema, slice: AsyncState<SliceResult>) -> Element {
    let view = use_context::<Signal<RecordView>>();
    let prefs = use_context::<Signal<Preferences>>();
    let offset = prefs.read().utc_offset_minutes;
    let form = view.read().slice.clone();
    let unit = schema
        .attributes
        .iter()
        .find(|a| a.name == form.attribute)
        .map(|a| a.unit.clone())
        .unwrap_or_default();
    let dtype = schema
        .attributes
        .iter()
        .find(|a| a.name == form.attribute)
        .map(|a| a.dtype.clone())
        .unwrap_or_default();
    let picked = slice.loaded().and_then(|r| {
        let (row, col) = form.cell.and_then(|c| locate(r, c))?;
        let lat = r.lats[row];
        let hemisphere = if lat >= 0.0 { "N" } else { "S" };
        Some((lat, hemisphere, r.lons[col], r.cell(row, col)?))
    });
    let time = hour_label(&schema, form.applied.time_index, offset);
    let extent = |name: &str| {
        schema
            .dimension(name)
            .map_or_else(String::new, |d| format!("{} .. {}", d.start, d.end))
    };
    let detail = |name: &str| {
        schema.dimension(name).map_or_else(String::new, |d| {
            if name == "time" {
                format!("({} steps, 1 hour)", d.points)
            } else {
                format!("({} points, {}°)", d.points, d.step)
            }
        })
    };
    rsx! {
        div { class: "inspector",
            h4 { class: "inspector-section first", "Dimensions" }
            div { class: "kv kv-stack",
                for name in ["time", "lat", "lon"] {
                    div { key: "{name}", class: "kv-row",
                        span { "{name}" }
                        span { class: "dim-value", "{extent(name)}" span { class: "meta", "{detail(name)}" } }
                    }
                }
            }
            div { class: "kv kv-compact",
                div { class: "kv-row", span { "Shape" } span { "{join(&schema.shape())}" } }
                div { class: "kv-row", span { "Chunk shape" } span { "{join(&schema.chunk_shape())}" } }
                div { class: "kv-row", span { "Data type" } span { "{dtype}" } }
                div { class: "kv-row", span { "Units" } span { "{unit}" } }
            }
            h4 { class: "inspector-section", "Selected cell" }
            if let Some((lat, hemisphere, lon, v)) = picked {
                div { class: "kv kv-compact",
                    div { class: "kv-row", span { "time" } span { "{time}" } }
                    div { class: "kv-row", span { "lat" } span { "{lat}° {hemisphere}" } }
                    div { class: "kv-row", span { "lon" } span { "{lon}°" } }
                    div { class: "kv-row", span { "{form.applied.attribute}" } span { "{v} {unit}" } }
                }
            } else {
                p { class: "inspector-note", "Select a cell of the slice to read its value." }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::array_data::SliceRequest;
    use crate::services::MockConnectionService;
    use crate::services::explorer_data::ExplorerData;

    async fn reference() -> SliceResult {
        MockConnectionService::ready()
            .array_slice(
                "climate_cube",
                &SliceRequest {
                    time_index: 12,
                    lat: (40.0, 41.0),
                    lon: (-74.0, -73.0),
                    attribute: "temperature_c".into(),
                },
            )
            .await
            .expect("slice")
    }

    #[test]
    fn the_scale_runs_from_blue_to_red() {
        assert_eq!(color(0.0), "rgb(0,0,128)");
        assert_eq!(color(1.0), "rgb(128,0,0)");
        assert_ne!(color(0.5), color(0.0));
    }

    #[tokio::test]
    async fn a_cell_in_tenths_finds_its_row_and_column() {
        let r = reference().await;
        assert_eq!(locate(&r, (405, -735)), Some((5, 5)));
        assert_eq!(locate(&r, (999, -735)), None);
    }

    #[test]
    fn the_legend_names_the_quantity_and_its_unit() {
        assert_eq!(legend_title("temperature_c", "°C"), "Temperature (°C)");
    }

    #[tokio::test]
    async fn hours_read_in_the_users_zone() {
        let s = MockConnectionService::ready()
            .array_schema("climate_cube")
            .await
            .expect("schema");
        assert_eq!(hour_label(&s, 12, 0), "2026-06-13 12:00:00 (UTC)");
        assert_eq!(hour_label(&s, 12, 480), "2026-06-13 20:00:00 (UTC+08:00)");
    }

    #[test]
    fn dimensions_join_with_a_times_sign() {
        assert_eq!(join(&[24, 181, 360]), "24 × 181 × 360");
    }
}
