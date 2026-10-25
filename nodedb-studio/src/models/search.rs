//! Search: collection-backed retrieval over the records a connection holds.
//!
//! A full-text index and a vector index are attributes of fields of one
//! collection, never a collection of their own. A search names the
//! collection, the retrieval mode, the fields, the query inputs, and the
//! filters, and answers with ranked records. A ranked result is a view of
//! the source records: its rank and score belong to one execution and never
//! to the record.
//!
//! Keyword ranks by BM25 (larger is better). Semantic ranks by the vector
//! index's distance (smaller is closer). Hybrid ranks by a fusion of the two
//! ranked lists, so its score is rank-based and not comparable to either raw
//! score.

use crate::models::records::Filter;
use crate::models::vector::Metric;

/// How a search retrieves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    Keyword,
    Semantic,
    #[default]
    Hybrid,
}

impl SearchMode {
    pub const ALL: [SearchMode; 3] = [
        SearchMode::Keyword,
        SearchMode::Semantic,
        SearchMode::Hybrid,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SearchMode::Keyword => "Keyword",
            SearchMode::Semantic => "Semantic",
            SearchMode::Hybrid => "Hybrid",
        }
    }

    /// Whether the mode ranks by the text index.
    pub fn uses_text(self) -> bool {
        match self {
            SearchMode::Keyword | SearchMode::Hybrid => true,
            SearchMode::Semantic => false,
        }
    }

    /// Whether the mode ranks by the vector index.
    pub fn uses_vector(self) -> bool {
        match self {
            SearchMode::Semantic | SearchMode::Hybrid => true,
            SearchMode::Keyword => false,
        }
    }
}

/// The vector index a collection's search can use.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchVector {
    pub field: String,
    pub index: String,
    pub dimensions: u32,
    pub metric: Metric,
}

/// A full-text index a collection's search can use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchText {
    pub index: String,
    /// The indexed fields, in index order.
    pub fields: Vec<String>,
    pub analyzer: String,
    pub binding_revision: u32,
}

/// What Search needs to know about one collection.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchCollection {
    pub name: String,
    /// The field that holds a record's id.
    pub id_field: String,
    pub text: Option<SearchText>,
    pub vector: Option<SearchVector>,
    /// Fields a filter can name. A filter applies to every retrieval leg
    /// before the result is cut.
    pub filter_fields: Vec<String>,
}

impl SearchCollection {
    /// The indexed text fields, or none.
    pub fn text_fields(&self) -> &[String] {
        self.text.as_ref().map_or(&[], |t| t.fields.as_slice())
    }

    /// Which modes this collection can answer.
    pub fn supports(&self, mode: SearchMode) -> bool {
        (!mode.uses_text() || self.text.is_some()) && (!mode.uses_vector() || self.vector.is_some())
    }
}

/// What a connection offers Search.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchSetup {
    pub collections: Vec<SearchCollection>,
    /// The embedding provider, when one is configured. Without it a
    /// semantic search starts from a stored record or explicit values, and
    /// text is never embedded.
    pub provider: Option<String>,
}

impl SearchSetup {
    pub fn collection(&self, name: &str) -> Option<&SearchCollection> {
        self.collections.iter().find(|c| c.name == name)
    }
}

impl crate::services::async_state::IsEmpty for SearchSetup {
    fn is_empty(&self) -> bool {
        self.collections.is_empty()
    }
}

/// Where the semantic query vector comes from.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum VectorInput {
    /// Nothing chosen yet.
    #[default]
    None,
    /// Explicit values, attached or pasted.
    Values(Vec<f32>),
    /// The stored embedding of a record of the collection.
    Record(String),
}

impl VectorInput {
    /// The value in the strip: `Attached · 768D`.
    pub fn label(&self) -> String {
        match self {
            VectorInput::None => "None".to_string(),
            VectorInput::Values(v) => format!("Attached · {}D", v.len()),
            VectorInput::Record(id) => format!("Record {id}"),
        }
    }
}

