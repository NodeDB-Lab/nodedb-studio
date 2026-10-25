//! Reciprocal rank fusion of the keyword and vector rankings.
//!
//! Each ranking adds `weight / (k + rank)` for a record it holds, with a
//! one-based rank. A record only one ranking holds gets that one
//! contribution: a missing ranking adds nothing, and never a zero distance.
//! The raw BM25 score and the distance never enter the sum. They travel with
//! the contribution so the ranking can be read back.

use std::collections::HashMap;

use crate::models::search::{Fusion, Leg};

/// One ranking: record numbers best first, each with its raw value.
pub type Ranking = [(u64, f32)];

/// A fused record.
#[derive(Debug, Clone, PartialEq)]
pub struct Fused {
    /// The record's number.
    pub n: u64,
    pub score: f64,
    pub keyword: Option<Leg>,
    pub vector: Option<Leg>,
}

fn leg(rank: usize, raw: f32, weight: f64, k: u32) -> Leg {
    let rank = rank as u32 + 1;
    Leg {
        rank,
        raw,
        contribution: Some(weight / (f64::from(k) + f64::from(rank))),
    }
}

/// Fuse two rankings. Ties in the fused score break by the better of the two
/// ranks, then by the lower record number, so the order is stable.
pub fn fuse(keyword: &Ranking, vector: &Ranking, fusion: &Fusion) -> Vec<Fused> {
    let mut by_record: HashMap<u64, Fused> = HashMap::new();
    for (i, (n, raw)) in keyword.iter().enumerate() {
        let l = leg(i, *raw, fusion.keyword_weight, fusion.k);
        by_record
            .entry(*n)
            .or_insert(Fused {
                n: *n,
                score: 0.0,
                keyword: None,
                vector: None,
            })
            .keyword = Some(l);
    }
    for (i, (n, raw)) in vector.iter().enumerate() {
        let l = leg(i, *raw, fusion.vector_weight, fusion.k);
        by_record
            .entry(*n)
            .or_insert(Fused {
                n: *n,
                score: 0.0,
                keyword: None,
                vector: None,
            })
            .vector = Some(l);
    }
    let mut out: Vec<Fused> = by_record.into_values().collect();
    for f in &mut out {
        f.score = [f.keyword, f.vector]
            .into_iter()
            .flatten()
            .filter_map(|l| l.contribution)
            .sum();
    }
    let best = |f: &Fused| {
        [f.keyword, f.vector]
            .into_iter()
            .flatten()
            .map(|l| l.rank)
            .min()
            .unwrap_or(u32::MAX)
    };
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(best(a).cmp(&best(b)))
            .then(a.n.cmp(&b.n))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ranking that puts `order` first to last, with a raw value of 0.
    fn ranking(order: &[u64]) -> Vec<(u64, f32)> {
        order.iter().map(|n| (*n, 0.0)).collect()
    }

    /// The design's fixture: smoothing 60, equal weights, five records.
    fn design() -> Vec<Fused> {
        // Keyword ranks: 087 first, 104 second, 138 third, 212 fourth, 055 fifth.
        let keyword = ranking(&[87, 104, 138, 212, 55]);
        // Vector ranks: 104 first, 212 second, 087 third, 055 fourth, and 138
        // eighth behind three other records.
        let vector = ranking(&[104, 212, 87, 55, 901, 902, 903, 138]);
        fuse(&keyword, &vector, &Fusion::default())
    }

    #[test]
    fn the_design_scores_match_to_six_places() {
        let fused = design();
        let score = |n: u64| {
            fused
                .iter()
                .find(|f| f.n == n)
                .map(|f| f.score)
                .unwrap_or(0.0)
        };
        let six = |x: f64| (x * 1e6).round() / 1e6;
        assert_eq!(six(score(104)), 0.032522);
        assert_eq!(six(score(87)), 0.032266);
        assert_eq!(six(score(212)), 0.031754);
        assert_eq!(six(score(55)), 0.031010);
        assert_eq!(six(score(138)), 0.030579);
    }

    #[test]
    fn the_design_order_is_the_fused_order() {
        let order: Vec<u64> = design().iter().take(5).map(|f| f.n).collect();
        assert_eq!(order, vec![104, 87, 212, 55, 138]);
    }

    #[test]
    fn contributions_are_weight_over_k_plus_rank() {
        let fused = design();
        let f = fused.iter().find(|f| f.n == 104).expect("104");
        let (k, v) = (f.keyword.expect("keyword"), f.vector.expect("vector"));
        assert_eq!((k.rank, v.rank), (2, 1));
        assert_eq!(k.contribution, Some(1.0 / 62.0));
        assert_eq!(v.contribution, Some(1.0 / 61.0));
    }

    #[test]
    fn a_record_one_ranking_holds_has_one_contribution_and_no_fake_leg() {
        let fused = design();
        let f = fused.iter().find(|f| f.n == 901).expect("901");
        assert!(f.keyword.is_none());
        assert_eq!(
            f.score,
            f.vector.and_then(|l| l.contribution).unwrap_or(f64::NAN)
        );
    }

    #[test]
    fn weights_and_smoothing_are_separate_inputs() {
        let keyword = ranking(&[1]);
        let vector = ranking(&[2]);
        let weighted = fuse(
            &keyword,
            &vector,
            &Fusion {
                vector_weight: 3.0,
                ..Fusion::default()
            },
        );
        assert_eq!(weighted[0].n, 2, "a heavier source ranks its record first");
        let smoothed = fuse(
            &keyword,
            &vector,
            &Fusion {
                k: 10,
                ..Fusion::default()
            },
        );
        assert_eq!(smoothed[0].score, 1.0 / 11.0);
    }

    #[test]
    fn equal_scores_break_by_the_better_rank_then_the_lower_number() {
        let fused = fuse(&ranking(&[9]), &ranking(&[4]), &Fusion::default());
        assert_eq!(fused[0].score, fused[1].score);
        assert_eq!(fused.iter().map(|f| f.n).collect::<Vec<_>>(), vec![4, 9]);
    }

    #[test]
    fn two_empty_rankings_fuse_to_nothing() {
        assert!(fuse(&[], &[], &Fusion::default()).is_empty());
    }
}
