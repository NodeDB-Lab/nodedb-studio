//! Search execution over the resident articles: the keyword leg ranks by
//! BM25, the vector leg by the exact cosine distance of Vector Space's
//! fixtures, and Hybrid fuses the two by rank.
//!
//! Filters, the candidate budget, and the limit are separate: a filter drops
//! records from both legs before either leg is cut, the budget sizes each leg,
//! and the limit cuts the final ranking.

use super::analyzer::{analyze, query_terms};
use super::bm25;
use super::fusion::{Fused, fuse};
use crate::data::mock::vector::{ArticleText, article_number, vector_record, vector_search};
use crate::models::format::epoch;
use crate::models::records::Filter;
use crate::models::search::{
    Excerpt, Hit, Leg, Score, SearchMode, SearchRequest, SearchResult, VectorInput,
};
use crate::models::search_sql::search_sql;
use crate::models::vector::{NeighborRequest, QuerySource};
use crate::services::error::StudioError;

/// The window an excerpt shows, in bytes of the record's text.
const WINDOW: usize = 150;
/// How far before the first match an excerpt starts.
const LEAD: usize = 40;
const ELLIPSIS: &str = "…";

fn rejected(reason: impl Into<String>) -> StudioError {
    StudioError::Rejected {
        reason: reason.into(),
    }
}

/// The instant the fixtures observe the index and the results at.
pub fn observed_at() -> i64 {
    epoch(2026, 6, 13, 13, 40, 6)
}

fn floor_boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// The part of `body` around the first match, with the matched spans.
/// Adjacent matched words, with only whitespace between, form one span.
pub fn excerpt(body: &str, terms: &[String]) -> Excerpt {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for t in analyze(body).tokens {
        if !terms.contains(&t.stem) {
            continue;
        }
        match spans.last_mut() {
            Some(last)
                if body
                    .get(last.1..t.start)
                    .is_some_and(|gap| gap.chars().all(char::is_whitespace)) =>
            {
                last.1 = t.end;
            }
            _ => spans.push((t.start, t.end)),
        }
    }
    let first = spans.first().map_or(0, |s| s.0);
    let mut start = 0;
    if first > LEAD + 20 {
        start = floor_boundary(body, first - LEAD);
        // Begin on a word.
        if let Some(space) = body[start..].find(' ') {
            start += space + 1;
        }
    }
    let mut end = floor_boundary(body, start + WINDOW);
    if end < body.len() {
        // End on a word, unless that would drop the first match.
        if let Some(space) = body[start..end].rfind(' ')
            && start + space > first
        {
            end = start + space;
        }
    }
    let lead = if start > 0 { ELLIPSIS } else { "" };
    let tail = if end < body.len() { ELLIPSIS } else { "" };
    let text = format!("{lead}{}{tail}", body[start..end].trim_end());
    let shown = text.len() - tail.len();
    let spans = spans
        .into_iter()
        .filter(|(s, e)| *s >= start && *e <= end)
        .map(|(s, e)| (s - start + lead.len(), e - start + lead.len()))
        .filter(|(_, e)| *e <= shown)
        .collect();
    Excerpt { text, spans }
}

fn keep(filters: &[Filter]) -> Result<impl Fn(&ArticleText) -> bool + use<>, StudioError> {
    for f in filters {
        if !matches!(f.field.as_str(), "language" | "model") {
            return Err(rejected(format!(
                "articles has no filter on `{}`.",
                f.field
            )));
        }
    }
    let filters = filters.to_vec();
    Ok(move |d: &ArticleText| {
        filters.iter().all(|f| match f.field.as_str() {
            "language" => d.language == Some(f.value.as_str()),
            "model" => d.model == f.value,
            _ => false,
        })
    })
}

/// The query vector: the values, or the stored embedding of a record.
pub fn query_vector(input: &VectorInput) -> Result<Vec<f32>, StudioError> {
    match input {
        VectorInput::None => Err(rejected(
            "A semantic search needs a query vector. Text is not embedded.",
        )),
        VectorInput::Values(v) => Ok(v.clone()),
        VectorInput::Record(id) => vector_record("articles", "embedding", id).map(|r| r.vector),
    }
}

/// One keyword or vector candidate: the record's number and its raw value.
type Candidates = Vec<(u64, f32)>;

fn keyword_leg(
    request: &SearchRequest,
    filters: &impl Fn(&ArticleText) -> bool,
    budget: usize,
) -> Result<Candidates, StudioError> {
    let terms = query_terms(&request.query);
    if terms.is_empty() {
        return Err(rejected(
            "The query has no searchable words after analysis. Every word is a stop word.",
        ));
    }
    let field = request.text_field.as_deref().unwrap_or("body");
    let found = bm25::search(field, &terms, filters, budget)
        .ok_or_else(|| rejected(format!("`{field}` is not an indexed text field.")))?;
    Ok(found
        .into_iter()
        .filter_map(|s| {
            let n = article_number(&bm25::documents()[s.slot].id)?;
            Some((n, s.score))
        })
        .collect())
}

