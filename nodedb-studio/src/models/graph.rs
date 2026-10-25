//! Graph Explorer data: nodes, edges, and the bindings that overlay them.
//!
//! A graph binding is an overlay on collections, not a storage engine. Every
//! node is a record, named by its collection and id (`users/u44182`), and
//! every edge joins two records. These types carry what the canvas and the
//! inspector show. A result is the visible part of a graph: whole-graph
//! totals belong to the binding, never to the result.

use nodedb_types::Value;

/// What a node is. The kind picks the node's shape, glyph, and color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeKind {
    User,
    Post,
    Topic,
    Order,
    Product,
    Article,
}

impl NodeKind {
    pub const ALL: [NodeKind; 6] = [
        NodeKind::User,
        NodeKind::Post,
        NodeKind::Topic,
        NodeKind::Order,
        NodeKind::Product,
        NodeKind::Article,
    ];

    /// The Cypher label and the legend text.
    pub fn label(self) -> &'static str {
        match self {
            NodeKind::User => "User",
            NodeKind::Post => "Post",
            NodeKind::Topic => "Topic",
            NodeKind::Order => "Order",
            NodeKind::Product => "Product",
            NodeKind::Article => "Article",
        }
    }

    /// The collection whose records are this kind of node.
    pub fn collection(self) -> &'static str {
        match self {
            NodeKind::User => "users",
            NodeKind::Post => "posts",
            NodeKind::Topic => "topics",
            NodeKind::Order => "orders",
            NodeKind::Product => "products",
            NodeKind::Article => "articles",
        }
    }

    /// A Cypher label names a kind by its label or its collection, in any case.
    pub fn from_label(label: &str) -> Option<NodeKind> {
        NodeKind::ALL.into_iter().find(|k| {
            k.label().eq_ignore_ascii_case(label) || k.collection().eq_ignore_ascii_case(label)
        })
    }

    /// The label a generated query writes: the collection, capitalized.
    pub fn query_label(self) -> String {
        let name = self.collection();
        let mut chars = name.chars();
        chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    }
}

/// A relationship type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Relation {
    Posted,
    Tagged,
    Follows,
    Purchased,
    Contains,
    Mentions,
}

impl Relation {
    pub const ALL: [Relation; 6] = [
        Relation::Posted,
        Relation::Tagged,
        Relation::Follows,
        Relation::Purchased,
        Relation::Contains,
        Relation::Mentions,
    ];

    /// The Cypher type and the edge label.
    pub fn label(self) -> &'static str {
        match self {
            Relation::Posted => "POSTED",
            Relation::Tagged => "TAGGED",
            Relation::Follows => "FOLLOWS",
            Relation::Purchased => "PURCHASED",
            Relation::Contains => "CONTAINS",
            Relation::Mentions => "MENTIONS",
        }
    }

    pub fn from_label(label: &str) -> Option<Relation> {
        Relation::ALL
            .into_iter()
            .find(|r| r.label().eq_ignore_ascii_case(label))
    }

    /// The kinds an edge of this type joins, source first.
    pub fn ends(self) -> (NodeKind, NodeKind) {
        match self {
            Relation::Posted => (NodeKind::User, NodeKind::Post),
            Relation::Tagged => (NodeKind::Post, NodeKind::Topic),
            Relation::Follows => (NodeKind::User, NodeKind::User),
            Relation::Purchased => (NodeKind::User, NodeKind::Order),
            Relation::Contains => (NodeKind::Order, NodeKind::Product),
            Relation::Mentions => (NodeKind::Article, NodeKind::Product),
        }
    }
}

/// How many edges leave and enter a node in the whole graph. This differs
/// from its degree inside a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Degree {
    pub out: u64,
    pub inbound: u64,
}

/// A property value as one line of text. Times read in UTC, as the
/// footer says.
pub fn value_text(value: &Value) -> String {
    use crate::models::format::date_second;
    match value {
        Value::String(s) => s.clone(),
        Value::DateTime(t) | Value::NaiveDateTime(t) => date_second(t.unix_secs(), 0),
        other => crate::models::record_json::compact(other, &[]),
    }
}

