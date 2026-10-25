//! The Search form as typed, and the check that turns it into a request.
//!
//! Every input stays in the form when the mode changes, so switching from
//! Hybrid to Keyword and back loses nothing. `check` reads only the inputs
//! the mode uses and names every reason a request cannot run yet.

use crate::models::records::Filter;
use crate::models::search::{
    DEFAULT_LIMIT, Fusion, MAX_CANDIDATES, MAX_K, MAX_LIMIT, MAX_WEIGHT, SearchCollection,
    SearchMode, SearchRequest, VectorInput,
};
use crate::models::vector::parse_vector;

/// Which control an issue belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueField {
    Collection,
    Query,
    TextField,
    VectorField,
    Vector,
    Limit,
    Filter,
    Fusion,
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

/// The fusion inputs as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct FusionForm {
    pub k: String,
    pub keyword_weight: String,
    pub vector_weight: String,
    pub candidates: String,
}

impl Default for FusionForm {
    fn default() -> Self {
        let d = Fusion::default();
        Self {
            k: d.k.to_string(),
            keyword_weight: d.keyword_weight.to_string(),
            vector_weight: d.vector_weight.to_string(),
            candidates: d.candidates.to_string(),
        }
    }
}

impl FusionForm {
    fn check(&self) -> Result<Fusion, Vec<Issue>> {
        let mut issues = Vec::new();
        let whole = |text: &str, name: &str, lo: u32, hi: u32, issues: &mut Vec<Issue>| match text
            .trim()
            .parse::<u32>()
        {
            Ok(v) if (lo..=hi).contains(&v) => v,
            _ => {
                issues.push(Issue::new(
                    IssueField::Fusion,
                    format!("{name} is a whole number from {lo} to {hi}."),
                ));
                lo
            }
        };
        let weight =
            |text: &str, name: &str, issues: &mut Vec<Issue>| match text.trim().parse::<f64>() {
                Ok(v) if v.is_finite() && v > 0.0 && v <= MAX_WEIGHT => v,
                _ => {
                    issues.push(Issue::new(
                        IssueField::Fusion,
                        format!("{name} is a number above 0 and up to {MAX_WEIGHT}."),
                    ));
                    1.0
                }
            };
        let k = whole(&self.k, "Smoothing k", 1, MAX_K, &mut issues);
        let candidates = whole(
            &self.candidates,
            "Candidates per source",
            1,
            MAX_CANDIDATES,
            &mut issues,
        );
        let keyword_weight = weight(&self.keyword_weight, "The keyword weight", &mut issues);
        let vector_weight = weight(&self.vector_weight, "The vector weight", &mut issues);
        if issues.is_empty() {
            Ok(Fusion {
                k,
                keyword_weight,
                vector_weight,
                candidates,
            })
        } else {
            Err(issues)
        }
    }
}

/// How the vector input is being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VectorKind {
    #[default]
    Record,
    Values,
}

impl VectorKind {
    pub const ALL: [VectorKind; 2] = [VectorKind::Record, VectorKind::Values];

    pub fn label(self) -> &'static str {
        match self {
            VectorKind::Record => "Stored record",
            VectorKind::Values => "Pasted values",
        }
    }
}

/// The editor of the vector input. It applies to the form on Apply.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VectorEditor {
    pub kind: VectorKind,
    pub record_id: String,
    pub values: String,
}

impl VectorEditor {
    /// The editor opened on the form's current input.
    pub fn of(input: &VectorInput) -> Self {
        match input {
            VectorInput::None => Self::default(),
            VectorInput::Record(id) => Self {
                kind: VectorKind::Record,
                record_id: id.clone(),
                values: String::new(),
            },
            VectorInput::Values(v) => Self {
                kind: VectorKind::Values,
                record_id: String::new(),
                values: format!("{v:?}"),
            },
        }
    }

    /// The input this editor holds, or why it is not one.
    pub fn apply(&self, dimensions: u32) -> Result<VectorInput, String> {
        match self.kind {
            VectorKind::Record => {
                let id = self.record_id.trim();
                if id.is_empty() {
                    Err("Enter the id of a record whose stored vector is the query.".to_string())
                } else {
                    Ok(VectorInput::Record(id.to_string()))
                }
            }
            VectorKind::Values => check_values(&self.values, dimensions).map(VectorInput::Values),
        }
    }
}

