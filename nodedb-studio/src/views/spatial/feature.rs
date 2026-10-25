//! The Feature panel: one selected feature, read-only. Attributes shows the
//! record's values, Geometry shows the stored geometry and its checks, and
//! Source says where it comes from. Editing a feature stays in Explorer.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::Panel;
use crate::components::tabs::TabBar;
use crate::models::format::thousands;
use crate::models::geo_ops::{Shape, Validity};
use crate::models::spatial::{Feature, FeatureRef, GeoSource, SpatialSetup};
use crate::models::spatial_text::crs_unit;
use crate::routes::Route;
use crate::routes::tabs::SpatialTab;
use crate::services::backend::Backend;
use crate::state::explorer::{RecordView, Selected, select_record};
use crate::state::spatial::{FeatureTab, SpatialUi};
use crate::state::workspace::Surface;

const SURFACE: Surface = Surface::Spatial(SpatialTab::Map);

fn unknown_or(v: Option<String>) -> String {
    v.unwrap_or_else(|| "Unknown".to_string())
}

fn kind_text(shape: &Shape, source: Option<&GeoSource>) -> String {
    let ordinates = source.map_or("XY", |s| s.ordinates.as_str());
    format!("{} · {ordinates}", shape.kind())
}

/// The coordinate rows of a shape: longitude and latitude for a point, the
/// vertex count and extent for anything else.
fn coordinate_rows(shape: &Shape) -> Vec<(String, String)> {
    match shape {
        Shape::Point(x, y) => vec![
            ("Longitude".to_string(), format!("{x}")),
            ("Latitude".to_string(), format!("{y}")),
        ],
        Shape::Polygon(rings) => {
            let mut rows = vec![
                ("Rings".to_string(), rings.len().to_string()),
                ("Vertices".to_string(), shape.vertices().to_string()),
            ];
            if let Some(b) = shape.bbox() {
                rows.push((
                    "West, south".to_string(),
                    format!("{}, {}", b.west, b.south),
                ));
                rows.push((
                    "East, north".to_string(),
                    format!("{}, {}", b.east, b.north),
                ));
            }
            rows
        }
    }
}

#[component]
fn Rows(rows: Vec<(String, String)>) -> Element {
    rsx! {
        dl { class: "sp-kv",
            for (k , v) in rows {
                div { key: "{k}", class: "sp-kv-row",
                    dt { "{k}" }
                    dd { "{v}" }
                }
            }
        }
    }
}

#[component]
fn Representation(label: String, text: String) -> Element {
    let copier = use_copier();
    rsx! {
        div { class: "sp-repr",
            div { class: "sp-repr-head",
                span { "{label}" }
                button {
                    class: "icon-btn sm",
                    title: "Copy {label}",
                    "aria-label": "Copy {label}",
                    onclick: {
                        let t = text.clone();
                        move |_| copier.copy(t.clone())
                    },
                    Ico { icon: Icon::Copy, size: 14 }
                }
            }
            pre { class: "code-block sp-repr-text", "{text}" }
        }
    }
}

