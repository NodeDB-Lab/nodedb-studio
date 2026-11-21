//! The Projection tab: a sample of a vector index drawn in two dimensions.
//!
//! A projection is exploratory. It keeps the directions of greatest variance
//! and drops the rest, so two points that sit close on screen can be far
//! apart in the index, and the reverse. Ranked distances come from Search,
//! which reads the original vectors.

use crate::models::vector::VectorTarget;

/// The most records one projection samples. A larger request would not fit
/// in the worker's memory budget.
pub const MAX_SAMPLE: u32 = 20_000;
pub const DEFAULT_SAMPLE: u32 = 5_000;
pub const DEFAULT_SEED: u64 = 42;
/// The legend label for records that have no value in the color field.
pub const MISSING: &str = "Missing";
/// A point with no category: color by None.
pub const NO_CATEGORY: u8 = u8::MAX;

/// How a sample is reduced to two dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Pca,
    Umap,
    Tsne,
}

impl Method {
    pub const ALL: [Method; 3] = [Method::Pca, Method::Umap, Method::Tsne];

    pub fn label(self) -> &'static str {
        match self {
            Method::Pca => "PCA",
            Method::Umap => "UMAP",
            Method::Tsne => "t-SNE",
        }
    }

    /// Why Studio cannot run the method, or `Ok` when it can. A method with
    /// no worker is never faked with another method's output.
    pub fn support(self) -> Result<(), &'static str> {
        match self {
            Method::Pca => Ok(()),
            Method::Umap => {
                Err("UMAP needs a neighbor-graph worker, which this build does not include.")
            }
            Method::Tsne => {
                Err("t-SNE needs an optimizer worker, which this build does not include.")
            }
        }
    }

    /// The fewest vectors the method can project.
    pub fn min_vectors(self) -> u32 {
        match self {
            Method::Pca => 3,
            Method::Umap => 15,
            Method::Tsne => 10,
        }
    }
}

/// The sample sizes a target allows: from the method's minimum to the
/// smaller of the index population and the resource cap.
pub fn sample_bounds(target: &VectorTarget, method: Method) -> (u32, u32) {
    let cap = u32::try_from(target.population.min(u64::from(MAX_SAMPLE))).unwrap_or(MAX_SAMPLE);
    (method.min_vectors().min(cap), cap)
}

/// One projection, every part explicit.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionRequest {
    pub collection: String,
    pub field: String,
    pub method: Method,
    pub sample: u32,
    pub seed: u64,
    /// A metadata field of the collection, or `None` for one uniform color.
    pub color_by: Option<String>,
    /// Records the sample must hold, added by an explicit action.
    pub include: Vec<String>,
}

/// The projection controls as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionForm {
    pub method: Method,
    pub sample: String,
    pub seed: String,
    pub color_by: Option<String>,
}

impl Default for ProjectionForm {
    fn default() -> Self {
        Self {
            method: Method::Pca,
            sample: DEFAULT_SAMPLE.to_string(),
            seed: DEFAULT_SEED.to_string(),
            color_by: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionField {
    Method,
    Sample,
    Seed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionIssue {
    pub field: ProjectionField,
    pub message: String,
}

impl ProjectionForm {
    /// The request these controls ask, or every reason it cannot run.
    pub fn check(
        &self,
        target: &VectorTarget,
        include: &[String],
    ) -> Result<ProjectionRequest, Vec<ProjectionIssue>> {
        let mut issues = Vec::new();
        let mut issue = |field, message: String| issues.push(ProjectionIssue { field, message });

        if let Err(reason) = self.method.support() {
            issue(ProjectionField::Method, reason.to_string());
        }
        let (min, max) = sample_bounds(target, self.method);
        let sample = match self.sample.trim().replace(',', "").parse::<u32>() {
            Ok(n) if (min..=max).contains(&n) => n,
            Ok(n) if n > MAX_SAMPLE => {
                issue(
                    ProjectionField::Sample,
                    format!(
                        "{n} records do not fit the worker's memory limit. Sample at most {MAX_SAMPLE}."
                    ),
                );
                0
            }
            _ => {
                issue(
                    ProjectionField::Sample,
                    format!(
                        "Sample runs from {min} to {max}. {} needs at least {} vectors, and the index holds {}.",
                        self.method.label(),
                        self.method.min_vectors(),
                        target.population
                    ),
                );
                0
            }
        };
        let seed = match self.seed.trim().parse::<u64>() {
            Ok(s) => s,
            Err(_) => {
                issue(
                    ProjectionField::Seed,
                    "Seed is a whole number from 0.".to_string(),
                );
                0
            }
        };
        if let Some(field) = &self.color_by
            && !target.color_fields.contains(field)
        {
            issue(
                ProjectionField::Method,
                format!("{} has no field `{field}` to color by.", target.collection),
            );
        }

        if issues.is_empty() {
            Ok(ProjectionRequest {
                collection: target.collection.clone(),
                field: target.field.clone(),
                method: self.method,
                sample,
                seed,
                color_by: self.color_by.clone(),
                include: include.to_vec(),
            })
        } else {
            Err(issues)
        }
    }
}

/// One legend entry: a value of the color field and how many points carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub label: String,
    pub count: usize,
}

/// One sampled record in the plane. `x` and `y` run from -1 to 1.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionPoint {
    pub id: String,
    pub x: f32,
    pub y: f32,
    /// An index into `Projection::categories`, or `NO_CATEGORY`.
    pub category: u8,
}

/// A finished projection with everything that produced it, so the screen
/// can say what it shows and a later run can be compared with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub request: ProjectionRequest,
    /// Vectors the index held when the sample was drawn.
    pub population: u64,
    /// The index revision the vectors came from.
    pub revision: u64,
    /// The code that reduced the sample, e.g. `pca-power-iteration/1`.
    pub algorithm: String,
    /// Seconds since the Unix epoch.
    pub generated_at: i64,
    /// Share of the sample's variance each axis keeps.
    pub explained: [f32; 2],
    pub categories: Vec<Category>,
    pub points: Vec<ProjectionPoint>,
}

impl Projection {
    pub fn point(&self, id: &str) -> Option<&ProjectionPoint> {
        self.points.iter().find(|p| p.id == id)
    }