fn vector_leg(
    request: &SearchRequest,
    vector: &[f32],
    budget: usize,
) -> Result<Candidates, StudioError> {
    let found = vector_search(&NeighborRequest {
        collection: request.collection.clone(),
        field: request
            .vector_field
            .clone()
            .unwrap_or_else(|| "embedding".into()),
        source: QuerySource::Vector(vector.to_vec()),
        top_k: budget as u32,
        exclude_source: false,
        filters: request.filters.clone(),
        ef_search: None,
    })?;
    Ok(found
        .neighbors
        .into_iter()
        .filter_map(|n| Some((article_number(&n.id)?, n.distance)))
        .collect())
}

fn hit(
    rank: u32,
    n: u64,
    terms: &[String],
    score: Score,
    keyword: Option<Leg>,
    vector: Option<Leg>,
) -> Hit {
    let doc = crate::data::mock::vector::article_text(n);
    Hit {
        rank,
        id: doc.id,
        title: doc.title,
        excerpt: excerpt(&doc.body, terms),
        body: doc.body,
        language: doc.language.map(str::to_string),
        score,
        keyword,
        vector,
    }
}

fn bare(rank: usize, raw: f32) -> Leg {
    Leg {
        rank: rank as u32 + 1,
        raw,
        contribution: None,
    }
}

