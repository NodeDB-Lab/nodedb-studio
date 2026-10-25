//! How a record's values read in a table cell.
//!
//! Pure functions over `Value`: the same value always prints the same, in
//! the user's time zone for times.

use nodedb_types::Value;
use nodedb_types::geometry::Geometry;

use crate::models::format::date_second;
use crate::models::record_json::compact;
use crate::models::records::Record;
use crate::models::schema::{CellFormat, CollectionSchema, ColumnSpec};

/// A byte count: `5 B`, `2.1 KB`, `18 KB`, `1.4 MB`.
pub fn bytes_label(n: i64) -> String {
    let n = n.max(0) as f64;
    let scaled = |v: f64, unit: &str| {
        if v >= 10.0 {
            format!("{v:.0} {unit}")
        } else {
            format!("{v:.1} {unit}")
        }
    };
    if n < 1024.0 {
        format!("{n:.0} B")
    } else if n < 1024.0 * 1024.0 {
        scaled(n / 1024.0, "KB")
    } else {
        scaled(n / (1024.0 * 1024.0), "MB")
    }
}

/// Time to expiry: `45s`, `14m`, `2h`, `3d`, or `-` for none.
pub fn ttl_label(secs: Option<i64>) -> String {
    match secs {
        None => "-".to_string(),
        Some(s) if s < 60 => format!("{s}s"),
        Some(s) if s < 3600 => format!("{}m", s / 60),
        Some(s) if s < 86_400 => format!("{}h", s / 3600),
        Some(s) => format!("{}d", s / 86_400),
    }
}

/// `Point(-73.9857, 40.7484)`.
pub fn geometry_label(g: &Geometry) -> String {
    match g {
        Geometry::Point { coordinates } => {
            format!("Point({}, {})", coordinates[0], coordinates[1])
        }
        other => sonic_rs::to_string(other)
            .map_or_else(|_| "Geometry".to_string(), |t| t.chars().take(40).collect()),
    }
}

/// The first components of an embedding: `[0.021, -0.104, …]`.
pub fn embedding_label(v: &[f32]) -> String {
    let head: Vec<String> = v.iter().take(2).map(|f| format!("{f:.3}")).collect();
    format!("[{}, …]", head.join(", "))
}

const PREVIEW_CHARS: usize = 34;

/// A value on one line, cut at `PREVIEW_CHARS`.
pub fn preview_label(v: &Value) -> String {
    if matches!(v, Value::Bytes(_)) {
        return "(binary)".to_string();
    }
    let text = compact(v, &[]);
    if text.chars().count() > PREVIEW_CHARS {
        let cut: String = text.chars().take(PREVIEW_CHARS).collect();
        format!("{cut}…")
    } else {
        text
    }
}

/// A time in the user's zone, `2026-06-13 12:41:02`.
pub fn time_label(v: &Value, offset_min: i16) -> String {
    match v {
        Value::DateTime(t) | Value::NaiveDateTime(t) => date_second(t.unix_secs(), offset_min),
        Value::Null => String::new(),
        other => crate::models::record_json::compact(other, &[]),
    }
}

/// Plain text of a value, with no quoting.
pub fn text_label(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        other => compact(other, &[]),
    }
}

/// What one cell shows, as text. `Id` and the status formats show the same
/// text; the view decides how to dress it.
pub fn cell_text(
    column: &ColumnSpec,
    record: &Record,
    schema: &CollectionSchema,
    offset_min: i16,
) -> String {
    let value = record.get(&column.field);
    match (column.format, value) {
        (CellFormat::Srid, _) => schema
            .geometry()
            .map(|(_, srid)| format!("EPSG:{srid}"))
            .unwrap_or_default(),
        (_, None) => String::new(),
        (CellFormat::Timestamp, Some(v)) => time_label(v, offset_min),
        (CellFormat::Geometry, Some(Value::Geometry(g))) => geometry_label(g),
        (CellFormat::Embedding, Some(Value::Vector(v))) => embedding_label(v),
        (CellFormat::Bytes, Some(Value::Integer(n))) => bytes_label(*n),
        (CellFormat::Ttl, Some(Value::Integer(s))) => ttl_label(Some(*s)),
        (CellFormat::Ttl, Some(_)) => ttl_label(None),
        (CellFormat::Preview, Some(v)) => preview_label(v),
        (
            CellFormat::Text
            | CellFormat::Number
            | CellFormat::StatusDot
            | CellFormat::StatusBadge
            | CellFormat::Id
            | CellFormat::Geometry
            | CellFormat::Embedding
            | CellFormat::Bytes,
            Some(v),
        ) => text_label(v),
    }
}

