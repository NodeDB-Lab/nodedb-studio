//! Runs a `RecordQuery` over a fixture dataset, with this session's writes
//! laid over it.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use nodedb_types::Value;

use crate::data::mock::explorer::analytics::{INVENTORY_SET, SALES_SET};
use crate::data::mock::explorer::build::{Dataset, NOW};
use crate::data::mock::explorer::docs::{ARTICLES_SET, ORDERS_SET, USERS_SET};
use crate::data::mock::explorer::places::{PLACES_SET, lon_lat};
use crate::data::mock::explorer::schemas::collection_schema;
use crate::data::mock::explorer::sessions::SESSIONS_SET;
use crate::data::mock::explorer::shipments::SHIPMENTS_SET;
use crate::models::records::{Filter, Record, RecordPage, RecordQuery};
use crate::models::schema::CollectionSchema;

/// Writes made this session, over the fixtures.
#[derive(Debug, Clone, Default)]
pub struct Overlay {
    pub replaced: BTreeMap<(String, String), Record>,
    pub deleted: BTreeSet<(String, String)>,
    /// Inserted records, newest first.
    pub inserted: BTreeMap<String, Vec<Record>>,
    pub edges_deleted: BTreeSet<String>,
}

/// The instant the fixtures were observed.
pub fn observed_at() -> i64 {
    NOW
}

fn dataset(collection: &str) -> Option<Dataset> {
    Some(match collection {
        "users" => USERS_SET,
        "orders" => ORDERS_SET,
        "sales" => SALES_SET,
        "places" => PLACES_SET,
        "articles" => ARTICLES_SET,
        "sessions" => SESSIONS_SET,
        "inventory" => INVENTORY_SET,
        "shipments" => SHIPMENTS_SET,
        _ => return None,
    })
}

/// Text for a value, as a filter compares it.
pub fn plain_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => s.clone(),
        Value::DateTime(t) | Value::NaiveDateTime(t) => crate::models::record_json::iso(t),
        other => crate::models::record_json::compact(other, &[]),
    }
}

/// Every record of `collection` in natural order, this session's writes
/// included. `None` for a collection without records.
pub fn all_records(collection: &str, overlay: &Overlay) -> Option<Vec<Record>> {
    natural(collection, overlay)
}

fn natural(collection: &str, overlay: &Overlay) -> Option<Vec<Record>> {
    let set = dataset(collection)?;
    let mut all: Vec<Record> = overlay
        .inserted
        .get(collection)
        .cloned()
        .unwrap_or_default();
    for i in 0..set.total {
        let row = (set.row)(i);
        let key = (collection.to_string(), row.id.clone());
        if overlay.deleted.contains(&key) {
            continue;
        }
        all.push(overlay.replaced.get(&key).cloned().unwrap_or(row));
    }
    Some(all)
}

fn matches_filter(record: &Record, filter: &Filter) -> bool {
    record
        .get(&filter.field)
        .is_some_and(|v| plain_text(v).eq_ignore_ascii_case(filter.value.trim()))
}

fn matches_text(record: &Record, needle: &str) -> bool {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    record.id.to_lowercase().contains(&needle)
        || match &record.value {
            Value::Object(map) => map.values().any(|v| match v {
                Value::String(s) => s.to_lowercase().contains(&needle),
                _ => false,
            }),
            _ => false,
        }
}

fn compare(a: Option<&Value>, b: Option<&Value>) -> Ordering {
    let key = |v: Option<&Value>| match v {
        Some(Value::Integer(i)) => (0, *i as f64, String::new()),
        Some(Value::Float(f)) => (0, *f, String::new()),
        Some(Value::DateTime(t)) => (0, t.unix_secs() as f64, String::new()),
        Some(Value::String(s)) => match s.parse::<f64>() {
            Ok(n)
                if s.chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit() || c == '-') =>
            {
                (0, n, String::new())
            }
            _ => (1, 0.0, s.to_lowercase()),
        },
        _ => (2, 0.0, String::new()),
    };
    let (ka, kb) = (key(a), key(b));
    ka.0.cmp(&kb.0)
        .then(ka.1.partial_cmp(&kb.1).unwrap_or(Ordering::Equal))
        .then(ka.2.cmp(&kb.2))
}

fn in_range(record: &Record, schema: &CollectionSchema, query: &RecordQuery) -> bool {
    let Some(key) = schema.time_key() else {
        return true;
    };
    match record.get(&key.name) {
        Some(Value::DateTime(t)) => {
            let age = NOW - t.unix_secs();
            (0..=query.range.seconds()).contains(&age)
        }
        _ => true,
    }
}

fn matching(
    collection: &str,
    query: &RecordQuery,
    overlay: &Overlay,
) -> Option<(CollectionSchema, Vec<Record>)> {
    let schema = collection_schema(collection)?;
    let mut rows: Vec<Record> = natural(collection, overlay)?
        .into_iter()
        .filter(|r| in_range(r, &schema, query))
        .filter(|r| query.filter.as_ref().is_none_or(|f| matches_filter(r, f)))
        .filter(|r| matches_text(r, &query.text))
        .filter(|r| {
            query
                .bbox
                .is_none_or(|b| lon_lat(r).is_some_and(|(lon, lat)| b.contains(lon, lat)))
        })
        .collect();
    if let Some((field, descending)) = &query.sort {
        rows.sort_by(|a, b| {
            let o = compare(a.get(field), b.get(field));
            if *descending { o.reverse() } else { o }
        });
    }
    Some((schema, rows))
}