pub fn search(request: &SearchRequest, execution: u32) -> Result<SearchResult, StudioError> {
    if request.collection != "articles" {
        return Err(StudioError::UnknownObject {
            kind: "collection",
            id: request.collection.clone(),
        });
    }
    let filters = keep(&request.filters)?;
    let limit = request.limit as usize;
    let candidates = request.fusion.candidates as usize;
    let terms = if request.mode.uses_text() {
        query_terms(&request.query)
    } else {
        Vec::new()
    };
    let vector = if request.mode.uses_vector() {
        Some(query_vector(&request.vector)?)
    } else {
        None
    };
    let (mut hits, keyword_candidates, vector_candidates) = match request.mode {
        SearchMode::Keyword => {
            let list = keyword_leg(request, &filters, limit)?;
            let hits: Vec<Hit> = list
                .iter()
                .enumerate()
                .map(|(i, (n, s))| {
                    hit(
                        i as u32 + 1,
                        *n,
                        &terms,
                        Score::Bm25(*s),
                        Some(bare(i, *s)),
                        None,
                    )
                })
                .collect();
            (hits, Some(list.len() as u32), None)
        }
        SearchMode::Semantic => {
            let query = vector.as_deref().unwrap_or(&[]);
            let list = vector_leg(request, query, limit)?;
            let hits = list
                .iter()
                .enumerate()
                .map(|(i, (n, d))| {
                    hit(
                        i as u32 + 1,
                        *n,
                        &terms,
                        Score::Distance(*d),
                        None,
                        Some(bare(i, *d)),
                    )
                })
                .collect();
            (hits, None, Some(list.len() as u32))
        }
        SearchMode::Hybrid => {
            let query = vector.as_deref().unwrap_or(&[]);
            let keyword = keyword_leg(request, &filters, candidates)?;
            let semantic = vector_leg(request, query, candidates)?;
            let fused = fuse(&keyword, &semantic, &request.fusion);
            let hits = fused
                .iter()
                .take(limit)
                .enumerate()
                .map(|(i, f): (usize, &Fused)| {
                    hit(
                        i as u32 + 1,
                        f.n,
                        &terms,
                        Score::Fused(f.score),
                        f.keyword,
                        f.vector,
                    )
                })
                .collect();
            (
                hits,
                Some(keyword.len() as u32),
                Some(semantic.len() as u32),
            )
        }
    };
    hits.truncate(limit);
    let sql = search_sql(request, "_id", vector.as_deref());
    Ok(SearchResult {
        request: request.clone(),
        execution,
        hits,
        keyword_candidates,
        vector_candidates,
        metric: request
            .mode
            .uses_vector()
            .then_some(crate::models::vector::Metric::Cosine),
        vector_dimensions: vector.as_ref().map(|v| v.len() as u32),
        sql,
        observed_at: observed_at(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::search::Fusion;

    fn request(mode: SearchMode) -> SearchRequest {
        SearchRequest {
            collection: "articles".into(),
            mode,
            query: "distributed database".into(),
            text_field: Some("body".into()),
            vector_field: Some("embedding".into()),
            vector: VectorInput::Record("art104".into()),
            filters: Vec::new(),
            limit: 5,
            fusion: Fusion::default(),
        }
    }

    #[test]
    fn keyword_ranks_by_bm25_with_marked_matches() {
        let r = search(&request(SearchMode::Keyword), 1).expect("searches");
        assert_eq!(r.hits.len(), 5);
        assert!(r.hits.windows(2).all(|w| match (w[0].score, w[1].score) {
            (Score::Bm25(a), Score::Bm25(b)) => a >= b,
            _ => false,
        }));
        let first = &r.hits[0];
        let runs = first.excerpt.runs();
        assert!(
            runs.iter()
                .any(|(t, hit)| *hit && t.to_lowercase().contains("distributed"))
        );
        assert!(first.vector.is_none() && first.keyword.is_some());
        assert_eq!(r.vector_candidates, None);
    }

    #[test]
    fn semantic_ranks_by_ascending_distance_with_the_record_first() {
        let r = search(&request(SearchMode::Semantic), 1).expect("searches");
        assert_eq!(r.hits[0].id, "art104");
        assert!(matches!(r.hits[0].score, Score::Distance(d) if d < 1e-4));
        assert!(r.hits.windows(2).all(|w| match (w[0].score, w[1].score) {
            (Score::Distance(a), Score::Distance(b)) => a <= b,
            _ => false,
        }));
        assert!(
            r.hits.iter().all(|h| h.excerpt.spans.is_empty()),
            "no words, no marks"
        );
    }

    #[test]
    fn hybrid_fuses_by_rank_and_keeps_each_legs_rank() {
        let r = search(&request(SearchMode::Hybrid), 1).expect("searches");
        assert!(r.hits.windows(2).all(|w| match (w[0].score, w[1].score) {
            (Score::Fused(a), Score::Fused(b)) => a >= b,
            _ => false,
        }));
        for h in &r.hits {
            let Score::Fused(score) = h.score else {
                panic!("hybrid scores are fused");
            };
            let sum: f64 = [h.keyword, h.vector]
                .into_iter()
                .flatten()
                .filter_map(|l| l.contribution)
                .sum();
            assert!((score - sum).abs() < 1e-12);
            assert!(h.keyword.is_some() || h.vector.is_some());
        }
        assert_eq!(r.keyword_candidates, Some(100));
        assert_eq!(r.vector_candidates, Some(100));
    }

    #[test]
    fn a_filter_applies_to_both_legs_before_the_cut() {
        let mut req = request(SearchMode::Hybrid);
        req.filters = vec![Filter {
            field: "language".into(),
            value: "ms".into(),
        }];
        let r = search(&req, 1).expect("searches");
        assert_eq!(r.hits.len(), 5);
        assert!(r.hits.iter().all(|h| h.language.as_deref() == Some("ms")));
    }

    #[test]
    fn the_candidate_budget_changes_the_fusion_and_not_the_limit() {
        let mut req = request(SearchMode::Hybrid);
        req.fusion.candidates = 5;
        let narrow = search(&req, 1).expect("searches");
        req.fusion.candidates = 100;
        let wide = search(&req, 1).expect("searches");
        assert_eq!(narrow.hits.len(), 5);
        assert_eq!(narrow.keyword_candidates, Some(5));
        let ids = |r: &SearchResult| r.hits.iter().map(|h| h.id.clone()).collect::<Vec<_>>();
        assert_ne!(ids(&narrow), ids(&wide), "depth changes an RRF ranking");
    }

    #[test]
    fn a_query_of_stop_words_is_refused_with_the_reason() {
        let mut req = request(SearchMode::Keyword);
        req.query = "the of a".into();
        let err = search(&req, 1).expect_err("nothing to search");
        assert!(err.to_string().contains("stop word"), "{err}");
    }

    #[test]
    fn a_missing_record_or_collection_is_named() {
        let mut req = request(SearchMode::Semantic);
        req.vector = VectorInput::Record("art0".into());
        assert!(matches!(
            search(&req, 1).expect_err("missing"),
            StudioError::RecordMissing { .. }
        ));
        req.collection = "users".into();
        assert!(matches!(
            search(&req, 1).expect_err("unknown"),
            StudioError::UnknownObject { .. }
        ));
    }

    #[test]
    fn the_statement_and_the_result_share_one_request() {
        let r = search(&request(SearchMode::Hybrid), 7).expect("searches");
        assert_eq!(r.execution, 7);
        assert!(
            r.sql
                .display
                .contains("bm25_score(body, 'distributed database')")
        );
        assert!(r.sql.executable.contains("ARRAY["));
        assert_eq!(r.vector_dimensions, Some(768));
    }

    #[test]
    fn an_excerpt_marks_one_span_for_adjacent_words_and_stays_on_boundaries() {
        let terms = query_terms("distributed database");
        let e = excerpt(
            "A distributed database partitions data across nodes.",
            &terms,
        );
        assert_eq!(e.spans.len(), 1);
        assert_eq!(&e.text[e.spans[0].0..e.spans[0].1], "distributed database");
        let long = format!("{} naïve distributed database here.", "word ".repeat(30));
        let e = excerpt(&long, &terms);
        assert!(e.text.starts_with('…'));
        let (s, t) = e.spans[0];
        assert_eq!(&e.text[s..t], "distributed database");
    }
}
