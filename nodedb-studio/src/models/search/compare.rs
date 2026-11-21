//! Compare modes: the same query run as Keyword, Semantic, and Hybrid with
//! one source, one set of filters, and one limit, and how the three rankings
//! overlap and move records.
//!
//! The comparison reports overlap and rank changes only. It judges no
//! relevance: a quality score needs a named evaluation dataset and recorded
//! judgments, which a comparison does not have.

use crate::models::search::{SearchMode, SearchResult};

/// One record's rank in each mode. `None` when the mode's list leaves it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareRow {
    pub id: String,
    pub title: String,
    pub keyword: Option<u32>,
    pub semantic: Option<u32>,
    pub hybrid: Option<u32>,
}

impl CompareRow {
    /// How many places the record sits from its Keyword rank in Hybrid. A
    /// positive number means Hybrid ranks it higher. `None` unless both lists
    /// hold it.
    pub fn hybrid_moved_from_keyword(&self) -> Option<i64> {
        match (self.keyword, self.hybrid) {
            (Some(k), Some(h)) => Some(i64::from(k) - i64::from(h)),
            (None, _) | (_, None) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub limit: u32,
    pub rows: Vec<CompareRow>,
    pub keyword_count: usize,
    pub semantic_count: usize,
    pub hybrid_count: usize,
    /// Records both Keyword and Semantic return.
    pub keyword_and_semantic: usize,
    /// Records all three modes return.
    pub in_all_three: usize,
}

impl crate::services::async_state::IsEmpty for Comparison {
    fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// The comparison of three results. Each must be of its own mode.
pub fn compare(
    keyword: &SearchResult,
    semantic: &SearchResult,
    hybrid: &SearchResult,
) -> Comparison {
    debug_assert_eq!(keyword.request.mode, SearchMode::Keyword);
    debug_assert_eq!(semantic.request.mode, SearchMode::Semantic);
    debug_assert_eq!(hybrid.request.mode, SearchMode::Hybrid);
    let mut rows: Vec<CompareRow> = Vec::new();
    let mut place = |result: &SearchResult, set: fn(&mut CompareRow, u32)| {
        for h in &result.hits {
            let at = match rows.iter().position(|r| r.id == h.id) {
                Some(i) => i,
                None => {
                    rows.push(CompareRow {
                        id: h.id.clone(),
                        title: h.title.clone(),
                        keyword: None,
                        semantic: None,
                        hybrid: None,
                    });
                    rows.len() - 1
                }
            };
            set(&mut rows[at], h.rank);
        }
    };
    // The Hybrid list sets the row order, so the table reads in its ranking.
    place(hybrid, |r, k| r.hybrid = Some(k));
    place(keyword, |r, k| r.keyword = Some(k));
    place(semantic, |r, k| r.semantic = Some(k));
    rows.sort_by_key(|r| {
        (
            r.hybrid.unwrap_or(u32::MAX),
            r.keyword.unwrap_or(u32::MAX),
            r.semantic.unwrap_or(u32::MAX),
        )
    });
    Comparison {
        limit: hybrid.request.limit,
        keyword_count: keyword.hits.len(),
        semantic_count: semantic.hits.len(),
        hybrid_count: hybrid.hits.len(),
        keyword_and_semantic: rows
            .iter()
            .filter(|r| r.keyword.is_some() && r.semantic.is_some())
            .count(),
        in_all_three: rows
            .iter()
            .filter(|r| r.keyword.is_some() && r.semantic.is_some() && r.hybrid.is_some())
            .count(),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::search::{
        Excerpt, Fusion, Hit, Score, SearchRequest, SearchSql, VectorInput,
    };

    fn result(mode: SearchMode, ids: &[&str]) -> SearchResult {
        SearchResult {
            request: SearchRequest {
                collection: "articles".into(),
                mode,
                query: "q".into(),
                text_field: None,
                vector_field: None,
                vector: VectorInput::None,
                filters: Vec::new(),
                limit: 3,
                fusion: Fusion::default(),
            },
            execution: 1,
            hits: ids
                .iter()
                .enumerate()
                .map(|(i, id)| Hit {
                    rank: i as u32 + 1,
                    id: (*id).into(),
                    title: format!("Title {id}"),
                    excerpt: Excerpt::default(),
                    body: String::new(),
                    language: None,
                    score: Score::Bm25(1.0),
                    keyword: None,
                    vector: None,
                })
                .collect(),
            keyword_candidates: None,
            vector_candidates: None,
            metric: None,
            vector_dimensions: None,
            sql: SearchSql {
                display: String::new(),
                executable: String::new(),
                caveat: None,
            },
            observed_at: 0,
        }
    }

    #[test]
    fn rows_read_in_hybrid_order_and_keep_every_mode_rank() {
        let c = compare(
            &result(SearchMode::Keyword, &["a", "b", "c"]),
            &result(SearchMode::Semantic, &["b", "d", "a"]),
            &result(SearchMode::Hybrid, &["b", "a", "d"]),
        );
        let ids: Vec<&str> = c.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a", "d", "c"]);
        let b = &c.rows[0];
        assert_eq!(
            (b.keyword, b.semantic, b.hybrid),
            (Some(2), Some(1), Some(1))
        );
        assert_eq!(b.hybrid_moved_from_keyword(), Some(1));
        let last = &c.rows[3];
        assert_eq!(
            (last.keyword, last.semantic, last.hybrid),
            (Some(3), None, None)
        );
        assert_eq!(last.hybrid_moved_from_keyword(), None);
    }

    #[test]
    fn overlap_counts_records_not_ranks() {
        let c = compare(
            &result(SearchMode::Keyword, &["a", "b", "c"]),
            &result(SearchMode::Semantic, &["b", "d", "a"]),
            &result(SearchMode::Hybrid, &["b", "a", "d"]),
        );
        assert_eq!((c.keyword_and_semantic, c.in_all_three), (2, 2));
        assert_eq!(
            (c.keyword_count, c.semantic_count, c.hybrid_count),
            (3, 3, 3)
        );
    }
}
