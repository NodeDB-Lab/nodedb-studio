//! Nearest-neighbor search over the resident articles.
//!
//! Distances are measured on the real vectors, exactly. Filters and the
//! source exclusion apply before ranking, so the top-k cut never drops a
//! match.

use crate::data::mock::vector::corpus::{
    COLLECTION, DIMENSIONS, FIELD, INDEX, RESIDENT, REVISION, id_of, meta, n_of, resident,
    vector_of,
};
use crate::models::vector::{Neighbor, NeighborRequest, NeighborResult, QuerySource};
use crate::services::error::StudioError;

fn rejected(reason: impl Into<String>) -> StudioError {
    StudioError::Rejected {
        reason: reason.into(),
    }
}

/// The index a request names, or the error for one that does not exist.
pub fn check_target(collection: &str, field: &str) -> Result<(), StudioError> {
    if collection == COLLECTION && field == FIELD {
        Ok(())
    } else {
        Err(StudioError::UnknownObject {
            kind: "vector index",
            id: format!("{collection}.{field}"),
        })
    }
}

fn passes(filter: &crate::models::records::Filter, n: u64) -> Result<bool, StudioError> {
    let m = meta(n);
    match filter.field.as_str() {
        "language" => Ok(m.language == Some(filter.value.as_str())),
        "model" => Ok(m.model == filter.value),
        other => Err(rejected(format!(
            "{COLLECTION} has no filter on `{other}`."
        ))),
    }
}

pub fn search(req: &NeighborRequest) -> Result<NeighborResult, StudioError> {
    check_target(&req.collection, &req.field)?;
    let (query, source_n) = match &req.source {
        QuerySource::Record(id) => {
            let n = n_of(id).ok_or_else(|| StudioError::RecordMissing {
                collection: req.collection.clone(),
                id: id.clone(),
            })?;
            (vector_of(n), Some(n))
        }
        QuerySource::Vector(v) => {
            if v.len() != DIMENSIONS {
                return Err(rejected(format!(
                    "The query vector has {} values. {INDEX} holds {DIMENSIONS}.",
                    v.len()
                )));
            }
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm == 0.0 {
                return Err(rejected("A cosine index cannot rank from a zero vector."));
            }
            (v.iter().map(|x| x / norm).collect(), None)
        }
    };

    let block = resident();
    let mut scored: Vec<(f32, u64)> = Vec::new();
    for n in 1..=RESIDENT {
        if req.exclude_source && source_n == Some(n) {
            continue;
        }
        let mut keep = true;
        for f in &req.filters {
            if !passes(f, n)? {
                keep = false;
                break;
            }
        }
        if !keep {
            continue;
        }
        let at = (n as usize - 1) * DIMENSIONS;
        let dot: f32 = block[at..at + DIMENSIONS]
            .iter()
            .zip(&query)
            .map(|(a, b)| a * b)
            .sum();
        scored.push((1.0 - dot, n));
    }
    scored.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

    let neighbors = scored
        .into_iter()
        .take(req.top_k as usize)
        .enumerate()
        .map(|(i, (distance, n))| {
            let m = meta(n);
            Neighbor {
                rank: i as u32 + 1,
                id: id_of(n),
                distance: distance.max(0.0),
                title: m.title,
                excerpt: m.body,
            }
        })
        .collect();
    Ok(NeighborResult {
        request: req.clone(),
        neighbors,
        revision: REVISION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::records::Filter;

    fn request(id: &str, k: u32) -> NeighborRequest {
        NeighborRequest {
            collection: COLLECTION.into(),
            field: FIELD.into(),
            source: QuerySource::Record(id.into()),
            top_k: k,
            exclude_source: true,
            filters: Vec::new(),
            ef_search: None,
        }
    }

    #[test]
    fn neighbors_rank_by_ascending_distance() {
        let r = search(&request("art104", 10)).expect("searches");
        assert_eq!(r.neighbors.len(), 10);
        assert!(
            r.neighbors
                .windows(2)
                .all(|w| w[0].distance <= w[1].distance)
        );
        assert_eq!(r.neighbors[0].rank, 1);
        assert!(r.neighbors.iter().all(|n| n.id != "art104"));
    }

    #[test]
    fn without_the_exclusion_the_source_ranks_first_at_zero() {
        let r = search(&NeighborRequest {
            exclude_source: false,
            ..request("art104", 3)
        })
        .expect("searches");
        assert_eq!(r.neighbors[0].id, "art104");
        assert!(r.neighbors[0].distance < 1e-4);
    }

    #[test]
    fn a_filter_applies_before_the_cut() {
        let r = search(&NeighborRequest {
            filters: vec![Filter {
                field: "language".into(),
                value: "ms".into(),
            }],
            ..request("art104", 5)
        })
        .expect("searches");
        assert_eq!(r.neighbors.len(), 5, "the cut still fills with matches");
        assert!(
            r.neighbors
                .iter()
                .all(|n| { meta(n_of(&n.id).expect("id")).language == Some("ms") })
        );
    }

    #[test]
    fn a_missing_record_is_named() {
        let err = search(&request("art9999999", 3)).expect_err("missing");
        assert!(matches!(err, StudioError::RecordMissing { .. }));
    }

    #[test]
    fn a_vector_of_the_wrong_length_is_rejected() {
        let err = search(&NeighborRequest {
            source: QuerySource::Vector(vec![0.1, 0.2]),
            ..request("", 3)
        })
        .expect_err("short");
        assert!(matches!(err, StudioError::Rejected { .. }));
    }

    #[test]
    fn a_vector_query_ranks_like_the_record_it_copies() {
        let by_record = search(&request("art104", 4)).expect("record");
        let by_vector = search(&NeighborRequest {
            source: QuerySource::Vector(vector_of(104)),
            ..request("", 4)
        })
        .expect("vector");
        let ids = |r: &NeighborResult| r.neighbors.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
        // The vector query does not exclude the source, so it ranks it first.
        assert_eq!(by_vector.neighbors[0].id, "art104");
        assert_eq!(ids(&by_vector)[1..], ids(&by_record)[..3]);
    }

    #[test]
    fn an_unknown_index_is_an_error() {
        assert!(check_target("users", "embedding").is_err());
    }
}
