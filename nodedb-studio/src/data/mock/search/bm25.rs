//! BM25 over the resident articles, one inverted index per text field.
//!
//! Scoring is Okapi BM25 with `k1 = 1.2` and `b = 0.75`, and the idf of
//! Lucene: `ln(1 + (N - n + 0.5) / (n + 0.5))`. A query matches a record that
//! holds any of its terms, and the score sums the terms. Filters apply before
//! the cut, so the cut never drops a match.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::analyzer::analyze;
use crate::data::mock::vector::{ArticleText, RESIDENT_ARTICLES, article_text};

const K1: f32 = 1.2;
const B: f32 = 0.75;

/// One inverted index over one field.
struct FieldIndex {
    /// Term to `(document slot, term frequency)`.
    postings: HashMap<String, Vec<(u32, u32)>>,
    lengths: Vec<u32>,
    average: f32,
}

impl FieldIndex {
    fn build(docs: &[ArticleText], text: impl Fn(&ArticleText) -> &str) -> Self {
        let mut postings: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
        let mut lengths = Vec::with_capacity(docs.len());
        for (slot, doc) in docs.iter().enumerate() {
            let mut counts: HashMap<String, u32> = HashMap::new();
            let tokens = analyze(text(doc)).tokens;
            lengths.push(tokens.len() as u32);
            for t in tokens {
                *counts.entry(t.stem).or_default() += 1;
            }
            for (term, tf) in counts {
                postings.entry(term).or_default().push((slot as u32, tf));
            }
        }
        let total: u64 = lengths.iter().map(|l| u64::from(*l)).sum();
        let average = (total as f32 / lengths.len().max(1) as f32).max(1.0);
        Self {
            postings,
            lengths,
            average,
        }
    }
}

struct Corpus {
    docs: Vec<ArticleText>,
    title: FieldIndex,
    body: FieldIndex,
}

fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let docs: Vec<ArticleText> = (1..=RESIDENT_ARTICLES).map(article_text).collect();
        let title = FieldIndex::build(&docs, |d| d.title.as_str());
        let body = FieldIndex::build(&docs, |d| d.body.as_str());
        Corpus { docs, title, body }
    })
}

/// The articles the index covers, in id order.
pub fn documents() -> &'static [ArticleText] {
    &corpus().docs
}

/// A scored record: its slot in `documents()` and its BM25 score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    pub slot: usize,
    pub score: f32,
}

/// The indexed field's index, or `None` for a field the index does not hold.
fn field(name: &str) -> Option<&'static FieldIndex> {
    match name {
        "title" => Some(&corpus().title),
        "body" => Some(&corpus().body),
        _ => None,
    }
}

/// Records that hold any of `terms` in `field_name`, best first, ties broken
/// by the lower slot, cut at `limit` after `keep` drops the filtered ones.
/// `None` when the index holds no such field.
pub fn search(
    field_name: &str,
    terms: &[String],
    keep: impl Fn(&ArticleText) -> bool,
    limit: usize,
) -> Option<Vec<Scored>> {
    let index = field(field_name)?;
    let docs = &corpus().docs;
    let n = docs.len() as f32;
    let mut scores: HashMap<u32, f32> = HashMap::new();
    for term in terms {
        let Some(list) = index.postings.get(term) else {
            continue;
        };
        let df = list.len() as f32;
        let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
        for (slot, tf) in list {
            let len = index.lengths[*slot as usize] as f32;
            let tf = *tf as f32;
            let norm = tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len / index.average));
            *scores.entry(*slot).or_default() += idf * norm;
        }
    }
    let mut out: Vec<Scored> = scores
        .into_iter()
        .filter(|(slot, _)| keep(&docs[*slot as usize]))
        .map(|(slot, score)| Scored {
            slot: slot as usize,
            score,
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.slot.cmp(&b.slot)));
    out.truncate(limit);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::search::analyzer::query_terms;

    fn ids(found: &[Scored]) -> Vec<&str> {
        found
            .iter()
            .map(|s| documents()[s.slot].id.as_str())
            .collect()
    }

    #[test]
    fn a_matching_record_ranks_and_a_missing_term_ranks_nothing() {
        let found = search("body", &query_terms("distributed database"), |_| true, 10)
            .expect("body is indexed");
        assert_eq!(found.len(), 10);
        assert!(found.windows(2).all(|w| w[0].score >= w[1].score));
        assert!(
            search("body", &query_terms("zzzxqy"), |_| true, 10)
                .expect("body")
                .is_empty()
        );
    }

    #[test]
    fn the_original_article_matches_its_own_words() {
        let found = search("body", &query_terms("enable"), |_| true, 3).expect("body");
        assert_eq!(ids(&found)[0], "art104");
    }

    #[test]
    fn a_rarer_term_scores_higher_than_a_common_one() {
        let rare = search("body", &query_terms("regions"), |_| true, 1).expect("body");
        let common = search("body", &query_terms("database"), |_| true, 1).expect("body");
        assert!(rare[0].score > common[0].score);
    }

    #[test]
    fn a_filter_applies_before_the_cut() {
        let all = search("body", &query_terms("database"), |_| true, 5).expect("body");
        let ms = search(
            "body",
            &query_terms("database"),
            |d| d.language == Some("ms"),
            5,
        )
        .expect("body");
        assert_eq!(ms.len(), 5, "the cut still fills with matches");
        assert!(
            ms.iter()
                .all(|s| documents()[s.slot].language == Some("ms"))
        );
        assert_ne!(ids(&all), ids(&ms));
    }

    #[test]
    fn a_field_the_index_lacks_is_none() {
        assert!(search("language", &["en".into()], |_| true, 3).is_none());
    }

    #[test]
    fn ties_break_by_the_lower_slot() {
        let found = search("body", &query_terms("vector"), |_| true, 50).expect("body");
        for w in found.windows(2) {
            if w[0].score == w[1].score {
                assert!(w[0].slot < w[1].slot);
            }
        }
    }
}