/// How the two ranked lists combine. Reciprocal rank fusion: each list adds
/// `weight / (k + rank)` for a record it holds, with a one-based rank. The
/// smoothing constant, the source weights, and the candidate budget are
/// three separate things.
#[derive(Debug, Clone, PartialEq)]
pub struct Fusion {
    /// The smoothing constant.
    pub k: u32,
    pub keyword_weight: f64,
    pub vector_weight: f64,
    /// How many records each leg ranks before fusion. Not the result limit.
    pub candidates: u32,
}

pub const DEFAULT_K: u32 = 60;
pub const DEFAULT_CANDIDATES: u32 = 100;
pub const MAX_CANDIDATES: u32 = 500;
pub const MAX_K: u32 = 1_000;
pub const MAX_WEIGHT: f64 = 100.0;
pub const DEFAULT_LIMIT: u32 = 20;
pub const MAX_LIMIT: u32 = 100;

impl Default for Fusion {
    fn default() -> Self {
        Self {
            k: DEFAULT_K,
            keyword_weight: 1.0,
            vector_weight: 1.0,
            candidates: DEFAULT_CANDIDATES,
        }
    }
}

impl Fusion {
    /// `1 / 1` or `1 / 2`.
    pub fn weights_label(&self) -> String {
        format!("{} / {}", self.keyword_weight, self.vector_weight)
    }
}

/// One search, every part explicit.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRequest {
    pub collection: String,
    pub mode: SearchMode,
    /// The keyword query. Empty for a semantic search.
    pub query: String,
    pub text_field: Option<String>,
    pub vector_field: Option<String>,
    pub vector: VectorInput,
    /// Filters apply to every leg before the cut.
    pub filters: Vec<Filter>,
    pub limit: u32,
    pub fusion: Fusion,
}

/// One leg's place for a record: its rank in that leg and the raw value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leg {
    /// One-based.
    pub rank: u32,
    /// BM25 for the keyword leg, the distance for the vector leg.
    pub raw: f32,
    /// What the leg added to the fused score. `None` outside a fusion.
    pub contribution: Option<f64>,
}

/// The score a result is ranked by, with its meaning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Score {
    /// Larger ranks first. Not a probability.
    Bm25(f32),
    /// In the index metric. Smaller ranks first.
    Distance(f32),
    /// The fused score. Rank-based.
    Fused(f64),
}

impl Score {
    /// The caption over the number: `RRF score`.
    pub fn caption(self) -> &'static str {
        match self {
            Score::Bm25(_) => "BM25 score",
            Score::Distance(_) => "Distance",
            Score::Fused(_) => "RRF score",
        }
    }

    /// The number as the list shows it.
    pub fn text(self) -> String {
        match self {
            Score::Bm25(s) => format!("{s:.3}"),
            Score::Distance(d) => format!("{d:.3}"),
            Score::Fused(f) => format!("{f:.6}"),
        }
    }
}

/// A passage of a record with the spans that matched, as byte ranges into
/// `text` on character boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Excerpt {
    pub text: String,
    pub spans: Vec<(usize, usize)>,
}

impl Excerpt {
    /// The text cut into `(piece, matched)` runs, for display. A span the
    /// text cannot hold is dropped, never panics.
    pub fn runs(&self) -> Vec<(&str, bool)> {
        let mut out = Vec::new();
        let mut at = 0;
        for &(start, end) in &self.spans {
            if start < at || end <= start || end > self.text.len() {
                continue;
            }
            let (Some(before), Some(hit)) = (self.text.get(at..start), self.text.get(start..end))
            else {
                continue;
            };
            if !before.is_empty() {
                out.push((before, false));
            }
            out.push((hit, true));
            at = end;
        }
        if let Some(rest) = self.text.get(at..)
            && !rest.is_empty()
        {
            out.push((rest, false));
        }
        out
    }
}

