//! Records, and the queries and pages the Explorer reads them through.
//!
//! A record's values are `nodedb_types::Value`, as the server returns them.
//! Studio keeps field order from the schema, never from the value: an
//! object's own key order is not stable.

use std::collections::HashMap;

use nodedb_types::Value;
use serde::{Deserialize, Serialize};

/// One stored record.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// The key the server addresses the record by.
    pub id: String,
    /// The version the server holds. A write names the revision it read.
    pub revision: u64,
    /// A `Value::Object` of the record's fields.
    pub value: Value,
}

impl Record {
    pub fn new(id: &str, revision: u64, fields: Vec<(&str, Value)>) -> Self {
        let map: HashMap<String, Value> = fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        Self {
            id: id.to_string(),
            revision,
            value: Value::Object(map),
        }
    }

    pub fn get(&self, field: &str) -> Option<&Value> {
        match &self.value {
            Value::Object(map) => map.get(field),
            _ => None,
        }
    }
}

/// `field = value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    pub field: String,
    pub value: String,
}

impl Filter {
    /// The chip text, `status = active`.
    pub fn label(&self) -> String {
        format!("{} = {}", self.field, self.value)
    }
}

/// How far back a timeseries view reads, from the observation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimeRange {
    Last24Hours,
    #[default]
    Last7Days,
    Last30Days,
}

impl TimeRange {
    pub const ALL: [TimeRange; 3] = [
        TimeRange::Last24Hours,
        TimeRange::Last7Days,
        TimeRange::Last30Days,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TimeRange::Last24Hours => "Last 24 hours",
            TimeRange::Last7Days => "Last 7 days",
            TimeRange::Last30Days => "Last 30 days",
        }
    }

    pub fn seconds(self) -> i64 {
        match self {
            TimeRange::Last24Hours => 86_400,
            TimeRange::Last7Days => 7 * 86_400,
            TimeRange::Last30Days => 30 * 86_400,
        }
    }
}

/// A longitude/latitude box in the geometry field's CRS.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bbox {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BboxError {
    #[error("Enter four numbers: min lon, min lat, max lon, max lat.")]
    Shape,
    #[error("Longitude runs from -180 to 180 and latitude from -90 to 90.")]
    Range,
    #[error("The minimum must not exceed the maximum.")]
    Order,
}

impl Bbox {
    /// Parses `min_lon, min_lat, max_lon, max_lat`.
    pub fn parse(text: &str) -> Result<Self, BboxError> {
        let parts: Vec<f64> = text
            .split(',')
            .map(|p| p.trim().parse::<f64>().map_err(|_| BboxError::Shape))
            .collect::<Result<_, _>>()?;
        let [min_lon, min_lat, max_lon, max_lat] = parts[..] else {
            return Err(BboxError::Shape);
        };
        if min_lon.abs() > 180.0 || max_lon.abs() > 180.0 {
            return Err(BboxError::Range);
        }
        if min_lat.abs() > 90.0 || max_lat.abs() > 90.0 {
            return Err(BboxError::Range);
        }
        if min_lon > max_lon || min_lat > max_lat {
            return Err(BboxError::Order);
        }
        Ok(Self {
            min_lon,
            min_lat,
            max_lon,
            max_lat,
        })
    }

    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        (self.min_lon..=self.max_lon).contains(&lon) && (self.min_lat..=self.max_lat).contains(&lat)
    }
}

/// What one read of a collection asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordQuery {
    pub filter: Option<Filter>,
    /// Matches any text field, ignoring case.
    pub text: String,
    /// A field and whether it sorts descending.
    pub sort: Option<(String, bool)>,
    /// Zero-based.
    pub page: usize,
    pub page_size: usize,
    pub range: TimeRange,
    pub bbox: Option<Bbox>,
}

impl RecordQuery {
    pub fn new(page_size: usize) -> Self {
        Self {
            filter: None,
            text: String::new(),
            sort: None,
            page: 0,
            page_size,
            range: TimeRange::default(),
            bbox: None,
        }
    }

    /// Whether any narrowing applies, which changes how an empty page reads.
    pub fn narrowed(&self) -> bool {
        self.filter.is_some() || !self.text.trim().is_empty() || self.bbox.is_some()
    }
}

/// One page of matching records.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordPage {
    pub rows: Vec<Record>,
    /// Records that match the query, across every page.
    pub total: u64,
    /// When the server answered, seconds since the epoch.
    pub observed_at: i64,
}

impl RecordPage {
    pub fn pages(&self, page_size: usize) -> usize {
        let size = page_size.max(1) as u64;
        usize::try_from(self.total.div_ceil(size)).unwrap_or(usize::MAX)
    }
}

/// Why the server refuses an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denial {
    /// The session's role lacks the privilege.
    Privilege,
    /// The collection never allows it, whoever asks.
    Policy(String),
}

/// Whether the session may do one thing to a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grant {
    Allowed,
    Denied(Denial),
}

impl Grant {
    pub fn allowed(&self) -> bool {
        matches!(self, Grant::Allowed)
    }

    /// The sentence that explains a refusal, `INSERT permission required`.
    pub fn headline(&self, privilege: &str) -> Option<String> {
        match self {
            Grant::Allowed => None,
            Grant::Denied(Denial::Privilege) => Some(format!("{privilege} permission required")),
            Grant::Denied(Denial::Policy(why)) => Some(why.clone()),
        }
    }

    /// The line under the headline.
    pub fn detail(&self, privilege: &str, to: &str) -> Option<String> {
        match self {
            Grant::Allowed => None,
            Grant::Denied(Denial::Privilege) => Some(format!(
                "You need {privilege} permission on this collection to {to}."
            )),
            Grant::Denied(Denial::Policy(_)) => None,
        }
    }
}

/// The session's rights over one collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionAccess {
    pub role: String,
    pub create: Grant,
    pub read: Grant,
    pub update: Grant,
    pub delete: Grant,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_parses_four_ordered_numbers() {
        let b = Bbox::parse("-74.0, 40.7, -73.9, 40.8").expect("valid");
        assert!(b.contains(-73.9857, 40.7484));
        assert!(!b.contains(-73.5, 40.7484));
    }

    #[test]
    fn bbox_rejects_each_malformed_shape() {
        assert_eq!(Bbox::parse("1,2,3"), Err(BboxError::Shape));
        assert_eq!(Bbox::parse("a,b,c,d"), Err(BboxError::Shape));
        assert_eq!(Bbox::parse("-200,0,0,10"), Err(BboxError::Range));
        assert_eq!(Bbox::parse("10,0,0,10"), Err(BboxError::Order));
    }

    #[test]
    fn pages_round_up() {
        let page = RecordPage {
            rows: Vec::new(),
            total: 1284,
            observed_at: 0,
        };
        assert_eq!(page.pages(5), 257);
        assert_eq!(page.pages(50), 26);
    }

    #[test]
    fn a_text_or_filter_narrows_the_query() {
        let mut q = RecordQuery::new(10);
        assert!(!q.narrowed());
        q.text = " x ".into();
        assert!(q.narrowed());
    }
}
