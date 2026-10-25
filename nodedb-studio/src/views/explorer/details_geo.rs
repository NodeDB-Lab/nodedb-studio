//! Spatial feature details: a position preview, the GeoJSON, and the
//! geometry facts.

use dioxus::prelude::*;
use nodedb_types::Value;
use nodedb_types::geometry::Geometry;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::collection::Collection;
use crate::models::record_json::pretty;
use crate::models::records::{CollectionAccess, Record};
use crate::models::schema::CollectionSchema;
use crate::models::spatial::SourceRef;
use crate::routes::Route;
use crate::routes::tabs::SpatialTab;
use crate::state::explorer::{DetailsTab, RecordView};
use crate::state::spatial::SpatialUi;
use crate::views::explorer::actions::use_launcher;
use crate::views::explorer::cells::text_label;
use crate::views::explorer::details::{DetailTabs, effective_tab};
use crate::views::explorer::details_vector::IndexList;
use crate::views::explorer::links::OpenLink;

/// The feature as GeoJSON. A point prints in the order GeoJSON documents
/// use: type, geometry, properties.
pub fn geojson_text(schema: &CollectionSchema, record: &Record) -> String {
    let geometry = schema.geometry().and_then(|(f, _)| record.get(&f.name));
    let props: Vec<String> = schema
        .fields
        .iter()
        .filter(|f| !matches!(f.ty, crate::models::schema::FieldType::Geometry { .. }))
        .map(|f| {
            let v = record.get(&f.name).map(text_label).unwrap_or_default();
            format!("    \"{}\": \"{}\"", f.name, v.replace('"', "\\\""))
        })
        .collect();
    match geometry {
        Some(Value::Geometry(Geometry::Point { coordinates })) => format!(
            "{{\n  \"type\": \"Feature\",\n  \"geometry\": {{\n    \"type\": \"Point\",\n    \"coordinates\": [{}, {}]\n  }},\n  \"properties\": {{\n{}\n  }}\n}}",
            coordinates[0],
            coordinates[1],
            props.join(",\n")
        ),
        Some(other) => pretty(other, &[]),
        None => "{}".to_string(),
    }
}

/// The point of a feature, as longitude and latitude.
fn point(schema: &CollectionSchema, record: &Record) -> Option<(f64, f64)> {
    match schema.geometry().and_then(|(f, _)| record.get(&f.name)) {
        Some(Value::Geometry(Geometry::Point { coordinates })) => {
            Some((coordinates[0], coordinates[1]))
        }
        _ => None,
    }
}

/// A small position preview: a graticule, the point, and its coordinates.
/// There is no basemap, so it shows where the point is, not what is there.
#[component]
fn MapPreview(lon: f64, lat: f64) -> Element {
    rsx! {
        div { class: "map-preview", role: "img", "aria-label": "Position preview at longitude {lon}, latitude {lat}",
            svg { view_box: "0 0 300 150", preserve_aspect_ratio: "xMidYMid slice",
                for i in 1..6 {
                    line { key: "v{i}", class: "grid-line", x1: "{i * 50}", y1: "0", x2: "{i * 50}", y2: "150" }
                }
                for i in 1..3 {
                    line { key: "h{i}", class: "grid-line", x1: "0", y1: "{i * 50}", x2: "300", y2: "{i * 50}" }
                }
                path { class: "pin", d: "M150 96 C150 96 134 74 134 62 A16 16 0 0 1 166 62 C166 74 150 96 150 96 Z" }
                circle { class: "pin-dot", cx: "150", cy: "62", r: "5" }
            }
            div { class: "coords",
                span { "Lon" }
                strong { "{lon}" }
                span { "Lat" }
                strong { "{lat}" }
            }
        }
    }
}

