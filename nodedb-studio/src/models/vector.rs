//! Vector Space data: the vector indexes a connection holds, the records
//! they cover, and the nearest-neighbor search over them.
//!
//! A vector index is an attribute of one field of one collection. It is not
//! a storage engine, so every type here names the collection and the field
//! it belongs to. Distances come from the index's metric and rank ascending:
//! a smaller distance is closer. A distance is never a probability.

use crate::models::records::Filter;

/// How an index measures the distance between two vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Cosine,
    L2,
    InnerProduct,
}

impl Metric {
    /// The metric from the server's name for it.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "cosine" => Some(Metric::Cosine),
            "l2" | "euclidean" => Some(Metric::L2),
            "inner_product" | "ip" | "dot" => Some(Metric::InnerProduct),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Metric::Cosine => "cosine",
            Metric::L2 => "L2",
            Metric::InnerProduct => "inner product",
        }
    }

    /// The line over the ranked results. It names the metric and the order.
    pub fn distance_note(self) -> &'static str {
        match self {
            Metric::Cosine => "Cosine distance · lower is closer",
            Metric::L2 => "L2 distance · lower is closer",
            Metric::InnerProduct => "Inner-product distance · lower is closer",
        }
    }
}

/// One vector index and what Vector Space needs to know about its field.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorTarget {
    pub collection: String,
    pub field: String,
    pub index: String,
    /// The collection's key field, e.g. `_id`.
    pub id_field: String,
    pub dimensions: u32,
    pub metric: Metric,
    /// The model that wrote the embeddings, when the catalog records one.
    pub model: Option<String>,
    /// Vectors the index holds, live. Bounds a projection sample.
    pub population: u64,
    /// Changes whenever the indexed vectors change. A result is tied to it.
    pub revision: u64,
    /// Fields a search can filter on before the top-k cut.
    pub filter_fields: Vec<String>,
    /// Fields a projection can color by.
    pub color_fields: Vec<String>,
}

impl VectorTarget {
    /// `articles.embedding`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.collection, self.field)
    }
}

/// What a connection offers Vector Space.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorSetup {
    pub targets: Vec<VectorTarget>,
    /// The embedding provider, when one is configured. Without it a search
    /// starts from a record or a pasted vector, never from text.
    pub provider: Option<String>,
}

impl VectorSetup {
    pub fn target(&self, collection: &str, field: &str) -> Option<&VectorTarget> {
        self.targets
            .iter()
            .find(|t| t.collection == collection && t.field == field)
    }

    /// The collections that carry a vector index, once each.
    pub fn collections(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for t in &self.targets {
            if !out.contains(&t.collection.as_str()) {
                out.push(&t.collection);
            }
        }
        out
    }

    /// The vector fields of one collection.
    pub fn fields(&self, collection: &str) -> Vec<&str> {
        self.targets
            .iter()
            .filter(|t| t.collection == collection)
            .map(|t| t.field.as_str())
            .collect()
    }
}

impl crate::services::async_state::IsEmpty for VectorSetup {
    fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

/// A stored record with its embedding, read-only.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorRecord {
    pub collection: String,
    pub id: String,
    pub title: String,
    pub body: String,
    pub language: Option<String>,
    pub model: Option<String>,
    /// Seconds since the Unix epoch.
    pub embedded_at: Option<i64>,
    pub vector: Vec<f32>,
}

impl VectorRecord {
    /// The first values, then an ellipsis: `[0.021, -0.104, 0.008, …]`.
    pub fn preview(&self) -> String {
        let head: Vec<String> = self
            .vector
            .iter()
            .take(3)
            .map(|v| format!("{v:.3}"))
            .collect();
        if self.vector.len() > 3 {
            format!("[{}, …]", head.join(", "))
        } else {
            format!("[{}]", head.join(", "))
        }
    }

    /// Every value, for the expanded view.
    pub fn all_values(&self) -> String {
        let all: Vec<String> = self.vector.iter().map(|v| format!("{v:.4}")).collect();
        format!("[{}]", all.join(", "))
    }

