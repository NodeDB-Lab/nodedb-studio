//! The stored graphs, and the constructors their fixtures share.

use nodedb_types::Value;

use crate::data::mock::explorer::build::{dec, s, ts};
use crate::models::graph::{
    Degree, GraphBinding, GraphEdge, GraphNode, GraphResult, NodeKind, Relation, node_path,
};

/// A graph binding with every node and edge it holds. The binding's totals
/// describe the real graph. The nodes here are the part the fixtures
/// author, enough to browse, expand, and search.
#[derive(Debug, Clone)]
pub struct StoredGraph {
    pub binding: GraphBinding,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl StoredGraph {
    pub fn node(&self, path: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.path == path)
    }

    /// Every authored node and edge as one result.
    pub fn whole(&self) -> GraphResult {
        GraphResult {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            truncated: false,
        }
    }
}

/// A node. `props` are display values: text through `s`, decimals through
/// `dec`, times through `ts`.
pub fn node(
    kind: NodeKind,
    id: &str,
    name: Option<&str>,
    props: Vec<(&str, Value)>,
    (out, inbound): (u64, u64),
) -> GraphNode {
    GraphNode {
        path: node_path(kind.collection(), id),
        kind,
        name: name.map(str::to_string),
        properties: props.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        degree: Degree { out, inbound },
    }
}

/// An edge between two records, with no properties.
pub fn edge(id: &str, relation: Relation, from: &GraphNode, to: &GraphNode) -> GraphEdge {
    GraphEdge {
        id: id.to_string(),
        relation,
        from: from.path.clone(),
        to: to.path.clone(),
        properties: Vec::new(),
    }
}

/// An edge that carries `created_at`, 2026, month/day/hour/minute/second.
pub fn edge_at(
    id: &str,
    relation: Relation,
    from: &GraphNode,
    to: &GraphNode,
    (mo, d, h, mi, sec): (u32, u32, u32, u32, u32),
) -> GraphEdge {
    GraphEdge {
        properties: vec![("created_at".to_string(), ts(2026, mo, d, h, mi, sec))],
        ..edge(id, relation, from, to)
    }
}

/// The people both graphs share, as the Explorer's `users` holds them.
/// `country` is the graph's own property.
pub fn user(id: &str, name: &str, country: &str, degree: (u64, u64)) -> GraphNode {
    let email = format!(
        "{}@example.com",
        name.split(' ').next().unwrap_or(id).to_lowercase()
    );
    node(
        NodeKind::User,
        id,
        Some(name),
        vec![
            ("name", s(name)),
            ("email", s(&email)),
            ("status", s("active")),
            ("country", s(country)),
            ("created_at", ts(2024, 11, 3, 8, 24, 17)),
        ],
        degree,
    )
}

pub fn order(id: &str, total: &str, status: &str, at: (u32, u32, u32, u32, u32)) -> GraphNode {
    node(
        NodeKind::Order,
        id,
        None,
        vec![
            ("total", dec(total)),
            ("currency", s("USD")),
            ("status", s(status)),
            ("created_at", ts(2026, at.0, at.1, at.2, at.3, at.4)),
        ],
        (2, 1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_user_node_reads_like_the_explorer_record() {
        let n = user("u44182", "Hana Idris", "MY", (412, 88));
        assert_eq!(n.path, "users/u44182");
        assert_eq!(n.property("email"), Some(&s("hana@example.com")));
        assert_eq!((n.degree.out, n.degree.inbound), (412, 88));
    }
}