    /// The metadata line under the toolbar: what this image is.
    pub fn summary(&self) -> String {
        format!(
            "{} sampled of {} · {} · seed {}",
            crate::models::format::thousands(self.points.len() as u64),
            crate::models::format::thousands(self.population),
            self.request.method.label(),
            self.request.seed
        )
    }
}

impl crate::services::async_state::IsEmpty for Projection {
    fn is_empty(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::vector::Metric;

    fn target(population: u64) -> VectorTarget {
        VectorTarget {
            collection: "articles".into(),
            field: "embedding".into(),
            index: "idx".into(),
            id_field: "_id".into(),
            dimensions: 768,
            metric: Metric::Cosine,
            model: None,
            population,
            revision: 1,
            filter_fields: Vec::new(),
            color_fields: vec!["language".into()],
        }
    }

    #[test]
    fn the_default_form_is_a_valid_request() {
        let r = ProjectionForm::default()
            .check(&target(1_108_492), &[])
            .expect("valid");
        assert_eq!((r.sample, r.seed, r.method), (5_000, 42, Method::Pca));
    }

    #[test]
    fn bounds_follow_the_population_and_the_cap() {
        assert_eq!(
            sample_bounds(&target(1_108_492), Method::Pca),
            (3, MAX_SAMPLE)
        );
        assert_eq!(sample_bounds(&target(800), Method::Pca), (3, 800));
        assert_eq!(sample_bounds(&target(2), Method::Pca), (2, 2));
    }

    #[test]
    fn a_sample_outside_the_bounds_is_an_issue() {
        for bad in ["2", "0", "abc", "", "-5"] {
            let f = ProjectionForm {
                sample: bad.into(),
                ..ProjectionForm::default()
            };
            let issues = f.check(&target(1_000_000), &[]).expect_err("invalid");
            assert_eq!(issues[0].field, ProjectionField::Sample, "{bad}");
        }
    }

    #[test]
    fn a_sample_over_the_cap_names_the_limit() {
        let f = ProjectionForm {
            sample: "30,000".into(),
            ..ProjectionForm::default()
        };
        let issues = f.check(&target(1_000_000), &[]).expect_err("invalid");
        assert!(issues[0].message.contains("memory limit"));
    }

    #[test]
    fn an_unsupported_method_is_refused_with_a_reason() {
        let f = ProjectionForm {
            method: Method::Umap,
            ..ProjectionForm::default()
        };
        let issues = f.check(&target(1_000_000), &[]).expect_err("invalid");
        assert_eq!(issues[0].field, ProjectionField::Method);
        assert!(issues[0].message.contains("UMAP"));
    }

    #[test]
    fn a_bad_seed_is_an_issue() {
        let f = ProjectionForm {
            seed: "-1".into(),
            ..ProjectionForm::default()
        };
        let issues = f.check(&target(1_000_000), &[]).expect_err("invalid");
        assert_eq!(issues[0].field, ProjectionField::Seed);
    }

    #[test]
    fn an_unknown_color_field_is_refused() {
        let f = ProjectionForm {
            color_by: Some("nope".into()),
            ..ProjectionForm::default()
        };
        assert!(f.check(&target(1_000_000), &[]).is_err());
        let ok = ProjectionForm {
            color_by: Some("language".into()),
            ..ProjectionForm::default()
        };
        assert!(ok.check(&target(1_000_000), &[]).is_ok());
    }

    #[test]
    fn included_records_travel_in_the_request() {
        let r = ProjectionForm::default()
            .check(&target(1_000_000), &["art103".to_string()])
            .expect("valid");
        assert_eq!(r.include, vec!["art103".to_string()]);
    }
}
