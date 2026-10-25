//! A projection of a seeded sample of the index.

use std::collections::BTreeMap;

use crate::data::mock::vector::corpus::{
    DIMENSIONS, POPULATION, REVISION, id_of, meta, n_of, vector_of,
};
use crate::data::mock::vector::search::check_target;
use crate::models::format::epoch;
use crate::models::vector_pca::{PCA_VERSION, PcaError, project_2d, sample_ids};
use crate::models::vector_projection::{
    Category, MISSING, NO_CATEGORY, Projection, ProjectionPoint, ProjectionRequest,
};
use crate::services::error::StudioError;

fn rejected(reason: impl Into<String>) -> StudioError {
    StudioError::Rejected {
        reason: reason.into(),
    }
}

/// The value of `field` for record `n`, `None` when the record has none.
fn value_of(field: &str, n: u64) -> Result<Option<String>, StudioError> {
    let m = meta(n);
    match field {
        "language" => Ok(m.language.map(str::to_string)),
        "model" => Ok(Some(m.model.to_string())),
        other => Err(rejected(format!(
            "articles has no field `{other}` to color by."
        ))),
    }
}

/// Reduce the sample to two dimensions. `canceled` is polled between PCA
/// iterations: a canceled run returns an error and no output.
pub fn project(
    req: &ProjectionRequest,
    canceled: &dyn Fn() -> bool,
) -> Result<Projection, StudioError> {
    check_target(&req.collection, &req.field)?;
    req.method.support().map_err(rejected)?;
    if req.sample < req.method.min_vectors() || u64::from(req.sample) > POPULATION {
        return Err(rejected(format!(
            "{} needs at least {} vectors.",
            req.method.label(),
            req.method.min_vectors()
        )));
    }

    let mut forced = Vec::with_capacity(req.include.len());
    for id in &req.include {
        forced.push(n_of(id).ok_or_else(|| StudioError::RecordMissing {
            collection: req.collection.clone(),
            id: id.clone(),
        })?);
    }
    let ids = sample_ids(POPULATION, req.sample as usize, req.seed, &forced);

    let mut flat: Vec<f32> = Vec::with_capacity(ids.len() * DIMENSIONS);
    for n in &ids {
        flat.extend(vector_of(*n));
    }
    let pca = project_2d(&flat, DIMENSIONS, req.seed, canceled).map_err(|e| match e {
        PcaError::Canceled => rejected("The projection was canceled."),
        other @ (PcaError::TooFewVectors { .. } | PcaError::RaggedInput { .. }) => {
            rejected(other.to_string())
        }
    })?;

    // Legend: each value of the color field, commonest first, Missing last.
    let mut counts: BTreeMap<Option<String>, usize> = BTreeMap::new();
    let mut values: Vec<Option<String>> = Vec::new();
    if let Some(field) = &req.color_by {
        for n in &ids {
            let v = value_of(field, *n)?;
            *counts.entry(v.clone()).or_default() += 1;
            values.push(v);
        }
    }
    let mut order: Vec<(Option<String>, usize)> = counts.into_iter().collect();
    order.sort_by(|a, b| match (&a.0, &b.0) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(x), Some(y)) => b.1.cmp(&a.1).then(x.cmp(y)),
    });
    let categories: Vec<Category> = order
        .iter()
        .map(|(v, count)| Category {
            label: v.clone().unwrap_or_else(|| MISSING.to_string()),
            count: *count,
        })
        .collect();

    let points = ids
        .iter()
        .zip(&pca.scores)
        .enumerate()
        .map(|(i, (n, s))| ProjectionPoint {
            id: id_of(*n),
            x: s[0],
            y: s[1],
            category: values
                .get(i)
                .and_then(|v| order.iter().position(|(o, _)| o == v))
                .and_then(|p| u8::try_from(p).ok())
                .unwrap_or(NO_CATEGORY),
        })
        .collect();

    Ok(Projection {
        request: req.clone(),
        population: POPULATION,
        revision: REVISION,
        algorithm: PCA_VERSION.to_string(),
        generated_at: epoch(2026, 6, 13, 10, 41, 20),
        explained: pca.explained,
        categories,
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::vector::corpus::{COLLECTION, FIELD};
    use crate::models::vector_projection::Method;

    fn request(sample: u32) -> ProjectionRequest {
        ProjectionRequest {
            collection: COLLECTION.into(),
            field: FIELD.into(),
            method: Method::Pca,
            sample,
            seed: 42,
            color_by: None,
            include: Vec::new(),
        }
    }

    fn never() -> bool {
        false
    }

    #[test]
    fn the_sample_has_the_size_asked_and_every_point_is_a_record() {
        let p = project(&request(300), &never).expect("projects");
        assert_eq!(p.points.len(), 300);
        assert!(p.points.iter().all(|pt| n_of(&pt.id).is_some()));
        assert!(p.points.iter().all(|pt| pt.category == NO_CATEGORY));
        assert!(p.categories.is_empty());
        assert_eq!(p.population, POPULATION);
    }

    #[test]
    fn coordinates_are_computed_not_listed() {
        let p = project(&request(300), &never).expect("projects");
        assert!(
            p.points
                .iter()
                .all(|pt| pt.x.abs() <= 1.0 && pt.y.abs() <= 1.0)
        );
        let spread = p.points.iter().map(|pt| pt.x).fold(f32::MIN, f32::max)
            - p.points.iter().map(|pt| pt.x).fold(f32::MAX, f32::min);
        assert!(spread > 0.5);
        assert!(p.explained[0] >= p.explained[1] && p.explained[0] > 0.0);
    }

    #[test]
    fn one_seed_gives_one_projection_and_another_gives_another_sample() {
        let a = project(&request(200), &never).expect("a");
        let b = project(&request(200), &never).expect("b");
        assert_eq!(a, b);
        let c = project(
            &ProjectionRequest {
                seed: 7,
                ..request(200)
            },
            &never,
        )
        .expect("c");
        let ids = |p: &Projection| p.points.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
        assert_ne!(ids(&a), ids(&c));
    }

    #[test]
    fn an_included_record_is_in_the_sample() {
        let p = project(
            &ProjectionRequest {
                include: vec!["art103".into()],
                ..request(100)
            },
            &never,
        )
        .expect("projects");
        assert_eq!(p.points.len(), 100);
        assert!(p.point("art103").is_some());
    }

    #[test]
    fn coloring_by_language_lists_each_value_and_a_missing_group() {
        let p = project(
            &ProjectionRequest {
                color_by: Some("language".into()),
                ..request(600)
            },
            &never,
        )
        .expect("projects");
        let labels: Vec<&str> = p.categories.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels.first(), Some(&"en"));
        assert_eq!(labels.last(), Some(&MISSING));
        assert_eq!(p.categories.iter().map(|c| c.count).sum::<usize>(), 600);
        assert!(p.points.iter().all(|pt| pt.category != NO_CATEGORY));
    }

    #[test]
    fn a_method_without_a_worker_is_refused() {
        let err = project(
            &ProjectionRequest {
                method: Method::Umap,
                ..request(100)
            },
            &never,
        )
        .expect_err("refused");
        assert!(matches!(err, StudioError::Rejected { .. }));
    }

    #[test]
    fn too_few_vectors_is_refused_with_the_minimum() {
        let err = project(&request(2), &never).expect_err("refused");
        assert!(err.to_string().contains("at least 3"));
    }

    #[test]
    fn a_canceled_projection_has_no_output() {
        assert!(project(&request(100), &|| true).is_err());
    }

    #[test]
    fn an_unknown_included_record_is_named() {
        let err = project(
            &ProjectionRequest {
                include: vec!["nope".into()],
                ..request(100)
            },
            &never,
        )
        .expect_err("missing");
        assert!(matches!(err, StudioError::RecordMissing { .. }));
    }
}
