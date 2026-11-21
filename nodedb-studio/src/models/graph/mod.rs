//! Graph Explorer data: nodes, edges, and the bindings that overlay them.
//!
//! A graph binding is an overlay on collections, not a storage engine. Every
//! node is a record, named by its collection and id (`users/u44182`), and
//! every edge joins two records. These types carry what the canvas and the
//! inspector show. A result is the visible part of a graph: whole-graph
//! totals belong to the binding, never to the result.

mod base;
pub mod builder;
pub mod cypher;
pub mod cypher_lex;
pub mod layout;
pub mod rag;
pub mod records;
pub mod scene;
pub use base::*;
