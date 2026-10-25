//! Graph fixtures: two bindings over the Explorer's records, a Cypher
//! evaluator, and the GraphRAG corpus. Record ids and values come from the
//! Explorer fixtures (`users/u44182`, `orders/o104`), so a node opened from
//! here is the record the Explorer shows.

mod commerce;
mod eval;
mod rag;
mod social;
mod store;

use crate::models::cypher::parse;
use crate::models::graph::{
    GraphBinding, GraphPath, GraphResult, NodeKind, PathSet, paths_to_kind,
};
use crate::models::graph_rag::{RagRequest, RagResult};
use crate::services::error::StudioError;
use store::StoredGraph;

pub use rag::{rag_search, rag_setup};

/// Paths a search returns at most.
pub const MAX_PATHS: usize = 20;

fn stored(name: &str) -> Result<StoredGraph, StudioError> {
    match name {
        social::NAME => Ok(social::graph()),
        commerce::NAME => Ok(commerce::graph()),
        other => Err(StudioError::UnknownObject {
            kind: "graph binding",
            id: other.to_string(),
        }),
    }
}

pub fn graph_bindings() -> Vec<GraphBinding> {
    vec![social::graph().binding, commerce::graph().binding]
}

pub fn run_cypher(binding: &str, query: &str, limit: usize) -> Result<GraphResult, StudioError> {
    let graph = stored(binding)?;
    let pattern = parse(query).map_err(|e| StudioError::Syntax {
        reason: e.to_string(),
    })?;
    eval::run(&graph, &pattern, limit)
}

fn missing(path: &str) -> StudioError {
    let (collection, id) = path.split_once('/').unwrap_or(("", path));
    StudioError::RecordMissing {
        collection: collection.to_string(),
        id: id.to_string(),
    }
}

/// The node, its neighbors on both sides, and the edges between, up to
/// `limit` neighbors.
pub fn expand_node(binding: &str, path: &str, limit: usize) -> Result<GraphResult, StudioError> {
    let graph = stored(binding)?;
    let center = graph.node(path).ok_or_else(|| missing(path))?;
    let mut out = GraphResult {
        nodes: vec![center.clone()],
        ..GraphResult::default()
    };
    for e in graph
        .edges
        .iter()
        .filter(|e| e.from == path || e.to == path)
    {
        let other = if e.from == path { &e.to } else { &e.from };
        if out.node(other).is_none() {
            if out.nodes.len() > limit {
                out.truncated = true;
                continue;
            }
            if let Some(n) = graph.node(other) {
                out.nodes.push(n.clone());
            }
        }
        if out.node(other).is_some() {
            out.edges.push(e.clone());
        }
    }
    Ok(out)
}

pub fn find_paths(
    binding: &str,
    from: &str,
    to: NodeKind,
    max_hops: usize,
) -> Result<PathSet, StudioError> {
    let graph = stored(binding)?;
    graph.node(from).ok_or_else(|| missing(from))?;
    let whole = graph.whole();
    let paths: Vec<GraphPath> = paths_to_kind(&whole, from, to, max_hops)
        .into_iter()
        .take(MAX_PATHS)
        .collect();
    let mut used = GraphResult::default();
    for p in &paths {
        for n in &p.nodes {
            if let Some(node) = whole.node(n)
                && used.node(n).is_none()
            {
                used.nodes.push(node.clone());
            }
        }
        for e in &p.edges {
            if let Some(edge) = whole.edge(e)
                && used.edge(e).is_none()
            {
                used.edges.push(edge.clone());
            }
        }
    }
    Ok(PathSet { paths, graph: used })
}

/// Retrieval over the commerce corpus.
pub fn search(request: &RagRequest) -> Result<RagResult, StudioError> {
    let graph = stored(&request.binding)?;
    rag::search(&graph, request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_carry_the_design_totals() {
        let b = graph_bindings();
        assert_eq!(b[0].name, "social_graph");
        assert_eq!((b[0].nodes, b[0].edges), (3_200_000, 18_400_000));
        assert_eq!(b[1].name, "commerce_graph");
    }

    #[test]
    fn every_edge_joins_two_authored_nodes_of_the_right_kinds() {
        for name in ["social_graph", "commerce_graph"] {
            let g = stored(name).expect("binding");
            let mut ids: Vec<&String> = g.edges.iter().map(|e| &e.id).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), g.edges.len(), "{name}: edge ids repeat");
            let mut paths: Vec<&String> = g.nodes.iter().map(|n| &n.path).collect();
            paths.sort();
            paths.dedup();
            assert_eq!(paths.len(), g.nodes.len(), "{name}: node paths repeat");
            for e in &g.edges {
                let (from, to) = e.relation.ends();
                assert_eq!(
                    g.node(&e.from).map(|n| n.kind),
                    Some(from),
                    "{name} {}",
                    e.id
                );
                assert_eq!(g.node(&e.to).map(|n| n.kind), Some(to), "{name} {}", e.id);
                assert!(g.binding.relations.contains(&e.relation));
            }
            assert!(g.nodes.iter().all(|n| g.binding.kinds.contains(&n.kind)));
        }
    }

    #[test]
    fn purchases_match_the_explorer_edges() {
        let g = stored("social_graph").expect("binding");
        let e = g.edges.iter().find(|e| e.id == "e_7f3a2d59").expect("edge");
        assert_eq!(
            (e.from.as_str(), e.to.as_str()),
            ("users/u44182", "orders/o104")
        );
    }

    #[test]
    fn expanding_a_node_adds_its_neighbors_and_edges() {
        let r = expand_node("social_graph", "users/u44182", 25).expect("expands");
        assert!(r.node("orders/o104").is_some());
        assert!(r.node("users/u55319").is_some());
        assert_eq!(r.edges.len(), 8);
        assert!(
            r.edges
                .iter()
                .all(|e| e.from == "users/u44182" || e.to == "users/u44182")
        );
        let few = expand_node("social_graph", "users/u44182", 2).expect("expands");
        assert!(few.truncated);
        assert!(expand_node("social_graph", "users/nobody", 5).is_err());
    }

    #[test]
    fn paths_to_a_topic_run_shortest_first() {
        let set = find_paths("social_graph", "users/u44182", NodeKind::Topic, 3).expect("paths");
        assert_eq!(
            set.paths[0].nodes,
            ["users/u44182", "posts/p8281", "topics/databases"]
        );
        assert!(set.paths.len() >= 3, "two direct and one through a follow");
        assert!(set.paths.windows(2).all(|w| w[0].hops() <= w[1].hops()));
        for p in &set.paths {
            assert!(p.nodes.iter().all(|n| set.graph.node(n).is_some()));
            assert!(p.edges.iter().all(|e| set.graph.edge(e).is_some()));
        }
    }

    #[test]
    fn an_unknown_binding_or_query_is_an_error() {
        assert!(run_cypher("nope", "MATCH (a) RETURN a", 5).is_err());
        let err = run_cypher("social_graph", "MATCH (a RETURN a", 5).expect_err("syntax");
        assert!(matches!(err, StudioError::Syntax { .. }));
    }
}
