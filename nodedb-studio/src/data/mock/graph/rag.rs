//! GraphRAG retrieval over `articles.embedding`.
//!
//! The corpus is three articles with fixed similarity to the question the
//! design asks. Expansion is real: it walks the chosen binding from what
//! each article mentions, as many hops as the request says, up to its node
//! cap.

use crate::data::mock::graph::store::StoredGraph;
use crate::models::graph::{GraphPath, GraphResult, NodeKind, Relation};
use crate::models::graph_rag::{
    EmbeddingModel, Evidence, Provenance, RagRequest, RagResult, RagSetup, RagSource,
};
use crate::services::error::StudioError;

const INDEX: &str = "idx_articles_embedding";
const DIMENSION: u32 = 768;

pub fn rag_setup() -> RagSetup {
    RagSetup {
        sources: vec![RagSource {
            collection: "articles".to_string(),
            field: "embedding".to_string(),
            index: INDEX.to_string(),
            dimension: DIMENSION,
            metric: "cosine".to_string(),
        }],
        models: vec![
            EmbeddingModel {
                name: "support-embed-v2".to_string(),
                dimension: 768,
            },
            EmbeddingModel {
                name: "support-embed-v1".to_string(),
                dimension: 384,
            },
        ],
        bindings: vec!["commerce_graph".to_string(), "social_graph".to_string()],
        // No provider is configured: retrieval works, answers do not.
        provider: None,
    }
}

/// Article id and similarity to the question, best first.
const HITS: [(&str, f32); 3] = [("a10023", 0.82), ("a10456", 0.76), ("a09871", 0.71)];

/// Nodes within `hops` of `seeds`, following edges either way, nearest
/// first. Edges between reached nodes come with them.
fn neighborhood(graph: &StoredGraph, seeds: &[String], hops: usize, cap: usize) -> GraphResult {
    let mut out = GraphResult::default();
    let mut frontier: Vec<String> = Vec::new();
    for s in seeds {
        if let Some(n) = graph.node(s)
            && out.node(s).is_none()
        {
            out.nodes.push(n.clone());
            frontier.push(s.clone());
        }
    }
    for _ in 0..hops {
        let mut next = Vec::new();
        for at in &frontier {
            for e in graph.edges.iter().filter(|e| &e.from == at || &e.to == at) {
                let other = if &e.from == at { &e.to } else { &e.from };
                if out.node(other).is_some() {
                    continue;
                }
                // An article is where retrieval began, never a stop on the way.
                if graph
                    .node(other)
                    .is_some_and(|n| n.kind == NodeKind::Article)
                {
                    continue;
                }
                if let Some(n) = graph.node(other)
                    && out.nodes.len() < cap
                {
                    out.nodes.push(n.clone());
                    next.push(other.clone());
                }
            }
        }
        frontier = next;
    }
    out.edges = graph
        .edges
        .iter()
        .filter(|e| out.node(&e.from).is_some() && out.node(&e.to).is_some())
        .cloned()
        .collect();
    out
}

/// A node the breadth-first walk reached: how far, and from which node over
/// which edge.
#[derive(Clone)]
struct Reached {
    path: String,
    dist: usize,
    via: Option<(String, String)>,
}

/// The walk from the node farthest from `seed` back to it, over `graph`.
/// Other seeds and articles are not candidates: the walk shows who reached
/// the product, not which products sit near each other.
fn path_from_far(graph: &GraphResult, seed: &str, exclude: &[String]) -> Option<GraphPath> {
    let walkable = |path: &str| {
        graph
            .node(path)
            .is_some_and(|n| n.kind != NodeKind::Article)
    };
    // Breadth-first from the seed: each node's distance, and the edge and
    // node it was reached from.
    let mut reached = vec![Reached {
        path: seed.to_string(),
        dist: 0,
        via: None,
    }];
    let mut at = 0;
    while at < reached.len() {
        let (here, dist) = (reached[at].path.clone(), reached[at].dist);
        for e in graph.edges_of(&here) {
            let next = if e.from == here { &e.to } else { &e.from };
            if !walkable(next) || reached.iter().any(|r| &r.path == next) {
                continue;
            }
            reached.push(Reached {
                path: next.clone(),
                dist: dist + 1,
                via: Some((here.clone(), e.id.clone())),
            });
        }
        at += 1;
    }
    // The farthest candidate; the earliest wins a tie.
    let far = reached
        .iter()
        .filter(|r| r.dist > 0 && !exclude.contains(&r.path))
        .fold(None::<&Reached>, |best, r| match best {
            Some(b) if b.dist >= r.dist => Some(b),
            _ => Some(r),
        })?;
    // Follow the parents from `far` back to the seed.
    let mut path = GraphPath {
        nodes: vec![far.path.clone()],
        edges: Vec::new(),
    };
    let mut cur = far;
    while let Some((parent, edge)) = &cur.via {
        path.edges.push(edge.clone());
        path.nodes.push(parent.clone());
        cur = reached.iter().find(|r| &r.path == parent)?;
    }
    Some(path)
}