/// One page of `collection` for `query`. `None` for a collection without
/// records.
pub fn record_page(collection: &str, query: &RecordQuery, overlay: &Overlay) -> Option<RecordPage> {
    let (_, rows) = matching(collection, query, overlay)?;
    let total = rows.len() as u64;
    let size = query.page_size.max(1);
    Some(RecordPage {
        rows: rows
            .into_iter()
            .skip(query.page * size)
            .take(size)
            .collect(),
        total,
        observed_at: NOW,
    })
}

/// The record `id` holds now, this session's writes included.
pub fn stored_record(collection: &str, id: &str, overlay: &Overlay) -> Option<Record> {
    let key = (collection.to_string(), id.to_string());
    if overlay.deleted.contains(&key) {
        return None;
    }
    if let Some(r) = overlay.replaced.get(&key) {
        return Some(r.clone());
    }
    if let Some(r) = overlay
        .inserted
        .get(collection)
        .and_then(|rows| rows.iter().find(|r| r.id == id))
    {
        return Some(r.clone());
    }
    let set = dataset(collection)?;
    (0..set.total).map(|i| (set.row)(i)).find(|r| r.id == id)
}

/// The first rows an export of `fields` would write, and its row count.
pub fn transfer_preview(
    collection: &str,
    filter: Option<&Filter>,
    fields: &[String],
    overlay: &Overlay,
) -> Option<(Vec<Vec<String>>, u64)> {
    let query = RecordQuery {
        filter: filter.cloned(),
        range: crate::models::records::TimeRange::Last30Days,
        ..RecordQuery::new(3)
    };
    let (_, rows) = matching(collection, &query, overlay)?;
    let total = rows.len() as u64;
    let first = rows
        .iter()
        .take(3)
        .map(|r| {
            fields
                .iter()
                .map(|f| r.get(f).map(plain_text).unwrap_or_default())
                .collect()
        })
        .collect();
    Some((first, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::records::{Bbox, TimeRange};
    use nodedb_types::geometry::Geometry;

    fn is_point(value: &Value) -> bool {
        matches!(value, Value::Geometry(Geometry::Point { .. }))
    }

    fn page(collection: &str, query: &RecordQuery) -> RecordPage {
        record_page(collection, query, &Overlay::default()).expect("collection")
    }

    fn filter(field: &str, value: &str) -> Option<Filter> {
        Some(Filter {
            field: field.into(),
            value: value.into(),
        })
    }

    #[test]
    fn users_filter_by_status_counts_842() {
        let q = RecordQuery {
            filter: filter("status", "active"),
            ..RecordQuery::new(5)
        };
        let p = page("users", &q);
        assert_eq!((p.total, p.rows.len()), (842, 5));
        assert_eq!(page("users", &RecordQuery::new(5)).total, 1284);
    }

    #[test]
    fn inventory_in_nyc_sorted_by_sku_starts_with_the_authored_rows() {
        let q = RecordQuery {
            filter: filter("warehouse", "nyc"),
            sort: Some(("sku".into(), false)),
            ..RecordQuery::new(10)
        };
        let p = page("inventory", &q);
        assert_eq!(p.total, 58);
        let skus: Vec<String> = p
            .rows
            .iter()
            .map(|r| plain_text(r.get("sku").unwrap_or(&Value::Null)))
            .collect();
        assert_eq!(skus[0], "skuA104");
        assert_eq!(skus[9], "skuC340");
    }

    #[test]
    fn sales_in_the_last_seven_days_count_all_12482() {
        let p = page("sales", &RecordQuery::new(10));
        assert_eq!(p.total, 12_482);
        let day = RecordQuery {
            range: TimeRange::Last24Hours,
            ..RecordQuery::new(10)
        };
        assert!(page("sales", &day).total < 12_482);
    }

    #[test]
    fn a_bbox_keeps_only_points_inside() {
        let q = RecordQuery {
            bbox: Bbox::parse("-74.0, 40.74, -73.98, 40.75").ok(),
            ..RecordQuery::new(50)
        };
        let p = page("places", &q);
        assert!(
            p.total >= 1
                && p.rows
                    .iter()
                    .all(|r| is_point(r.get("geometry").unwrap_or(&Value::Null)))
        );
        assert!(p.rows.iter().any(|r| r.id == "place104"));
    }

    #[test]
    fn no_shipment_is_delayed() {
        let q = RecordQuery {
            filter: filter("status", "delayed"),
            ..RecordQuery::new(50)
        };
        assert_eq!(page("shipments", &q).total, 0);
        assert_eq!(page("shipments", &RecordQuery::new(50)).total, 312);
    }

    #[test]
    fn a_write_overlays_the_fixture() {
        let mut overlay = Overlay::default();
        let mut r = stored_record("users", "u44182", &overlay).expect("fixture");
        r.revision += 1;
        overlay
            .replaced
            .insert(("users".into(), "u44182".into()), r);
        assert_eq!(
            stored_record("users", "u44182", &overlay).map(|r| r.revision),
            Some(19)
        );
        overlay.deleted.insert(("users".into(), "u44182".into()));
        assert!(stored_record("users", "u44182", &overlay).is_none());
        let p = record_page("users", &RecordQuery::new(5), &overlay).expect("users");
        assert_eq!(p.total, 1283);
    }

    #[test]
    fn export_preview_reads_the_filtered_rows() {
        let (rows, total) = transfer_preview(
            "users",
            filter("status", "active").as_ref(),
            &["id".to_string(), "status".to_string()],
            &Overlay::default(),
        )
        .expect("users");
        assert_eq!(total, 842);
        assert_eq!(rows[0], ["u44182", "active"]);
    }
}