/// One ranked record.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// One-based, in the result's order.
    pub rank: u32,
    /// The source record's id, never an internal index id.
    pub id: String,
    pub title: String,
    pub excerpt: Excerpt,
    pub body: String,
    pub language: Option<String>,
    pub score: Score,
    /// Set when the keyword leg holds the record.
    pub keyword: Option<Leg>,
    /// Set when the vector leg holds the record.
    pub vector: Option<Leg>,
}

/// The statement that equals a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSql {
    /// The statement with the query vector as `$query_vec`.
    pub display: String,
    /// The statement with the vector written in, ready to run.
    pub executable: String,
    /// What the statement cannot express of the displayed ranking.
    pub caveat: Option<String>,
}

/// The ranked records of one request. `request` is the request that
/// produced them, so a result never shows under a query it did not answer.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub request: SearchRequest,
    /// Binds the ranks, excerpts, and statement to one run.
    pub execution: u32,
    pub hits: Vec<Hit>,
    /// Records the keyword leg ranked before the cut. `None` without one.
    pub keyword_candidates: Option<u32>,
    pub vector_candidates: Option<u32>,
    /// The metric the vector leg ranked by.
    pub metric: Option<Metric>,
    /// The query vector's size, when one ran.
    pub vector_dimensions: Option<u32>,
    pub sql: SearchSql,
    /// Seconds since the epoch, UTC.
    pub observed_at: i64,
}

impl crate::services::async_state::IsEmpty for SearchResult {
    fn is_empty(&self) -> bool {
        false
    }
}

impl SearchResult {
    pub fn hit(&self, id: &str) -> Option<&Hit> {
        self.hits.iter().find(|h| h.id == id)
    }

    /// The line over the ranked results: what ranks them and in which order.
    pub fn ranking_note(&self) -> String {
        match self.request.mode {
            SearchMode::Keyword => "Ranked by BM25 · larger is better".to_string(),
            SearchMode::Semantic => self.metric.map_or_else(
                || "Ranked by vector distance".to_string(),
                |m| m.distance_note().to_string(),
            ),
            SearchMode::Hybrid => format!(
                "Ranked by reciprocal rank fusion · k = {} · weights {}",
                self.request.fusion.k,
                self.request.fusion.weights_label()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_excerpt_cuts_into_matched_runs() {
        let e = Excerpt {
            text: "A distributed database scales.".into(),
            spans: vec![(2, 22)],
        };
        assert_eq!(
            e.runs(),
            vec![
                ("A ", false),
                ("distributed database", true),
                (" scales.", false)
            ]
        );
    }

    #[test]
    fn a_span_off_a_character_boundary_is_dropped() {
        let e = Excerpt {
            text: "naïve search".into(),
            // Inside the two-byte `ï`.
            spans: vec![(3, 5), (7, 13)],
        };
        assert_eq!(e.runs(), vec![("naïve ", false), ("search", true)]);
    }

    #[test]
    fn overlapping_and_out_of_range_spans_are_ignored() {
        let e = Excerpt {
            text: "abcdef".into(),
            spans: vec![(0, 3), (2, 4), (4, 99)],
        };
        assert_eq!(e.runs(), vec![("abc", true), ("def", false)]);
    }

    #[test]
    fn modes_need_their_indexes() {
        let c = SearchCollection {
            name: "notes".into(),
            id_field: "_id".into(),
            text: None,
            vector: None,
            filter_fields: Vec::new(),
        };
        assert!(SearchMode::ALL.iter().all(|m| !c.supports(*m)));
    }

    #[test]
    fn scores_name_their_meaning() {
        assert_eq!(Score::Fused(0.032_522).text(), "0.032522");
        assert_eq!(Score::Fused(0.0).caption(), "RRF score");
        assert_eq!(Score::Distance(0.084).caption(), "Distance");
        assert_eq!(Score::Bm25(7.4).caption(), "BM25 score");
    }
}
