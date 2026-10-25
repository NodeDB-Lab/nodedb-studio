//! Graph reads at the backend seam: Cypher, the Builder, expansion, path
//! search, and GraphRAG retrieval.
//!
//! A graph binding is an overlay on collections. Every read names the
//! binding it runs on, and every node is a record path such as
//! `users/u44182`, so a result opens in the Explorer without translation.

use async_trait::async_trait;

use crate::models::graph::{GraphBinding, GraphResult, NodeKind, PathSet};
use crate::models::graph_rag::{RagRequest, RagResult, RagSetup};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait GraphData {
    /// Every graph binding with its whole-graph node and edge totals.
    async fn graph_bindings(&self) -> Result<Vec<GraphBinding>, StudioError>;

    /// Run `query` on `binding`. `limit` caps the matches no matter what
    /// the query's own `LIMIT` says. The result is the subgraph the matches
    /// walk.
    async fn run_cypher(
        &self,
        binding: &str,
        query: &str,
        limit: usize,
    ) -> Result<GraphResult, StudioError>;

    /// The node at `path`, its neighbors on both sides, and the edges
    /// between them, at most `limit` neighbors.
    async fn expand_node(
        &self,
        binding: &str,
        path: &str,
        limit: usize,
    ) -> Result<GraphResult, StudioError>;

    /// Directed paths from `from` to nodes of kind `to`, at most `max_hops`
    /// long, shortest first, searched in the whole graph.
    async fn find_paths(
        &self,
        binding: &str,
        from: &str,
        to: NodeKind,
        max_hops: usize,
    ) -> Result<PathSet, StudioError>;

    /// The sources, models, and bindings a retrieval can use, and whether
    /// an answer provider is configured.
    async fn rag_setup(&self) -> Result<RagSetup, StudioError>;

    /// Retrieve evidence for a question: vector hits, then graph expansion.
    /// Never generates an answer.
    async fn rag_search(&self, request: &RagRequest) -> Result<RagResult, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    const DESIGN: &str = "MATCH (u:User)-[:POSTED]->(p:Post)-[:TAGGED]->(t:Topic {name: \"databases\"})\nWHERE u.country = \"MY\"\nRETURN u, p, t\nLIMIT 50";

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

    #[tokio::test]
    async fn binding_names_are_unique() {
        let svc = MockConnectionService::ready();
        let mut names: Vec<_> = svc
            .graph_bindings()
            .await
            .expect("bindings")
            .into_iter()
            .map(|b| b.name)
            .collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 2);
    }

    #[tokio::test]
    async fn a_query_returns_the_design_subgraph() {
        let svc = MockConnectionService::ready();
        let r = svc
            .run_cypher("social_graph", DESIGN, 50)
            .await
            .expect("runs");
        assert_eq!((r.nodes.len(), r.edges.len()), (8, 8));
    }

    #[tokio::test]
    async fn every_result_edge_references_a_result_node() {
        let svc = MockConnectionService::ready();
        let r = svc
            .run_cypher("social_graph", DESIGN, 50)
            .await
            .expect("runs");
        assert!(
            r.edges
                .iter()
                .all(|e| r.node(&e.from).is_some() && r.node(&e.to).is_some())
        );
        let rag = svc.rag_search(&request()).await.expect("searches");
        assert!(
            rag.graph
                .edges
                .iter()
                .all(|e| rag.graph.node(&e.from).is_some() && rag.graph.node(&e.to).is_some())
        );
    }

    #[tokio::test]
    async fn a_query_with_no_match_is_the_empty_state() {
        let svc = MockConnectionService::ready();
        let none = "MATCH (u:User {name: 'Nobody'}) RETURN u";
        let state = AsyncState::from_value(Some(svc.run_cypher("social_graph", none, 50).await));
        assert!(state.is_empty());
    }

    #[tokio::test]
    async fn the_empty_behaviour_returns_an_empty_graph_and_no_evidence() {
        let svc = MockConnectionService::empty();
        let r = svc
            .run_cypher("social_graph", DESIGN, 50)
            .await
            .expect("ok");
        assert!(r.nodes.is_empty());
        let state = AsyncState::from_value(Some(svc.rag_search(&request()).await));
        assert!(state.is_empty());
    }

    #[tokio::test]
    async fn the_erroring_behaviour_fails_every_read() {
        let svc = MockConnectionService::erroring();
        assert!(svc.graph_bindings().await.is_err());
        assert!(svc.run_cypher("social_graph", DESIGN, 50).await.is_err());
        assert!(
            svc.expand_node("social_graph", "users/u44182", 25)
                .await
                .is_err()
        );
        assert!(
            svc.find_paths("social_graph", "users/u44182", NodeKind::Topic, 3)
                .await
                .is_err()
        );
        assert!(svc.rag_setup().await.is_err());
        assert!(svc.rag_search(&request()).await.is_err());
    }

    #[tokio::test]
    async fn the_setup_has_no_answer_provider() {
        let svc = MockConnectionService::ready();
        let setup = svc.rag_setup().await.expect("setup");
        assert!(setup.provider.is_none());
        assert_eq!(setup.sources[0].index, "idx_articles_embedding");
    }

    #[tokio::test]
    async fn a_syntax_error_reaches_the_caller_typed() {
        let svc = MockConnectionService::ready();
        let err = svc
            .run_cypher("social_graph", "MATCH (u RETURN u", 50)
            .await
            .expect_err("syntax");
        assert!(matches!(err, StudioError::Syntax { .. }));
    }
}