#[component]
pub fn FeaturePanel(setup: SpatialSetup) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SpatialUi>();
    let nav = use_navigator();
    let mut explorer = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let mut map = ui.map;

    let (sel, tab, checked, check_error) = {
        let m = map.read();
        (
            m.selected.clone(),
            m.feature_tab,
            m.validity.clone(),
            m.validity_error.clone(),
        )
    };
    let feature_read = {
        let backend = backend.clone();
        use_resource(use_reactive!(|sel| {
            let backend = backend.clone();
            let _conn = conn();
            async move {
                match sel {
                    Some(r) => backend.spatial_feature(&r).await.map(Some),
                    None => Ok(None),
                }
            }
        }))
    };
    let state = resource_state(&feature_read);
    let feature: Option<Feature> = state.loaded().cloned().flatten();
    let source = sel.as_ref().and_then(|r| setup.source(&r.source)).cloned();
    let tabs: Vec<&'static str> = FeatureTab::ALL.iter().map(|t| t.label()).collect();
    let active = FeatureTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);

    rsx! {
        Panel { surface: SURFACE, id: "feature", title: "Feature", class: "fill",
            if sel.is_none() {
                div { class: "sp-empty", "Select a feature on the map to inspect it." }
            } else {
                AsyncView {
                    loading: state.is_loading(),
                    empty: false,
                    error: state.error_message(),
                    retriable: false,
                }
            }
            if let (Some(f), Some(r)) = (feature, sel.clone()) {
                div { class: "sp-feature",
                    div { class: "sp-feature-head",
                        h3 { class: "sp-feature-title", "{f.title}" }
                        div { class: "sp-feature-sub", "{f.id} · {r.source.collection}" }
                    }
                    TabBar {
                        class: "sub",
                        items: tabs,
                        active,
                        on_select: move |i: usize| {
                            if let Some(t) = FeatureTab::ALL.get(i) {
                                map.write().feature_tab = *t;
                            }
                        },
                    }
                    div { class: "sp-feature-body",
                        match tab {
                            FeatureTab::Attributes => rsx! {
                                Rows { rows: f.props.clone() }
                                h4 { class: "sp-section", "Geometry" }
                                Rows {
                                    rows: {
                                        let mut rows = vec![("Type".to_string(), kind_text(&f.shape, source.as_ref()))];
                                        rows.extend(coordinate_rows(&f.shape));
                                        rows
                                    },
                                }
                            },
                            FeatureTab::Geometry => rsx! {
                                GeometryTab {
                                    feature: f.clone(),
                                    source: source.clone(),
                                    checked: checked.clone().filter(|(c, _)| *c == r).map(|(_, v)| v),
                                    error: check_error.clone(),
                                    on_check: {
                                        let (backend, r) = (backend.clone(), r.clone());
                                        move |_| {
                                            let (backend, r) = (backend.clone(), r.clone());
                                            spawn_forever(async move {
                                                let out = backend.spatial_validity(&r).await;
                                                let mut m = map.write();
                                                // A late answer for an earlier selection is dropped.
                                                if m.selected.as_ref() != Some(&r) {
                                                    return;
                                                }
                                                match out {
                                                    Ok(v) => {
                                                        m.validity = Some((r, v));
                                                        m.validity_error = None;
                                                    }
                                                    Err(e) => m.validity_error = Some(e.to_string()),
                                                }
                                            });
                                        }
                                    },
                                }
                            },
                            FeatureTab::Source => rsx! { SourceTab { feature: r.clone(), source: source.clone() } },
                        }
                    }
                    div { class: "sp-feature-actions",
                        button {
                            class: "btn",
                            title: "Open this record in Explorer, where it can be edited",
                            onclick: {
                                let r = r.clone();
                                move |_| {
                                    select_record(&mut explorer, &mut view, &r.source.collection, &r.id);
                                    nav.push(Route::Explorer {});
                                }
                            },
                            Ico { icon: Icon::ExternalLink, size: 16 }
                            "Open in Explorer"
                        }
                        {
                            let point = match f.shape {
                                Shape::Point(x, y) => Some((x, y)),
                                Shape::Polygon(_) => None,
                            };
                            rsx! {
                                button {
                                    class: "btn",
                                    disabled: point.is_none(),
                                    title: if point.is_some() { "Start a Nearby analysis at this point. It does not run." } else { "Nearby needs a point. This feature is not one." },
                                    onclick: {
                                        let source = r.source.clone();
                                        move |_| {
                                            if let Some((x, y)) = point {
                                                ui.analyze_nearby(&source, x, y);
                                                nav.push(Route::Spatial { tab: SpatialTab::Analyze });
                                            }
                                        }
                                    },
                                    Ico { icon: Icon::ChartColumn, size: 16 }
                                    "Analyze nearby"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GeometryTab(
    feature: Feature,
    source: Option<GeoSource>,
    checked: Option<Validity>,
    error: Option<String>,
    on_check: EventHandler<()>,
) -> Element {
    let srid = source.as_ref().and_then(|s| s.srid);
    let mut rows = vec![
        ("Type".to_string(), feature.shape.kind().to_string()),
        (
            "Ordinates".to_string(),
            source
                .as_ref()
                .map_or("XY".to_string(), |s| s.ordinates.clone()),
        ),
        (
            "SRID".to_string(),
            unknown_or(srid.map(|s| format!("EPSG:{s}"))),
        ),
        (
            "CRS units".to_string(),
            unknown_or(srid.and_then(crs_unit).map(str::to_string)),
        ),
    ];
    rows.extend(coordinate_rows(&feature.shape));
    let (validity, tone) = match &checked {
        Some(v @ Validity::Valid) => (v.reason(), "ok"),
        Some(v @ Validity::Invalid(_)) => (format!("Invalid: {}", v.reason()), "err"),
        None => ("Not checked".to_string(), "idle"),
    };
    rsx! {
        Rows { rows }
        div { class: "sp-validity",
            span { class: "sp-validity-label", "Validity" }
            span { class: "sp-validity-value {tone}", "{validity}" }
            button {
                class: "btn sm",
                title: "Run ST_IsValid and ST_IsValidReason on the stored geometry",
                onclick: move |_| on_check.call(()),
                "Check validity"
            }
        }
        if let Some(e) = error {
            div { class: "callout err", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                span { "{e}" }
            }
        }
        h4 { class: "sp-section", "Representations" }
        Representation { label: "WKT", text: feature.shape.wkt() }
        if let Some(s) = srid {
            Representation { label: "EWKT", text: feature.shape.ewkt(s) }
        }
        Representation { label: "GeoJSON", text: feature.shape.geojson() }
    }
}

#[component]
fn SourceTab(feature: FeatureRef, source: Option<GeoSource>) -> Element {
    let Some(s) = source else {
        return rsx! { div { class: "sp-empty", "The catalog does not list this source." } };
    };
    let count = |v: Option<u64>| unknown_or(v.map(thousands));
    let mut rows = vec![
        ("Collection".to_string(), feature.source.collection.clone()),
        ("Geometry field".to_string(), feature.source.field.clone()),
        ("Storage".to_string(), s.storage.clone()),
        (
            "SRID".to_string(),
            unknown_or(s.srid.map(|v| format!("EPSG:{v}"))),
        ),
        (
            "CRS units".to_string(),
            unknown_or(s.srid.and_then(crs_unit).map(str::to_string)),
        ),
        ("Ordinates".to_string(), s.ordinates.clone()),
        ("Geometries".to_string(), count(s.geometry_count)),
        ("Records".to_string(), count(s.record_count)),
    ];
    if s.indexes.is_empty() {
        rows.push(("Geometry index".to_string(), "None listed".to_string()));
    }
    for i in &s.indexes {
        rows.push((
            "Geometry index".to_string(),
            format!("{} ({})", i.name, i.kind),
        ));
    }
    rsx! {
        dl { class: "sp-kv",
            for (n , (k , v)) in rows.into_iter().enumerate() {
                div { key: "{n}", class: "sp-kv-row",
                    dt { "{k}" }
                    dd { "{v}" }
                }
            }
        }
    }
}