#[component]
pub fn SpatialDetails(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
) -> Element {
    let launcher = use_launcher();
    let copier = use_copier();
    let spatial = use_context::<SpatialUi>();
    let view = use_context::<Signal<RecordView>>();
    let tabs = DetailsTab::for_collection(&collection);
    let active = effective_tab(view.read().details_tab, &tabs).unwrap_or(DetailsTab::Properties);
    let can_update = access.as_ref().is_some_and(|a| a.update.allowed());
    let text = geojson_text(&schema, &record);
    let at = point(&schema, &record);
    let srid = schema.geometry().map(|(_, s)| s).unwrap_or_default();
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "{record.id}" }
                span { class: "badge gray", "{collection.name}" }
                span { class: "badge green", span { class: "status-dot ok" } "Feature" }
            }
            DetailTabs { tabs, active }
            match active {
                DetailsTab::Geometry => rsx! {
                    div { class: "kv",
                        div { class: "kv-row", span { "Type" } span { if at.is_some() { "Point" } else { "Geometry" } } }
                        div { class: "kv-row", span { "SRID" } span { "EPSG:{srid}" } }
                        if let Some((lon, lat)) = at {
                            div { class: "kv-row", span { "Longitude" } span { "{lon}" } }
                            div { class: "kv-row", span { "Latitude" } span { "{lat}" } }
                        }
                    }
                },
                DetailsTab::Indexes => rsx! { IndexList { indexes: schema.indexes.clone() } },
                _ => rsx! {
                    if let Some((lon, lat)) = at {
                        MapPreview { lon, lat }
                    }
                    div { class: "card geojson",
                        div { class: "card-head",
                            h4 { class: "inspector-section", "GeoJSON" }
                            button {
                                class: "icon-btn sm",
                                title: "Copy GeoJSON",
                                "aria-label": "Copy GeoJSON",
                                onclick: {
                                    let t = text.clone();
                                    move |_| copier.copy(t.clone())
                                },
                                Ico { icon: Icon::Copy, size: 14 }
                            }
                        }
                        pre { class: "code-block", "{text}" }
                    }
                },
            }
            div { class: "inspector-actions",
                button {
                    class: "btn",
                    disabled: !can_update,
                    title: if can_update { "" } else { "UPDATE permission required" },
                    onclick: {
                        let (c, s, r) = (collection.clone(), schema.clone(), record.clone());
                        move |_| launcher.edit(&c, &s, &r)
                    },
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit feature"
                }
                OpenLink {
                    route: Route::Spatial { tab: SpatialTab::Map },
                    label: "Open in Spatial".to_string(),
                    icon: Icon::MapPin,
                    class: "btn".to_string(),
                    on_open: {
                        let source = SourceRef::new(
                            &collection.name,
                            schema.geometry().map_or("geometry", |(f, _)| f.name.as_str()),
                        );
                        let id = record.id.clone();
                        move |_| spatial.open_feature(source.clone(), &id)
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::explorer_data::ExplorerData;

    async fn place() -> (CollectionSchema, Record) {
        let svc = MockConnectionService::ready();
        (
            svc.collection_schema("places").await.expect("schema"),
            svc.record("places", "place104").await.expect("record"),
        )
    }

    #[tokio::test]
    async fn a_point_prints_as_ordered_geojson() {
        let (schema, r) = place().await;
        let text = geojson_text(&schema, &r);
        let order: Vec<usize> = ["\"type\": \"Feature\"", "\"geometry\"", "\"properties\""]
            .iter()
            .filter_map(|k| text.find(k))
            .collect();
        assert_eq!(order.len(), 3);
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{text}");
        assert!(
            text.contains("\"coordinates\": [-73.9857, 40.7484]"),
            "{text}"
        );
        assert!(text.contains("\"name\": \"Cafe North Point\""), "{text}");
    }

    #[tokio::test]
    async fn the_output_is_valid_json() {
        let (schema, r) = place().await;
        let parsed: Result<nodedb_types::Value, _> = sonic_rs::from_str(&geojson_text(&schema, &r));
        assert!(parsed.is_ok());
    }
}
