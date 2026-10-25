//! GraphRAG retrieval: a question, an explicit retrieval source, and the
//! evidence that comes back with the graph around it.
//!
//! Retrieval finds records by vector similarity, then expands from what
//! they link to along a graph binding. It never writes an answer. An answer
//! needs a configured provider, and `RagSetup::provider` says whether one
//! exists.

use crate::models::graph::{GraphPath, GraphResult};

/// An embedding model and the vectors it produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingModel {
    pub name: String,
    pub dimension: u32,
}

/// A vector index retrieval can search: one field of one collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RagSource {
    pub collection: String,
    pub field: String,
    pub index: String,
    pub dimension: u32,
    pub metric: String,
}

impl RagSource {
    /// `articles.embedding`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.collection, self.field)
    }
}

/// What the retrieval form can choose from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RagSetup {
    pub sources: Vec<RagSource>,
    pub models: Vec<EmbeddingModel>,
    /// Graph bindings an expansion can follow.
    pub bindings: Vec<String>,
    /// The answer provider, if one is configured.
    pub provider: Option<String>,
}

impl crate::services::async_state::IsEmpty for RagSetup {
    fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

pub const MAX_HOPS: u8 = 3;
pub const MAX_NODES: u32 = 500;

/// One retrieval, every part explicit.
#[derive(Debug, Clone, PartialEq)]
pub struct RagRequest {
    pub question: String,
    pub collection: String,
    pub field: String,
    pub index: String,
    pub binding: String,
    pub model: String,
    pub dimension: u32,
    pub hops: u8,
    pub max_nodes: u32,
}

impl RagRequest {
    /// Every reason the request cannot run. `source_dimension` is the
    /// dimension of the chosen index.
    pub fn issues(&self, source_dimension: Option<u32>) -> Vec<String> {
        let mut out = Vec::new();
        if self.question.trim().is_empty() {
            out.push("Enter a question.".to_string());
        }
        match source_dimension {
            None => out.push("Choose a retrieval source with a vector index.".to_string()),
            Some(d) if d != self.dimension => out.push(format!(
                "{} produces {}-dimension vectors. {} holds {d}.",
                self.model, self.dimension, self.index
            )),
            Some(_) => {}
        }
        if self.binding.is_empty() {
            out.push("Choose a graph binding.".to_string());
        }
        if !(1..=MAX_HOPS).contains(&self.hops) {
            out.push(format!("Graph expansion runs from 1 to {MAX_HOPS} hops."));
        }
        if !(1..=MAX_NODES).contains(&self.max_nodes) {
            out.push(format!("Max nodes runs from 1 to {MAX_NODES}."));
        }
        out
    }
}

/// How a hit was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The vector index that matched the question.
    pub index: String,
    pub metric: String,
    /// The records the hit links to, where expansion started.
    pub seeds: Vec<String>,
    pub binding: String,
    pub hops: u8,
}

/// One retrieved record.
#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    pub rank: u32,
    /// `articles/a10023`.
    pub record: String,
    pub title: String,
    /// Cosine similarity, 0 to 1.
    pub score: f32,
    pub excerpt: String,
    pub provenance: Provenance,
    /// The graph path from a customer to what the record mentions, when
    /// expansion found one. Its nodes and edges are in the result's graph.
    pub path: Option<GraphPath>,
}

impl Evidence {
    pub fn id(&self) -> &str {
        self.record
            .split_once('/')
            .map_or(&self.record, |(_, id)| id)
    }

    /// Cosine distance: one minus the score.
    pub fn distance(&self) -> f32 {
        1.0 - self.score
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RagResult {
    pub evidence: Vec<Evidence>,
    /// The neighborhood expansion reached, for all evidence together.
    pub graph: GraphResult,
}

impl crate::services::async_state::IsEmpty for RagResult {
    fn is_empty(&self) -> bool {
        self.evidence.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RagRequest {
        RagRequest {
            question: "Which customers bought related products?".into(),
            collection: "articles".into(),
            field: "embedding".into(),
            index: "idx_articles_embedding".into(),
            binding: "commerce_graph".into(),
            model: "support-embed-v2".into(),
            dimension: 768,
            hops: 2,
            max_nodes: 50,
        }
    }

    #[test]
    fn the_design_request_is_valid() {
        assert!(request().issues(Some(768)).is_empty());
    }

    #[test]
    fn each_part_is_checked() {
        let blank = RagRequest {
            question: "  ".into(),
            ..request()
        };
        assert_eq!(blank.issues(Some(768)), ["Enter a question."]);
        assert_eq!(request().issues(None).len(), 1);
        let wrong = RagRequest {
            model: "support-embed-v1".into(),
            dimension: 384,
            ..request()
        };
        assert!(wrong.issues(Some(768))[0].contains("384-dimension"));
        for bad in [
            RagRequest {
                hops: 0,
                ..request()
            },
            RagRequest {
                hops: MAX_HOPS + 1,
                ..request()
            },
            RagRequest {
                max_nodes: 0,
                ..request()
            },
            RagRequest {
                max_nodes: MAX_NODES + 1,
                ..request()
            },
            RagRequest {
                binding: String::new(),
                ..request()
            },
        ] {
            assert_eq!(bad.issues(Some(768)).len(), 1, "{bad:?}");
        }
    }

    #[test]
    fn distance_is_one_minus_score() {
        let e = Evidence {
            rank: 1,
            record: "articles/a10023".into(),
            title: "t".into(),
            score: 0.82,
            excerpt: String::new(),
            provenance: Provenance {
                index: "i".into(),
                metric: "cosine".into(),
                seeds: Vec::new(),
                binding: "b".into(),
                hops: 2,
            },
            path: None,
        };
        assert_eq!(e.id(), "a10023");
        assert!((e.distance() - 0.18).abs() < 1e-6);
    }
}
