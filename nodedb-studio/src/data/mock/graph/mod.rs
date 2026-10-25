//! Graph fixtures: two bindings over the Explorer's records, a Cypher
//! evaluator, and the GraphRAG corpus. Record ids and values come from the
//! Explorer fixtures (`users/u44182`, `orders/o104`), so a node opened from
//! here is the record the Explorer shows.

mod commerce;
mod eval;
mod query;
mod rag;
mod social;
mod store;

pub use query::{expand_node, find_paths, graph_bindings, run_cypher, search};
pub use rag::{rag_search, rag_setup};
