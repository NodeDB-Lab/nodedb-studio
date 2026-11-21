//! Edges of a collection's graph, as the Explorer lists them.
//!
//! A graph is an overlay on records: an edge joins two records, each named
//! by its collection and id. Studio reads edges here to browse and edit
//! source data. Traversal and queries belong to Graph Explorer.

use nodedb_types::Value;

/// One end of an edge: a record in a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRef {
    pub collection: String,
    pub id: String,
    /// The node's type label, `User` or `Order`.
    pub kind: String,
}

impl NodeRef {
    /// `users/u44182`.
    pub fn path(&self) -> String {
        format!("{}/{}", self.collection, self.id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub id: String,
    pub relation: String,
    pub from: NodeRef,
    pub to: NodeRef,
    /// A `Value::Object` of the edge's properties.
    pub properties: Value,
}

/// What one read of a graph's edges asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeQuery {
    pub relation: Option<String>,
    /// Matches an edge id or either endpoint, ignoring case.
    pub text: String,
    /// Zero-based.
    pub page: usize,
    pub page_size: usize,
}

impl EdgeQuery {
    pub fn new(page_size: usize) -> Self {
        Self {
            relation: None,
            text: String::new(),
            page: 0,
            page_size,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgePage {
    pub edges: Vec<Edge>,
    pub total: u64,
    /// Every relation the graph holds, for the filter.
    pub relations: Vec<String>,
}
