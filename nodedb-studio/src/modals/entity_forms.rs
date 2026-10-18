//! Entity insert/edit modal bodies, one per engine: document, strict, vector,
//! graph node, graph edge, KV and spatial.
//!
//! SEAM-UNWIRED: static mockup parity. Every field shows the mockup's fixture
//! record, and Save only closes the modal. No seam method writes a record yet,
//! and nothing selects one until the Explorer's master-detail lands. Both
//! arrive together; the record then travels in `ModalKind` and Save goes
//! through `use_action`.

use dioxus::prelude::*;

use crate::state::ui::ModalKind;

const DOC_JSON: &str = r#"{
  "_id": "evt_01H8QXG2K…",
  "type": "page_view",
  "user_id": "u_44182",
  "ts": "2026-06-13T04:22:18Z",
  "props": {
    "path": "/dashboard",
    "referrer": "google",
    "ms_to_load": 348,
    "viewport": { "w": 1440, "h": 900 }
  },
  "tags": ["web", "mobile"]
}"#;

const VECTOR_FLOATS: &str = "[0.041, -0.218, 0.110, 0.082, -0.044, 0.198, 0.012, -0.107, 0.295, -0.018, 0.142, -0.231, 0.088, 0.005, -0.140, 0.211, 0.038, -0.092, 0.167, -0.024, 0.103, … 748 more]";

const EDGE_PROPS: &str = r#"{
  "since": "2026-06-13T04:22:18Z"
}"#;

const KV_JSON: &str = r#"{
  "user": "u_44182",
  "since": "2026-06-13T04:00:18Z",
  "device": "macbook",
  "ip": "103.42.18.x",
  "last_seen": "2026-06-13T04:22:18Z"
}"#;

const GEOJSON: &str = r#"{
  "type": "Feature",
  "geometry": {
    "type": "Point",
    "coordinates": [-2.241, 53.481]
  },
  "properties": {
    "name": "Manchester Office",
    "category": "office",
    "headcount": 142,
    "opened": "2022-04-01"
  }
}"#;

/// Cancel + one primary action. Both close the modal until writes exist.
#[component]
fn SaveFooter(#[props(default = "Save".to_string())] label: String) -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    rsx! {
        div { class: "modal-footer",
            button { class: "btn ghost", onclick: move |_| modal.set(None), "Cancel" }
            button { class: "btn primary", onclick: move |_| modal.set(None), "{label}" }
        }
    }
}

/// Status line under a form: an ok pill plus optional mono detail.
#[component]
fn CheckLine(pill: String, #[props(default)] detail: String) -> Element {
    rsx! {
        div { class: "form-check",
            span { class: "pill ok", span { class: "dot" } "{pill}" }
            if !detail.is_empty() {
                span { class: "mono", "{detail}" }
            }
        }
    }
}

#[component]
pub fn DocForm() -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    rsx! {
        div { class: "modal-vtabs",
            div { class: "vtab active", "JSON" }
            div { class: "vtab", "Form" }
        }
        div { class: "modal-body",
            div { class: "form-field",
                label { "Document JSON" }
                textarea { class: "json-editor", value: DOC_JSON }
            }
            CheckLine { pill: "valid JSON", detail: "412 bytes · 14 fields · 1 nested object · 1 array" }
            div { class: "tbd-block",
                strong { "TBD" }
                " · Schema validation feedback. If NodeDB Document supports optional validation rules, this modal should surface validation errors inline (\"field "
                code { "props.ms_to_load" }
                " must be a non-negative integer\"). Currently no schema = always accepts."
            }
        }
        div { class: "modal-footer",
            button { class: "btn ghost", onclick: move |_| modal.set(None), "Cancel" }
            button { class: "btn", "Save as draft" }
            button { class: "btn primary", onclick: move |_| modal.set(None), "Save" }
        }
    }
}

