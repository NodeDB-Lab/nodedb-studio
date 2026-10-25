//! The collections Timeseries offers, read from the Explorer's catalog.

use crate::data::mock::explorer::{collection_schema, collections};
use crate::models::collection::Profile;
use crate::models::schema::FieldType;
use crate::models::timeseries::{FieldInfo, MoneyField, TimeField, TimeSource, Unsupported};

/// Declared money fields: collection, amount field, currency field, and the
/// currency a fresh query pins. A unit is never read off a field name.
const MONEY: &[(&str, &str, &str, &str)] = &[
    ("sales", "revenue", "currency", "USD"),
    ("orders", "total", "currency", "USD"),
];

/// Every catalog collection, with whether it can be explored over time.
pub fn time_sources() -> Vec<TimeSource> {
    collections()
        .into_iter()
        .map(|c| {
            let schema = collection_schema(&c.name);
            let (time_fields, fields) = match &schema {
                Some(s) => (
                    s.fields
                        .iter()
                        .filter(|f| matches!(f.ty, FieldType::Timestamp | FieldType::Datetime))
                        .map(|f| TimeField {
                            name: f.name.clone(),
                            time_key: f.time_key,
                        })
                        .collect::<Vec<_>>(),
                    s.fields
                        .iter()
                        .map(|f| FieldInfo {
                            name: f.name.clone(),
                            numeric: f.ty.numeric(),
                            type_label: f.ty.label(),
                        })
                        .collect::<Vec<_>>(),
                ),
                None => (Vec::new(), Vec::new()),
            };
            let unsupported = match (c.profile, schema.is_some(), time_fields.is_empty()) {
                (Profile::KeyValue, _, _) => Some(Unsupported::KeyValue),
                (_, false, _) => Some(Unsupported::NoRecords),
                (_, true, true) => Some(Unsupported::NoTimestamp),
                (_, true, false) => None,
            };
            let money = MONEY
                .iter()
                .filter(|(coll, m, cur, _)| {
                    *coll == c.name
                        && fields.iter().any(|f| f.name == *m)
                        && fields.iter().any(|f| f.name == *cur)
                })
                .map(|(_, m, cur, default)| MoneyField {
                    field: m.to_string(),
                    currency_field: cur.to_string(),
                    currency: default.to_string(),
                })
                .collect();
            TimeSource {
                collection: c.name,
                profile: c.profile,
                time_fields,
                fields,
                money,
                unsupported,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(name: &str) -> TimeSource {
        time_sources()
            .into_iter()
            .find(|s| s.collection == name)
            .expect("source")
    }

    #[test]
    fn sales_is_a_timeseries_source_keyed_by_sold_at() {
        let s = source("sales");
        assert!(s.eligible());
        let t = s.default_time_field().expect("time field");
        assert_eq!((t.name.as_str(), t.time_key), ("sold_at", true));
        assert_eq!(s.currency_field("revenue"), Some("currency"));
    }

    #[test]
    fn a_collection_with_timestamps_but_no_time_key_is_eligible_on_an_explicit_field() {
        let s = source("shipments");
        assert!(s.eligible());
        assert!(s.time_fields.iter().all(|t| !t.time_key));
        assert!(s.time_fields.len() >= 2);
    }

    #[test]
    fn unsupported_collections_say_why() {
        assert_eq!(source("sessions").unsupported, Some(Unsupported::KeyValue));
        assert_eq!(
            source("climate_cube").unsupported,
            Some(Unsupported::NoRecords)
        );
        for s in time_sources() {
            assert_eq!(s.eligible(), s.unsupported.is_none());
        }
    }

    #[test]
    fn every_catalog_collection_is_listed_once() {
        let names: Vec<String> = time_sources().into_iter().map(|s| s.collection).collect();
        let mut dedup = names.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(names.len(), dedup.len());
        assert_eq!(names.len(), collections().len());
    }
}