    /// The Euclidean length of the stored vector.
    pub fn norm(&self) -> f32 {
        self.vector.iter().map(|v| v * v).sum::<f32>().sqrt()
    }
}

impl crate::services::async_state::IsEmpty for VectorRecord {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A selection that may hold nothing reads as a loaded value either way.
impl crate::services::async_state::IsEmpty for Option<VectorRecord> {
    fn is_empty(&self) -> bool {
        false
    }
}

/// Where a query vector comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum QuerySource {
    /// The stored embedding of this record, in the target field.
    Record(String),
    /// A vector the user supplied.
    Vector(Vec<f32>),
}

/// The most neighbors one search returns, and the widest search beam.
pub const MAX_TOP_K: u32 = 100;
pub const MAX_EF_SEARCH: u32 = 1_000;
pub const DEFAULT_TOP_K: u32 = 10;

/// One search, every part explicit.
#[derive(Debug, Clone, PartialEq)]
pub struct NeighborRequest {
    pub collection: String,
    pub field: String,
    pub source: QuerySource,
    pub top_k: u32,
    /// Drop the source record from the candidates before ranking.
    pub exclude_source: bool,
    /// Candidate filters. They apply before the top-k cut.
    pub filters: Vec<Filter>,
    /// The HNSW beam width, when the form sets one.
    pub ef_search: Option<u32>,
}

/// One ranked neighbor.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbor {
    /// One-based.
    pub rank: u32,
    pub id: String,
    /// In the index metric. Ascending down the list.
    pub distance: f32,
    pub title: String,
    pub excerpt: String,
}

/// The neighbors of one request. `request` is the request that produced
/// them, so a result never shows under a query it did not answer.
#[derive(Debug, Clone, PartialEq)]
pub struct NeighborResult {
    pub request: NeighborRequest,
    pub neighbors: Vec<Neighbor>,
    /// The index revision the search ran against.
    pub revision: u64,
}

impl crate::services::async_state::IsEmpty for NeighborResult {
    fn is_empty(&self) -> bool {
        false
    }
}

/// Which control an issue belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueField {
    RecordId,
    Vector,
    TopK,
    EfSearch,
    Filter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub field: IssueField,
    pub message: String,
}

impl Issue {
    fn new(field: IssueField, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

/// How a search is started. Text is not here: it needs a configured
/// embedding provider, and the form offers it only when one exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Record,
    Vector,
}

impl SourceKind {
    pub const ALL: [SourceKind; 2] = [SourceKind::Record, SourceKind::Vector];

    pub fn label(self) -> &'static str {
        match self {
            SourceKind::Record => "Record",
            SourceKind::Vector => "Vector",
        }
    }
}

/// The search form as typed. Nothing here is checked until `check`.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchForm {
    pub source: SourceKind,
    pub record_id: String,
    pub vector_text: String,
    pub top_k: String,
    pub exclude_source: bool,
    pub filters: Vec<Filter>,
    pub ef_search: String,
}

impl Default for SearchForm {
    fn default() -> Self {
        Self {
            source: SourceKind::Record,
            record_id: String::new(),
            vector_text: String::new(),
            top_k: DEFAULT_TOP_K.to_string(),
            exclude_source: true,
            filters: Vec::new(),
            ef_search: String::new(),
        }
    }
}

/// Numbers from `[0.1, -0.2, 0.3]`, or the same without brackets, separated
/// by commas or spaces. The error names the first value that is not a
/// finite number.
pub fn parse_vector(text: &str) -> Result<Vec<f32>, String> {
    let inner = text.trim().trim_start_matches('[').trim_end_matches(']');
    let mut out = Vec::new();
    for (i, token) in inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .enumerate()
    {
        match token.parse::<f32>() {
            Ok(v) if v.is_finite() => out.push(v),
            Ok(_) => {
                return Err(format!(
                    "Value {} (`{token}`) is not a finite number.",
                    i + 1
                ));
            }
            Err(_) => return Err(format!("Value {} (`{token}`) is not a number.", i + 1)),
        }
    }
    Ok(out)
}