#[component]
pub fn StrictForm() -> Element {
    rsx! {
        div { class: "modal-body",
            div { class: "form-field",
                label { "id " span { class: "field-hint", "int8 · pk · readonly" } }
                input { value: "442003", disabled: true }
            }
            div { class: "form-row",
                div { class: "form-field",
                    label { "customer_id " span { class: "field-hint", "string · fk" } }
                    input { value: "u_44182" }
                }
                div { class: "form-field",
                    label { "status " span { class: "field-hint", "enum" } }
                    select {
                        option { "pending" }
                        option { "processing" }
                        option { selected: true, "shipped" }
                        option { "cancelled" }
                        option { "refunded" }
                    }
                }
            }
            div { class: "form-row",
                div { class: "form-field",
                    label { "total " span { class: "field-hint", "decimal(10,2)" } }
                    input { r#type: "number", step: "0.01", value: "129.40" }
                }
                div { class: "form-field",
                    label { "currency " span { class: "field-hint", "char(3)" } }
                    input { maxlength: "3", value: "USD" }
                }
            }
            div { class: "form-row",
                div { class: "form-field",
                    label { "placed_at " span { class: "field-hint", "timestamptz" } }
                    input { value: "2026-06-12 18:04:21+08" }
                }
                div { class: "form-field",
                    label { "shipped_at " span { class: "field-hint", "timestamptz · nullable" } }
                    input { value: "2026-06-13 09:12:00+08" }
                }
            }
            CheckLine { pill: "all checks pass", detail: "total ≥ 0 ✓ · shipped_at ≥ placed_at ✓" }
        }
        SaveFooter {}
    }
}

#[component]
pub fn VectorForm() -> Element {
    rsx! {
        div { class: "modal-vtabs",
            div { class: "vtab active", "Paste vector" }
            div { class: "vtab disabled", "Embed from text " span { class: "tbd-note", "TBD" } }
        }
        div { class: "modal-body",
            div { class: "form-field",
                label { "id" }
                input { value: "e_001", disabled: true }
            }
            div { class: "form-field",
                label { "Vector " span { class: "field-hint", "768 floats, comma-separated" } }
                textarea { class: "json-editor", style: "min-height: 140px;", value: VECTOR_FLOATS }
            }
            div { class: "form-row",
                div { class: "form-field", label { "source_doc" } input { value: "handbook.md#42" } }
                div { class: "form-field", label { "model" } input { value: "all-mpnet-base-v2" } }
            }
            div { class: "form-field",
                label { "chunk text" }
                textarea {
                    style: "min-height: 60px;",
                    value: "Engine-aware viewers open a purpose-built UI per database engine type.",
                }
            }
            CheckLine { pill: "vector valid · 768d · norm 1.0" }
        }
        SaveFooter {}
    }
}

#[component]
pub fn GraphNodeForm() -> Element {
    rsx! {
        div { class: "modal-body",
            div { class: "form-row",
                div { class: "form-field",
                    label { "_id " span { class: "field-hint", "readonly" } }
                    input { value: "u_44182", disabled: true }
                }
                div { class: "form-field",
                    label { "label" }
                    select { option { selected: true, "User" } option { "Post" } option { "Topic" } }
                }
            }
            div { class: "form-field",
                label { "name " span { class: "field-hint required", "required" } }
                input { value: "Aisha Tan" }
            }
            div { class: "form-row",
                div { class: "form-field", label { "country" } input { value: "MY" } }
                div { class: "form-field",
                    label { "joined " span { class: "field-hint required", "required" } }
                    input { r#type: "date", value: "2024-03-11" }
                }
            }
            div { class: "form-field",
                label { "verified " span { class: "field-hint", "bool" } }
                select { option { selected: true, "true" } option { "false" } }
            }
            div { class: "form-note",
                "Property fields are derived from the "
                strong { "User" }
                " label schema. To add a property to all User nodes, edit the schema in Designer."
            }
        }
        SaveFooter {}
    }
}

#[component]
pub fn GraphEdgeForm() -> Element {
    rsx! {
        div { class: "modal-body",
            div { class: "form-row three",
                div { class: "form-field", label { "From (node _id)" } input { placeholder: "u_44182" } }
                div { class: "form-field",
                    label { "type" }
                    select {
                        option { "POSTED" }
                        option { selected: true, "FOLLOWS" }
                        option { "LIKES" }
                        option { "TAGGED" }
                    }
                }
                div { class: "form-field", label { "To (node _id)" } input { placeholder: "u_77103" } }
            }
            div { class: "form-field",
                label { "Properties " span { class: "field-hint", "FOLLOWS edges accept: since (timestamp)" } }
                textarea { class: "json-editor", style: "min-height: 100px;", value: EDGE_PROPS }
            }
            div { class: "form-note",
                "Edge property schema is derived from the "
                strong { "FOLLOWS" }
                " edge type."
            }
        }
        SaveFooter { label: "Create edge" }
    }
}

#[component]
pub fn KvForm() -> Element {
    rsx! {
        div { class: "modal-body",
            div { class: "form-field", label { "key" } input { value: "session:u_44182" } }
            div { class: "form-row",
                div { class: "form-field",
                    label { "type" }
                    select {
                        option { selected: true, "json" }
                        option { "string" }
                        option { "int" }
                        option { "bool" }
                        option { "bytes" }
                        option { "counter" }
                    }
                }
                div { class: "form-field",
                    label { "TTL " span { class: "field-hint", "none = no expiry" } }
                    input { value: "14m", placeholder: "e.g. 30s, 5m, 2h, 1d" }
                }
            }
            div { class: "form-field",
                label { "value" }
                textarea { class: "json-editor", style: "min-height: 200px;", value: KV_JSON }
            }
            CheckLine { pill: "valid JSON · 2.1 KB" }
        }
        SaveFooter {}
    }
}

#[component]
pub fn SpatialForm() -> Element {
    rsx! {
        div { class: "modal-vtabs",
            div { class: "vtab active", "Paste GeoJSON" }
            div { class: "vtab disabled", "Draw on map " span { class: "tbd-note", "TBD" } }
        }
        div { class: "modal-body",
            div { class: "form-field",
                label { "id" }
                input { value: "feature_8281", disabled: true }
            }
            div { class: "form-field",
                label { "GeoJSON " span { class: "field-hint", "SRID 4326 (WGS84)" } }
                textarea { class: "json-editor", style: "min-height: 240px;", value: GEOJSON }
            }
            CheckLine { pill: "valid GeoJSON · Point · in bbox" }
            div { class: "tbd-block",
                strong { "TBD" }
                " · Drawing library for the \"Draw on map\" tab. Affects whether Leaflet.draw, Mapbox GL Draw, or a custom Rust drawing layer is chosen."
            }
        }
        SaveFooter {}
    }
}