/// `users/u44182`.
pub fn node_path(collection: &str, id: &str) -> String {
    format!("{collection}/{id}")
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    /// The record: `collection/id`. Unique in a graph.
    pub path: String,
    pub kind: NodeKind,
    /// The record's display name, when it has one.
    pub name: Option<String>,
    pub properties: Vec<(String, Value)>,
    pub degree: Degree,
}

impl GraphNode {
    pub fn collection(&self) -> &str {
        self.path.split_once('/').map_or("", |(c, _)| c)
    }

    pub fn id(&self) -> &str {
        self.path.split_once('/').map_or(&self.path, |(_, id)| id)
    }

    /// The name, else the id.
    pub fn title(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.id().to_string())
    }

    pub fn property(&self, field: &str) -> Option<&Value> {
        self.properties
            .iter()
            .find(|(k, _)| k == field)
            .map(|(_, v)| v)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphEdge {
    pub id: String,
    pub relation: Relation,
    /// The source node's path.
    pub from: String,
    /// The target node's path.
    pub to: String,
    pub properties: Vec<(String, Value)>,
}

/// The visible part of a graph: what a query, an expansion, or a retrieval
/// returned. Every edge references a node in `nodes`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphResult {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// The server stopped at a limit: more matches exist.
    pub truncated: bool,
}

impl GraphResult {
    pub fn node(&self, path: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.path == path)
    }

    pub fn edge(&self, id: &str) -> Option<&GraphEdge> {
        self.edges.iter().find(|e| e.id == id)
    }

    /// The edges that touch `path`, in result order.
    pub fn edges_of<'a>(&'a self, path: &'a str) -> impl Iterator<Item = &'a GraphEdge> {
        self.edges
            .iter()
            .filter(move |e| e.from == path || e.to == path)
    }

    /// Add what `other` holds and this does not. Returns how many nodes and
    /// edges were new.
    pub fn merge(&mut self, other: &GraphResult) -> (usize, usize) {
        let (n0, e0) = (self.nodes.len(), self.edges.len());
        for n in &other.nodes {
            if self.node(&n.path).is_none() {
                self.nodes.push(n.clone());
            }
        }
        for e in &other.edges {
            if self.edge(&e.id).is_none() {
                self.edges.push(e.clone());
            }
        }
        (self.nodes.len() - n0, self.edges.len() - e0)
    }

    /// The kinds present, in the order they first appear.
    pub fn kinds(&self) -> Vec<NodeKind> {
        let mut out = Vec::new();
        for n in &self.nodes {
            if !out.contains(&n.kind) {
                out.push(n.kind);
            }
        }
        out
    }

    /// The kind where paths end: some edge enters a node of it and none
    /// leaves one. `None` for a result with no edges.
    pub fn sink_kind(&self) -> Option<NodeKind> {
        let kind_of = |path: &str| self.node(path).map(|n| n.kind);
        let sources: Vec<NodeKind> = self.edges.iter().filter_map(|e| kind_of(&e.from)).collect();
        self.edges
            .iter()
            .rev()
            .filter_map(|e| kind_of(&e.to))
            .find(|k| !sources.contains(k))
    }
}

impl crate::services::async_state::IsEmpty for GraphResult {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// A named graph over collections, with its whole-graph totals.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphBinding {
    pub name: String,
    pub kinds: Vec<NodeKind>,
    pub relations: Vec<Relation>,
    pub nodes: u64,
    pub edges: u64,
}

impl crate::services::async_state::IsEmpty for GraphBinding {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A walk from one node to another. `edges[i]` joins `nodes[i]` and
/// `nodes[i + 1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphPath {
    pub nodes: Vec<String>,
    pub edges: Vec<String>,
}

impl GraphPath {
    pub fn hops(&self) -> usize {
        self.edges.len()
    }
}