/// The numbers in `text`, when there are exactly `dimensions` finite ones
/// and they are not all zero.
pub fn check_values(text: &str, dimensions: u32) -> Result<Vec<f32>, String> {
    let values = parse_vector(text)?;
    if values.len() != dimensions as usize {
        return Err(format!(
            "The vector has {} values. The index holds {dimensions}.",
            values.len()
        ));
    }
    if values.iter().all(|v| *v == 0.0) {
        return Err("A cosine index cannot rank from a zero vector.".to_string());
    }
    Ok(values)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchForm {
    pub mode: SearchMode,
    pub query: String,
    pub text_field: String,
    pub vector_field: String,
    pub vector: VectorInput,
    pub limit: String,
    pub filters: Vec<Filter>,
    pub fusion: FusionForm,
}

impl Default for SearchForm {
    fn default() -> Self {
        Self {
            mode: SearchMode::Keyword,
            query: String::new(),
            text_field: String::new(),
            vector_field: String::new(),
            vector: VectorInput::None,
            limit: DEFAULT_LIMIT.to_string(),
            filters: Vec::new(),
            fusion: FusionForm::default(),
        }
    }
}

impl SearchForm {
    /// Keep the fields ones the collection has, and the mode one it can
    /// answer. A field the user chose stays while it is valid.
    pub fn settle(&mut self, collection: &SearchCollection) {
        if let Some(text) = &collection.text
            && !text.fields.contains(&self.text_field)
        {
            self.text_field = text
                .fields
                .iter()
                .find(|f| f.as_str() == "body")
                .or_else(|| text.fields.first())
                .cloned()
                .unwrap_or_default();
        }
        if let Some(v) = &collection.vector
            && self.vector_field != v.field
        {
            self.vector_field.clone_from(&v.field);
        }
        self.filters
            .retain(|f| collection.filter_fields.contains(&f.field));
        if !collection.supports(self.mode)
            && let Some(mode) = SearchMode::ALL
                .into_iter()
                .find(|m| collection.supports(*m))
        {
            self.mode = mode;
        }
    }

    /// The request this form asks, or every reason it cannot run yet.
    pub fn check(&self, collection: &SearchCollection) -> Result<SearchRequest, Vec<Issue>> {
        let mut issues = Vec::new();
        if !collection.supports(self.mode) {
            let missing = if self.mode.uses_text() && collection.text.is_none() {
                "a full-text index"
            } else {
                "a vector index"
            };
            issues.push(Issue::new(
                IssueField::Collection,
                format!(
                    "{} has no {missing}, which {} search needs.",
                    collection.name,
                    self.mode.label()
                ),
            ));
        }
        if self.mode.uses_text() {
            if self.query.trim().is_empty() {
                issues.push(Issue::new(
                    IssueField::Query,
                    "Enter the words to search for.",
                ));
            }
            if !collection.text_fields().contains(&self.text_field) {
                issues.push(Issue::new(
                    IssueField::TextField,
                    "Choose an indexed text field.",
                ));
            }
        }
        if self.mode.uses_vector() {
            match (&collection.vector, &self.vector) {
                (Some(v), _) if v.field != self.vector_field => issues.push(Issue::new(
                    IssueField::VectorField,
                    "Choose a vector field with an index.",
                )),
                (Some(_), VectorInput::None) => issues.push(Issue::new(
                    IssueField::Vector,
                    "Attach a query vector or name a record. Text is not embedded.",
                )),
                (Some(v), VectorInput::Values(values)) if values.len() != v.dimensions as usize => {
                    issues.push(Issue::new(
                        IssueField::Vector,
                        format!(
                            "The vector has {} values. {} holds {}.",
                            values.len(),
                            v.index,
                            v.dimensions
                        ),
                    ));
                }
                (Some(_), VectorInput::Values(values)) if values.iter().any(|x| !x.is_finite()) => {
                    issues.push(Issue::new(
                        IssueField::Vector,
                        "Every vector value must be a finite number.",
                    ));
                }
                (Some(_), VectorInput::Record(id)) if id.trim().is_empty() => {
                    issues.push(Issue::new(
                        IssueField::Vector,
                        "Name the record whose vector is the query.",
                    ))
                }
                (Some(_), _) | (None, _) => {}
            }
        }
        let limit = match self.limit.trim().parse::<u32>() {
            Ok(v) if (1..=MAX_LIMIT).contains(&v) => v,
            _ => {
                issues.push(Issue::new(
                    IssueField::Limit,
                    format!("The limit is a whole number from 1 to {MAX_LIMIT}."),
                ));
                DEFAULT_LIMIT
            }
        };
        for f in &self.filters {
            if !collection.filter_fields.contains(&f.field) {
                issues.push(Issue::new(
                    IssueField::Filter,
                    format!("{} has no filter on `{}`.", collection.name, f.field),
                ));
            }
        }
        let fusion = if self.mode == SearchMode::Hybrid {
            match self.fusion.check() {
                Ok(f) if f.candidates < limit => {
                    issues.push(Issue::new(
                        IssueField::Fusion,
                        format!("Candidates per source must be at least the limit, {limit}."),
                    ));
                    f
                }
                Ok(f) => f,
                Err(mut more) => {
                    issues.append(&mut more);
                    Fusion::default()
                }
            }
        } else {
            Fusion::default()
        };
        if !issues.is_empty() {
            return Err(issues);
        }
        Ok(SearchRequest {
            collection: collection.name.clone(),
            mode: self.mode,
            query: if self.mode.uses_text() {
                self.query.trim().to_string()
            } else {
                String::new()
            },
            text_field: self.mode.uses_text().then(|| self.text_field.clone()),
            vector_field: self.mode.uses_vector().then(|| self.vector_field.clone()),
            vector: if self.mode.uses_vector() {
                self.vector.clone()
            } else {
                VectorInput::None
            },
            filters: self.filters.clone(),
            limit,
            fusion,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::search::{SearchText, SearchVector};
    use crate::models::vector::Metric;

    fn articles() -> SearchCollection {
        SearchCollection {
            name: "articles".into(),
            id_field: "_id".into(),
            text: Some(SearchText {
                index: "idx_articles_text".into(),
                fields: vec!["title".into(), "body".into()],
                analyzer: "English".into(),
                binding_revision: 7,
            }),
            vector: Some(SearchVector {
                field: "embedding".into(),
                index: "idx_articles_embedding".into(),
                dimensions: 4,
                metric: Metric::Cosine,
            }),
            filter_fields: vec!["language".into()],
        }
    }

    fn form(mode: SearchMode) -> SearchForm {
        let mut f = SearchForm {
            mode,
            query: "distributed database".into(),
            vector: VectorInput::Values(vec![0.1, 0.2, 0.3, 0.4]),
            ..SearchForm::default()
        };
        f.settle(&articles());
        f
    }

    #[test]
    fn settle_picks_body_and_the_vector_field() {
        let f = form(SearchMode::Hybrid);
        assert_eq!(
            (f.text_field.as_str(), f.vector_field.as_str()),
            ("body", "embedding")
        );
    }

    #[test]
    fn a_keyword_request_leaves_the_vector_out() {
        let r = form(SearchMode::Keyword).check(&articles()).expect("ok");
        assert_eq!(r.vector, VectorInput::None);
        assert_eq!(r.vector_field, None);
        assert_eq!(r.text_field.as_deref(), Some("body"));
    }

    #[test]
    fn a_semantic_request_needs_a_vector_and_no_words() {
        let mut f = form(SearchMode::Semantic);
        f.query.clear();
        assert!(f.check(&articles()).is_ok());
        f.vector = VectorInput::None;
        let issues = f.check(&articles()).expect_err("no vector");
        assert!(issues.iter().any(|i| i.field == IssueField::Vector));
    }

    #[test]
    fn a_vector_of_the_wrong_size_is_refused() {
        let mut f = form(SearchMode::Hybrid);
        f.vector = VectorInput::Values(vec![1.0, 2.0]);
        let issues = f.check(&articles()).expect_err("short");
        assert!(issues[0].message.contains("2 values"), "{issues:?}");
    }

    #[test]
    fn hybrid_reads_the_fusion_and_names_each_bad_value() {
        let mut f = form(SearchMode::Hybrid);
        f.fusion.k = "0".into();
        f.fusion.vector_weight = "-1".into();
        let issues = f.check(&articles()).expect_err("bad");
        assert_eq!(issues.len(), 2, "{issues:?}");
        // Keyword ignores the same inputs.
        f.mode = SearchMode::Keyword;
        assert!(f.check(&articles()).is_ok());
    }

    #[test]
    fn candidates_cover_the_limit() {
        let mut f = form(SearchMode::Hybrid);
        f.fusion.candidates = "10".into();
        let issues = f.check(&articles()).expect_err("few");
        assert!(issues[0].message.contains("at least the limit"));
    }

    #[test]
    fn settle_drops_a_filter_the_collection_cannot_apply() {
        let mut f = form(SearchMode::Keyword);
        f.filters = vec![
            Filter {
                field: "language".into(),
                value: "en".into(),
            },
            Filter {
                field: "colour".into(),
                value: "red".into(),
            },
        ];
        f.settle(&articles());
        assert_eq!(f.filters.len(), 1);
    }

    #[test]
    fn an_empty_query_is_not_ready() {
        let mut f = form(SearchMode::Keyword);
        f.query = "  ".into();
        let issues = f.check(&articles()).expect_err("empty");
        assert_eq!(issues[0].field, IssueField::Query);
    }

    #[test]
    fn the_editor_checks_pasted_values_against_the_size() {
        let e = VectorEditor {
            kind: VectorKind::Values,
            record_id: String::new(),
            values: "[0.1, 0.2, 0.3, 0.4]".into(),
        };
        assert_eq!(
            e.apply(4),
            Ok(VectorInput::Values(vec![0.1, 0.2, 0.3, 0.4]))
        );
        assert!(e.apply(3).is_err());
        let zero = VectorEditor {
            values: "[0, 0, 0, 0]".into(),
            ..e.clone()
        };
        assert!(zero.apply(4).is_err());
        let rec = VectorEditor {
            kind: VectorKind::Record,
            record_id: " art104 ".into(),
            values: String::new(),
        };
        assert_eq!(rec.apply(4), Ok(VectorInput::Record("art104".into())));
    }

    #[test]
    fn a_mode_the_collection_cannot_answer_is_replaced() {
        let mut c = articles();
        c.vector = None;
        let mut f = form(SearchMode::Hybrid);
        f.settle(&c);
        assert_eq!(f.mode, SearchMode::Keyword);
    }
}