impl SearchForm {
    /// The request this form asks, or every reason it cannot run yet.
    pub fn check(&self, target: &VectorTarget) -> Result<NeighborRequest, Vec<Issue>> {
        let mut issues = Vec::new();

        let source = match self.source {
            SourceKind::Record => {
                let id = self.record_id.trim();
                if id.is_empty() {
                    issues.push(Issue::new(IssueField::RecordId, "Enter a record ID."));
                }
                Some(QuerySource::Record(id.to_string()))
            }
            SourceKind::Vector => match parse_vector(&self.vector_text) {
                Err(message) => {
                    issues.push(Issue::new(IssueField::Vector, message));
                    None
                }
                Ok(v) if v.is_empty() => {
                    issues.push(Issue::new(IssueField::Vector, "Paste the query vector."));
                    None
                }
                Ok(v) if v.len() != target.dimensions as usize => {
                    issues.push(Issue::new(
                        IssueField::Vector,
                        format!(
                            "The vector has {} values. {} holds {}.",
                            v.len(),
                            target.label(),
                            target.dimensions
                        ),
                    ));
                    None
                }
                Ok(v) => Some(QuerySource::Vector(v)),
            },
        };

        let top_k = match self.top_k.trim().parse::<u32>() {
            Ok(k) if (1..=MAX_TOP_K).contains(&k) => k,
            _ => {
                issues.push(Issue::new(
                    IssueField::TopK,
                    format!("Top K runs from 1 to {MAX_TOP_K}."),
                ));
                0
            }
        };

        let ef_search = match self.ef_search.trim() {
            "" => None,
            text => match text.parse::<u32>() {
                Ok(e) if (1..=MAX_EF_SEARCH).contains(&e) && e >= top_k => Some(e),
                Ok(e) if (1..=MAX_EF_SEARCH).contains(&e) => {
                    issues.push(Issue::new(
                        IssueField::EfSearch,
                        format!("Beam width {e} is below Top K {top_k}."),
                    ));
                    None
                }
                _ => {
                    issues.push(Issue::new(
                        IssueField::EfSearch,
                        format!("Beam width runs from 1 to {MAX_EF_SEARCH}."),
                    ));
                    None
                }
            },
        };

        for f in &self.filters {
            if !target.filter_fields.contains(&f.field) {
                issues.push(Issue::new(
                    IssueField::Filter,
                    format!("{} has no filter on `{}`.", target.collection, f.field),
                ));
            } else if f.value.trim().is_empty() {
                issues.push(Issue::new(
                    IssueField::Filter,
                    format!("The filter on `{}` needs a value.", f.field),
                ));
            }
        }

        match source {
            Some(source) if issues.is_empty() => Ok(NeighborRequest {
                collection: target.collection.clone(),
                field: target.field.clone(),
                source,
                top_k,
                exclude_source: self.exclude_source,
                filters: self
                    .filters
                    .iter()
                    .map(|f| Filter {
                        field: f.field.clone(),
                        value: f.value.trim().to_string(),
                    })
                    .collect(),
                ef_search,
            }),
            _ => Err(issues),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> VectorTarget {
        VectorTarget {
            collection: "articles".into(),
            field: "embedding".into(),
            index: "idx_articles_embedding".into(),
            id_field: "_id".into(),
            dimensions: 3,
            metric: Metric::Cosine,
            model: Some("support-embed-v2".into()),
            population: 100,
            revision: 1,
            filter_fields: vec!["language".into()],
            color_fields: vec!["language".into()],
        }
    }

    fn form(id: &str) -> SearchForm {
        SearchForm {
            record_id: id.into(),
            top_k: "2".into(),
            ..SearchForm::default()
        }
    }

    #[test]
    fn a_record_search_with_an_id_is_a_request() {
        let r = form("art104").check(&target()).expect("valid");
        assert_eq!(r.source, QuerySource::Record("art104".into()));
        assert_eq!((r.top_k, r.exclude_source), (2, true));
    }

    #[test]
    fn a_missing_record_id_names_the_control() {
        let issues = form("  ").check(&target()).expect_err("invalid");
        assert_eq!(issues[0].field, IssueField::RecordId);
    }

    #[test]
    fn top_k_outside_its_bounds_is_an_issue() {
        for bad in ["0", "101", "x", ""] {
            let f = SearchForm {
                top_k: bad.into(),
                ..form("art104")
            };
            let issues = f.check(&target()).expect_err("invalid");
            assert!(issues.iter().any(|i| i.field == IssueField::TopK), "{bad}");
        }
    }

    #[test]
    fn a_pasted_vector_must_match_the_dimension() {
        let f = SearchForm {
            source: SourceKind::Vector,
            vector_text: "[0.1, 0.2]".into(),
            ..form("")
        };
        let issues = f.check(&target()).expect_err("short");
        assert!(issues[0].message.contains("2 values"));
        let ok = SearchForm {
            vector_text: "[0.1, 0.2, 0.3]".into(),
            ..f
        };
        assert!(matches!(
            ok.check(&target()).expect("valid").source,
            QuerySource::Vector(v) if v.len() == 3
        ));
    }

    #[test]
    fn a_bad_value_is_named_by_position() {
        assert!(
            parse_vector("0.1 nope 0.3")
                .expect_err("bad")
                .contains("Value 2")
        );
        assert!(
            parse_vector("0.1, inf")
                .expect_err("inf")
                .contains("finite")
        );
        assert!(parse_vector("NaN").is_err());
        assert_eq!(parse_vector("1, 2 3").expect("parses"), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn a_filter_needs_a_known_field_and_a_value() {
        let unknown = SearchForm {
            filters: vec![Filter {
                field: "nope".into(),
                value: "x".into(),
            }],
            ..form("art104")
        };
        assert!(unknown.check(&target()).is_err());
        let empty = SearchForm {
            filters: vec![Filter {
                field: "language".into(),
                value: " ".into(),
            }],
            ..form("art104")
        };
        assert!(empty.check(&target()).is_err());
        let ok = SearchForm {
            filters: vec![Filter {
                field: "language".into(),
                value: " en ".into(),
            }],
            ..form("art104")
        };
        assert_eq!(ok.check(&target()).expect("valid").filters[0].value, "en");
    }

    #[test]
    fn the_beam_cannot_be_narrower_than_top_k() {
        let f = SearchForm {
            top_k: "10".into(),
            ef_search: "5".into(),
            ..form("art104")
        };
        assert!(f.check(&target()).is_err());
        let f = SearchForm {
            ef_search: "64".into(),
            ..f
        };
        assert_eq!(f.check(&target()).expect("valid").ef_search, Some(64));
    }

    #[test]
    fn metrics_decode_from_server_names() {
        assert_eq!(Metric::from_name("Cosine"), Some(Metric::Cosine));
        assert_eq!(Metric::from_name("l2"), Some(Metric::L2));
        assert_eq!(Metric::from_name("ip"), Some(Metric::InnerProduct));
        assert_eq!(Metric::from_name("hamming"), None);
        assert!(Metric::L2.distance_note().contains("lower is closer"));
        assert!(
            Metric::InnerProduct
                .distance_note()
                .contains("lower is closer")
        );
    }

    #[test]
    fn a_record_previews_three_values() {
        let r = VectorRecord {
            collection: "articles".into(),
            id: "art104".into(),
            title: String::new(),
            body: String::new(),
            language: None,
            model: None,
            embedded_at: None,
            vector: vec![-0.0891, 0.412, 0.022, 0.5],
        };
        assert_eq!(r.preview(), "[-0.089, 0.412, 0.022, …]");
    }
}