/// Every directed path from `from` to a node of `kind`, at most `max_hops`
/// long, shortest first. Nodes do not repeat inside a path.
pub fn paths_to_kind(
    graph: &GraphResult,
    from: &str,
    kind: NodeKind,
    max_hops: usize,
) -> Vec<GraphPath> {
    fn walk(
        graph: &GraphResult,
        kind: NodeKind,
        max_hops: usize,
        path: &mut GraphPath,
        out: &mut Vec<GraphPath>,
    ) {
        let Some(here) = path.nodes.last().cloned() else {
            return;
        };
        if path.hops() > 0 && graph.node(&here).is_some_and(|n| n.kind == kind) {
            out.push(path.clone());
            return;
        }
        if path.hops() == max_hops {
            return;
        }
        for e in graph.edges.iter().filter(|e| e.from == here) {
            if path.nodes.contains(&e.to) {
                continue;
            }
            path.nodes.push(e.to.clone());
            path.edges.push(e.id.clone());
            walk(graph, kind, max_hops, path, out);
            path.nodes.pop();
            path.edges.pop();
        }
    }
    let mut out = Vec::new();
    let mut path = GraphPath {
        nodes: vec![from.to_string()],
        edges: Vec::new(),
    };
    walk(graph, kind, max_hops, &mut path, &mut out);
    out.sort_by_key(GraphPath::hops);
    out
}

/// What a path search found: the paths, and the nodes and edges they use,
/// so the canvas can show a path the visible result did not hold.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PathSet {
    pub paths: Vec<GraphPath>,
    pub graph: GraphResult,
}

impl crate::services::async_state::IsEmpty for PathSet {
    fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str, kind: NodeKind) -> GraphNode {
        GraphNode {
            path: path.into(),
            kind,
            name: None,
            properties: Vec::new(),
            degree: Degree::default(),
        }
    }

    fn edge(id: &str, from: &str, to: &str) -> GraphEdge {
        GraphEdge {
            id: id.into(),
            relation: Relation::Posted,
            from: from.into(),
            to: to.into(),
            properties: Vec::new(),
        }
    }

    fn chain() -> GraphResult {
        GraphResult {
            nodes: vec![
                node("users/u1", NodeKind::User),
                node("posts/p1", NodeKind::Post),
                node("posts/p2", NodeKind::Post),
                node("topics/t", NodeKind::Topic),
            ],
            edges: vec![
                edge("a", "users/u1", "posts/p1"),
                edge("b", "users/u1", "posts/p2"),
                edge("c", "posts/p1", "topics/t"),
                edge("d", "posts/p2", "topics/t"),
            ],
            truncated: false,
        }
    }

    #[test]
    fn labels_resolve_by_kind_or_collection_in_any_case() {
        assert_eq!(NodeKind::from_label("User"), Some(NodeKind::User));
        assert_eq!(NodeKind::from_label("users"), Some(NodeKind::User));
        assert_eq!(NodeKind::from_label("ORDERS"), Some(NodeKind::Order));
        assert_eq!(NodeKind::from_label("Nope"), None);
        assert_eq!(NodeKind::Order.query_label(), "Orders");
    }

    #[test]
    fn every_relation_joins_two_known_kinds_and_round_trips() {
        for r in Relation::ALL {
            assert_eq!(Relation::from_label(r.label()), Some(r));
        }
        assert_eq!(
            Relation::Purchased.ends(),
            (NodeKind::User, NodeKind::Order)
        );
    }

    #[test]
    fn a_path_names_its_record() {
        let n = node("users/u44182", NodeKind::User);
        assert_eq!((n.collection(), n.id()), ("users", "u44182"));
        assert_eq!(n.title(), "u44182");
    }

    #[test]
    fn paths_to_a_kind_are_directed_and_shortest_first() {
        let g = chain();
        let paths = paths_to_kind(&g, "users/u1", NodeKind::Topic, 3);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].nodes, ["users/u1", "posts/p1", "topics/t"]);
        assert_eq!(paths[0].edges, ["a", "c"]);
        // Edges are directed: nothing leaves the topic.
        assert!(paths_to_kind(&g, "topics/t", NodeKind::User, 3).is_empty());
        assert!(paths_to_kind(&g, "users/u1", NodeKind::Topic, 1).is_empty());
    }

    #[test]
    fn the_sink_kind_is_where_paths_end() {
        assert_eq!(chain().sink_kind(), Some(NodeKind::Topic));
        assert_eq!(GraphResult::default().sink_kind(), None);
    }

    #[test]
    fn merge_counts_only_new_items() {
        let mut g = chain();
        let mut more = chain();
        more.nodes.push(node("users/u2", NodeKind::User));
        more.edges.push(edge("e", "users/u2", "posts/p1"));
        assert_eq!(g.merge(&more), (1, 1));
        assert_eq!(g.merge(&more), (0, 0));
    }
}