/// The tone of a status value, for a dot or a badge.
pub fn status_tone(status: &str) -> &'static str {
    match status {
        "active" | "shipped" | "paid" | "delivered" | "completed" => "green",
        "processing" | "in_transit" | "running" => "blue",
        "pending" | "suspended" => "amber",
        "cancelled" | "failed" | "delayed" => "red",
        _ => "gray",
    }
}

/// The status-dot class for a tone.
pub fn dot_class(tone: &str) -> &'static str {
    match tone {
        "green" => "status-dot ok",
        "amber" => "status-dot warn",
        "red" => "status-dot err",
        "blue" => "status-dot info",
        _ => "status-dot idle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::explorer_data::ExplorerData;

    #[test]
    fn byte_counts_scale_and_drop_the_decimal_from_ten() {
        assert_eq!(bytes_label(5), "5 B");
        assert_eq!(bytes_label(2150), "2.1 KB");
        assert_eq!(bytes_label(18_432), "18 KB");
        assert_eq!(bytes_label(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn ttls_use_the_largest_whole_unit() {
        assert_eq!(ttl_label(None), "-");
        assert_eq!(ttl_label(Some(45)), "45s");
        assert_eq!(ttl_label(Some(840)), "14m");
        assert_eq!(ttl_label(Some(7200)), "2h");
        assert_eq!(ttl_label(Some(3 * 86_400)), "3d");
    }

    #[test]
    fn a_point_prints_longitude_first() {
        let g = Geometry::Point {
            coordinates: [-73.9857, 40.7484],
        };
        assert_eq!(geometry_label(&g), "Point(-73.9857, 40.7484)");
    }

    #[test]
    fn an_embedding_shows_two_components() {
        assert_eq!(embedding_label(&[0.021, -0.104, 0.5]), "[0.021, -0.104, …]");
    }

    #[test]
    fn a_long_preview_is_cut_and_binary_is_named() {
        let long = Value::String("x".repeat(80));
        assert!(preview_label(&long).ends_with('…'));
        assert_eq!(preview_label(&Value::Bytes(vec![1, 2])), "(binary)");
        assert_eq!(preview_label(&Value::Bool(true)), "true");
    }

    #[tokio::test]
    async fn cells_read_in_the_users_time_zone() {
        let svc = MockConnectionService::ready();
        let schema = svc.collection_schema("users").await.expect("schema");
        let record = svc.record("users", "u44182").await.expect("record");
        let col = schema
            .columns
            .iter()
            .find(|c| c.header == "updated_at")
            .expect("column");
        assert_eq!(cell_text(col, &record, &schema, 0), "2026-06-13 12:41:02");
        assert_eq!(cell_text(col, &record, &schema, 480), "2026-06-13 20:41:02");
    }

    #[tokio::test]
    async fn the_srid_column_reads_the_geometry_type() {
        let svc = MockConnectionService::ready();
        let schema = svc.collection_schema("places").await.expect("schema");
        let record = svc.record("places", "place104").await.expect("record");
        let col = schema
            .columns
            .iter()
            .find(|c| c.header == "SRID")
            .expect("column");
        assert_eq!(cell_text(col, &record, &schema, 0), "EPSG:4326");
    }

    #[test]
    fn statuses_have_a_tone() {
        assert_eq!(status_tone("shipped"), "green");
        assert_eq!(status_tone("cancelled"), "red");
        assert_eq!(status_tone("whatever"), "gray");
    }
}