pub fn search(graph: &StoredGraph, request: &RagRequest) -> Result<RagResult, StudioError> {
    let setup = rag_setup();
    let source = setup
        .sources
        .iter()
        .find(|s| s.collection == request.collection && s.field == request.field);
    let issues = request.issues(source.map(|s| s.dimension));
    if !issues.is_empty() {
        return Err(StudioError::Rejected {
            reason: issues.join(" "),
        });
    }
    let source = source.ok_or_else(|| StudioError::UnknownObject {
        kind: "vector index",
        id: request.index.clone(),
    })?;

    let mut seeds_all: Vec<String> = Vec::new();
    let mut evidence = Vec::new();
    for (rank, (id, score)) in HITS.iter().enumerate() {
        let record = format!("articles/{id}");
        let seeds: Vec<String> = graph
            .edges
            .iter()
            .filter(|e| e.relation == Relation::Mentions && e.from == record)
            .map(|e| e.to.clone())
            .collect();
        seeds_all.extend(seeds.iter().cloned());
        let (title, excerpt) = graph
            .node(&record)
            .map(|n| {
                let text = |f: &str| {
                    n.property(f)
                        .and_then(|v| match v {
                            nodedb_types::Value::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default()
                };
                (text("title"), text("excerpt"))
            })
            .unwrap_or_default();
        evidence.push(Evidence {
            rank: rank as u32 + 1,
            record,
            title,
            score: *score,
            excerpt,
            provenance: Provenance {
                index: source.index.clone(),
                metric: source.metric.clone(),
                seeds,
                binding: request.binding.clone(),
                hops: request.hops,
            },
            path: None,
        });
    }

    let mut context = neighborhood(
        graph,
        &seeds_all,
        usize::from(request.hops),
        request.max_nodes as usize,
    );
    for ev in &mut evidence {
        // The article joins the context with the mentions that link it.
        if !ev.provenance.seeds.is_empty()
            && let Some(article) = graph.node(&ev.record)
            && context.node(&ev.record).is_none()
            && context.nodes.len() < request.max_nodes as usize
        {
            context.nodes.push(article.clone());
            let links: Vec<_> = graph
                .edges
                .iter()
                .filter(|e| e.from == ev.record && context.node(&e.to).is_some())
                .cloned()
                .collect();
            context.edges.extend(links);
        }
    }
    for ev in &mut evidence {
        ev.path = ev
            .provenance
            .seeds
            .first()
            .and_then(|seed| path_from_far(&context, seed, &seeds_all));
    }
    Ok(RagResult {
        evidence,
        graph: context,
    })
}

pub fn rag_search(request: &RagRequest) -> Result<RagResult, StudioError> {
    super::search(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RagRequest {
        RagRequest {
            question: "Which customers bought related products?".into(),
            collection: "articles".into(),
            field: "embedding".into(),
            index: INDEX.into(),
            binding: "commerce_graph".into(),
            model: "support-embed-v2".into(),
            dimension: 768,
            hops: 2,
            max_nodes: 50,
        }
    }

    #[test]
    fn the_design_retrieval_finds_three_articles_and_the_customer_path() {
        let r = super::super::search(&request()).expect("searches");
        let ids: Vec<&str> = r.evidence.iter().map(|e| e.record.as_str()).collect();
        assert_eq!(
            ids,
            ["articles/a10023", "articles/a10456", "articles/a09871"]
        );
        assert_eq!(r.evidence[0].score, 0.82);
        let path = r.evidence[0].path.as_ref().expect("path");
        assert_eq!(path.nodes, ["users/u44182", "orders/o104", "products/p12"]);
        assert_eq!(path.hops(), 2);
        // The third article mentions no product, so expansion has no seed.
        assert!(r.evidence[2].path.is_none());
        assert!(r.evidence[2].provenance.seeds.is_empty());
    }

    #[test]
    fn the_context_holds_the_two_linked_products_their_order_and_customer() {
        let r = super::super::search(&request()).expect("searches");
        let mut paths: Vec<&str> = r.graph.nodes.iter().map(|n| n.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            [
                "articles/a10023",
                "articles/a10456",
                "orders/o104",
                "products/p12",
                "products/p77",
                "users/u44182"
            ]
        );
        assert!(
            r.graph
                .edges
                .iter()
                .all(|e| { r.graph.node(&e.from).is_some() && r.graph.node(&e.to).is_some() })
        );
    }

    #[test]
    fn fewer_hops_reach_fewer_nodes() {
        let one = super::super::search(&RagRequest {
            hops: 1,
            ..request()
        })
        .expect("searches");
        assert!(one.graph.node("users/u44182").is_none());
        assert!(one.graph.node("orders/o104").is_some());
        let path = one.evidence[0].path.as_ref().expect("path");
        assert_eq!(path.nodes, ["orders/o104", "products/p12"]);
    }

    #[test]
    fn the_node_cap_bounds_the_context() {
        let r = super::super::search(&RagRequest {
            max_nodes: 3,
            ..request()
        })
        .expect("searches");
        assert!(r.graph.nodes.len() <= 3);
    }

    #[test]
    fn a_request_the_form_refuses_is_rejected() {
        let wrong = RagRequest {
            model: "support-embed-v1".into(),
            dimension: 384,
            ..request()
        };
        assert!(matches!(
            super::super::search(&wrong),
            Err(StudioError::Rejected { .. })
        ));
        let blank = RagRequest {
            question: String::new(),
            ..request()
        };
        assert!(super::super::search(&blank).is_err());
    }
}
