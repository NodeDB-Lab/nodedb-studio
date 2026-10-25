//! The Analysis panel: the operation and its inputs, two rows. These are
//! query inputs, never edits to a source geometry.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::geo_ops::{Bbox, Ring, Shape};
use crate::models::spatial::{GeoSource, SpatialSetup};
use crate::models::spatial_analyze::{
    AnalysisForm, AreaKind, Availability, DistanceUnit, Issue, Operation,
};
use crate::state::spatial::{AnalyzeDraft, AnalyzeUi, Draft, SpatialUi};

/// An area the Map holds: its ring, and its box when it is a rectangle.
type MapArea = (Ring, Option<Bbox>);

fn edit(mut analyze: Signal<AnalyzeUi>, key: &str, f: impl FnOnce(&mut AnalysisForm)) {
    let mut a = analyze.write();
    let d = a.draft_mut(key);
    f(&mut d.form);
    d.tried = false;
}

fn bad(issues: &[Issue], tried: bool, field: &str) -> bool {
    tried && issues.iter().any(|i| i.field == field)
}

fn input_class(invalid: bool) -> &'static str {
    if invalid {
        "input mono invalid"
    } else {
        "input mono"
    }
}

#[component]
pub fn ComposerBody(
    setup: SpatialSetup,
    source: GeoSource,
    draft: AnalyzeDraft,
    issues: Vec<Issue>,
    running: bool,
    on_run: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let ui = use_context::<SpatialUi>();
    let analyze = ui.analyze;
    let key = source.source.key();
    let form = draft.form.clone();
    let tried = draft.tried;

    // What the Map holds that a form can take: a selected point, and a
    // finished drawn area.
    let (map_point, map_area): (Option<(f64, f64)>, Option<MapArea>) = {
        let m = ui.map.read();
        let point = m
            .selected
            .as_ref()
            .filter(|r| r.source == source.source)
            .and_then(|r| m.shown.as_ref()?.feature(r))
            .and_then(|f| match f.shape {
                Shape::Point(x, y) => Some((x, y)),
                Shape::Polygon(_) => None,
            });
        let area = m.draft.as_ref().filter(|d| d.done()).and_then(|d| {
            let ring = d.ring()?;
            let rect = match d {
                Draft::Rectangle { from, to, .. } => Some(Bbox::new(
                    from.0.min(to.0),
                    from.1.min(to.1),
                    from.0.max(to.0),
                    from.1.max(to.1),
                )),
                Draft::Polygon { .. } => None,
            };
            Some((ring, rect))
        });
        (point, area)
    };

    let unsupported = setup
        .operations
        .iter()
        .find(|o| o.op == form.op)
        .and_then(|o| match &o.availability {
            Availability::Unsupported(why) => Some(why.clone()),
            Availability::Supported => None,
        });
    let srid = source
        .srid
        .map_or_else(|| "unknown SRID".to_string(), |s| format!("EPSG:{s}"));
    let needs_area = form.op.needs_area();
    let nearby = form.op == Operation::Nearby;

    rsx! {
        div { class: "sp-form",
            div { class: "sp-row",
                div { class: "field sp-op",
                    label { r#for: "sp-op", "Operation" }
                    select {
                        id: "sp-op",
                        class: "select native",
                        onchange: {
                            let key = key.clone();
                            move |e| {
                                if let Some(op) = Operation::ALL.iter().find(|o| o.label() == e.value()) {
                                    edit(analyze, &key, |f| f.op = *op);
                                }
                            }
                        },
                        for o in setup.operations.iter() {
                            option {
                                key: "{o.op.label()}",
                                value: "{o.op.label()}",
                                selected: o.op == form.op,
                                if matches!(o.availability, Availability::Supported) {
                                    "{o.op.label()}"
                                } else {
                                    "{o.op.label()} (unavailable)"
                                }
                            }
                        }
                    }
                    span { class: "sp-fn mono", title: "{form.op.operands()}", "{form.op.function()}" }
                }
                if nearby {
                    div { class: "field sp-radius",
                        label { r#for: "sp-radius", "Radius" }
                        input {
                            id: "sp-radius",
                            class: input_class(bad(&issues, tried, "radius")),
                            inputmode: "decimal",
                            value: "{form.radius}",
                            oninput: {
                                let key = key.clone();
                                move |e| edit(analyze, &key, |f| f.radius = e.value())
                            },
                        }
                    }
                    div { class: "field sp-unit",
                        label { r#for: "sp-unit", "Unit" }
                        select {
                            id: "sp-unit",
                            class: "select native",
                            title: "The query always runs in metres. The radius converts first.",
                            onchange: {
                                let key = key.clone();
                                move |e| {
                                    if let Some(u) = DistanceUnit::ALL.iter().find(|u| u.label() == e.value()) {
                                        edit(analyze, &key, |f| f.unit = *u);
                                    }
                                }
                            },
                            for u in DistanceUnit::ALL {
                                option { key: "{u.label()}", value: "{u.label()}", selected: u == form.unit, "{u.label()}" }
                            }
                        }
                    }
                }
                div { class: "field sp-limit",
                    label { r#for: "sp-limit", "Limit" }
                    input {
                        id: "sp-limit",
                        class: input_class(bad(&issues, tried, "limit")),
                        inputmode: "numeric",
                        value: "{form.limit}",
                        oninput: {
                            let key = key.clone();
                            move |e| edit(analyze, &key, |f| f.limit = e.value())
                        },
                    }
                }
                span { class: "toolbar-spacer" }
                if running {
                    button { class: "btn", onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                } else {
                    button {
                        class: "btn primary",
                        disabled: unsupported.is_some(),
                        title: if unsupported.is_some() { "This operation is unavailable. The draft is kept." } else { "Run the analysis. Changing the form never runs it." },
                        onclick: move |_| on_run.call(()),
                        Ico { icon: Icon::Play, size: 16 }
                        "Run"
                    }
                }
            }
            if nearby {
                div { class: "sp-row",
                    div { class: "field sp-origin",
                        label { r#for: "sp-origin", "Origin" }
                        select {
                            id: "sp-origin",
                            class: "select native",
                            title: "The origin is a point in the source's CRS: {srid}",
                            option { value: "point", selected: true, "Point" }
                        }
                    }
                    div { class: "field sp-coord",
                        label { r#for: "sp-lon", "Longitude" }
                        input {
                            id: "sp-lon",
                            class: input_class(bad(&issues, tried, "lon")),
                            inputmode: "decimal",
                            value: "{form.lon}",
                            oninput: {
                                let key = key.clone();
                                move |e| edit(analyze, &key, |f| f.lon = e.value())
                            },
                        }
                    }
                    div { class: "field sp-coord",
                        label { r#for: "sp-lat", "Latitude" }
                        input {
                            id: "sp-lat",
                            class: input_class(bad(&issues, tried, "lat")),
                            inputmode: "decimal",
                            value: "{form.lat}",
                            oninput: {
                                let key = key.clone();
                                move |e| edit(analyze, &key, |f| f.lat = e.value())
                            },
                        }
                    }
                    button {
                        class: "btn sp-use",
                        disabled: map_point.is_none(),
                        title: if map_point.is_some() { "Take the point of the feature selected on the Map" } else { "Select a point feature on the Map first" },
                        onclick: {
                            let key = key.clone();
                            move |_| {
                                if let Some((x, y)) = map_point {
                                    edit(analyze, &key, |f| {
                                        f.lon = format!("{x}");
                                        f.lat = format!("{y}");
                                    });
                                }
                            }
                        },
                        "Use map selection"
                    }
                }
            }
            if needs_area {
                div { class: "sp-row",
                    div { class: "field sp-origin",
                        label { r#for: "sp-area", "Area" }
                        select {
                            id: "sp-area",
                            class: "select native",
                            onchange: {
                                let key = key.clone();
                                move |e| {
                                    let kind = if e.value() == AreaKind::Polygon.label() { AreaKind::Polygon } else { AreaKind::Rectangle };
                                    edit(analyze, &key, |f| f.area = kind);
                                }
                            },
                            for k in [AreaKind::Rectangle, AreaKind::Polygon] {
                                option { key: "{k.label()}", value: "{k.label()}", selected: k == form.area, "{k.label()}" }
                            }
                        }
                    }
                    if form.area == AreaKind::Rectangle {
                        for (id , label , value , field) in [
                            ("sp-west", "West", form.west.clone(), "west"),
                            ("sp-south", "South", form.south.clone(), "south"),
                            ("sp-east", "East", form.east.clone(), "east"),
                            ("sp-north", "North", form.north.clone(), "north"),
                        ] {
                            div { key: "{id}", class: "field sp-coord",
                                label { r#for: "{id}", "{label}" }
                                input {
                                    id: "{id}",
                                    class: input_class(bad(&issues, tried, field)),
                                    inputmode: "decimal",
                                    value: "{value}",
                                    oninput: {
                                        let key = key.clone();
                                        move |e| edit(analyze, &key, |f| match field {
                                            "west" => f.west = e.value(),
                                            "south" => f.south = e.value(),
                                            "east" => f.east = e.value(),
                                            _ => f.north = e.value(),
                                        })
                                    },
                                }
                            }
                        }
                    } else {
                        div { class: "field sp-grow",
                            span { class: "field-label", "Polygon" }
                            span { class: if bad(&issues, tried, "area") { "sp-static invalid" } else { "sp-static" },
                                if let Some(r) = form.polygon.as_ref() {
                                    "{r.len() - 1} vertices, drawn on the Map"
                                } else {
                                    "None drawn yet"
                                }
                            }
                        }
                    }
                    button {
                        class: "btn sp-use",
                        disabled: map_area.is_none(),
                        title: if map_area.is_some() { "Take the area drawn on the Map" } else { "Draw a rectangle or polygon on the Map first" },
                        onclick: {
                            let key = key.clone();
                            move |_| {
                                if let Some((ring, rect)) = map_area.clone() {
                                    edit(analyze, &key, |f| match rect {
                                        Some(b) => {
                                            f.area = AreaKind::Rectangle;
                                            f.west = format!("{}", b.west);
                                            f.south = format!("{}", b.south);
                                            f.east = format!("{}", b.east);
                                            f.north = format!("{}", b.north);
                                        }
                                        None => {
                                            f.area = AreaKind::Polygon;
                                            f.polygon = Some(ring.clone());
                                        }
                                    });
                                }
                            }
                        },
                        "Use map draft"
                    }
                }
            }
            if let Some(why) = unsupported {
                div { class: "callout warn", role: "status",
                    Ico { icon: Icon::TriangleAlert, size: 16 }
                    span { "{why} The draft is kept." }
                }
            } else if tried && !issues.is_empty() {
                ul { class: "sp-issues", role: "alert",
                    for i in issues.iter() {
                        li { key: "{i.field}", "{i.message}" }
                    }
                }
            } else if !nearby && !needs_area {
                div { class: "sp-hint", "This operation has no form yet." }
            }
        }
    }
}
