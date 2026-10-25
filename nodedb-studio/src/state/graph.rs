//! Graph Explorer UI state, one struct per tab, provided by `App` above the
//! router. A tab keeps its query, form, selection, and canvas when the user
//! leaves the page.
//!
//! Graph-node layout and zoom belong to the canvas. They never touch the
//! workspace's panel placement.

use dioxus::prelude::*;

use crate::models::graph::{GraphPath, GraphResult, NodeKind, paths_to_kind};
use crate::models::graph_builder::BuilderForm;
use crate::models::graph_layout::LayoutKind;
use crate::models::graph_rag::{RagRequest, RagSetup, RagSource};

/// The query the Cypher tab opens with.
pub const DEFAULT_CYPHER: &str = "MATCH (u:User)-[:POSTED]->(p:Post)\n      -[:TAGGED]->(t:Topic {name: \"databases\"})\nWHERE u.country = \"MY\"\nRETURN u, p, t\nLIMIT 50";

/// The binding each tab opens on.
pub const SOCIAL: &str = "social_graph";
pub const COMMERCE: &str = "commerce_graph";

/// How far Find paths and the inspector look.
pub const PATH_HOPS: usize = 3;
/// Neighbors one Expand adds at most.
pub const EXPAND_LIMIT: usize = 25;

/// What is selected on a canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    Node(String),
    Edge(String),
}

/// One run: its text, binding, and cap. `seq` makes a repeat run a new
/// request, so pressing Run again re-reads.
#[derive(Debug, Clone, PartialEq)]
pub struct RunRequest {
    pub seq: u32,
    pub binding: String,
    pub query: String,
    pub limit: usize,
}

/// One line that says what the last Expand or Find paths did.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub error: bool,
    pub text: String,
}

/// The canvas under one tab: what is picked, what Expand and Find paths
/// added, and how far it is zoomed.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    pub pick: Option<Pick>,
    /// Nodes and edges Expand and Find paths merged into the result.
    pub added: GraphResult,
    /// The paths the inspector lists, and which one the canvas highlights.
    pub paths: Vec<GraphPath>,
    pub path_ix: usize,
    /// Set by Find paths: the paths come from the server, not the result.
    pub searched: bool,
    pub zoom: u16,
    pub notice: Option<Notice>,
    pub busy: bool,
}

pub const ZOOM_STEP: u16 = 25;
pub const ZOOM_MIN: u16 = 50;
pub const ZOOM_MAX: u16 = 300;

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            pick: None,
            added: GraphResult::default(),
            paths: Vec::new(),
            path_ix: 0,
            searched: false,
            zoom: 100,
            notice: None,
            busy: false,
        }
    }
}

impl CanvasState {
    /// A new run starts the canvas over.
    pub fn reset(&mut self) {
        *self = Self {
            zoom: self.zoom,
            ..Self::default()
        };
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom + ZOOM_STEP).min(ZOOM_MAX);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = self.zoom.saturating_sub(ZOOM_STEP).max(ZOOM_MIN);
    }

    pub fn fit(&mut self) {
        self.zoom = 100;
    }

    /// Select a node or an edge. The path list belongs to the old pick.
    pub fn select(&mut self, pick: Pick) {
        if self.pick.as_ref() != Some(&pick) {
            self.paths.clear();
            self.path_ix = 0;
            self.searched = false;
            self.notice = None;
        }
        self.pick = Some(pick);
    }

    /// Merge what a read found into the canvas. Returns how many nodes and
    /// edges the visible graph did not hold. `result` is the run's result.
    pub fn absorb(&mut self, result: &GraphResult, found: &GraphResult) -> (usize, usize) {
        let mut visible = self.visible(result);
        let fresh = visible.merge(found);
        self.added.merge(found);
        fresh
    }

    /// The result and everything merged into it.
    pub fn visible(&self, result: &GraphResult) -> GraphResult {
        let mut all = result.clone();
        all.merge(&self.added);
        all
    }

    /// The pick, else the first node: a result always has a selection.
    pub fn effective_pick(&self, graph: &GraphResult) -> Option<Pick> {
        match &self.pick {
            Some(Pick::Node(p)) if graph.node(p).is_some() => self.pick.clone(),
            Some(Pick::Edge(e)) if graph.edge(e).is_some() => self.pick.clone(),
            _ => graph.nodes.first().map(|n| Pick::Node(n.path.clone())),
        }
    }

    /// The paths to show for node `path`: a search's, else the ones the
    /// visible graph holds to the kind where the run's own paths end.
    /// `result` is the run's result, before any expansion.
    pub fn paths_for(&self, result: &GraphResult, path: &str) -> Vec<GraphPath> {
        if self.searched {
            return self.paths.clone();
        }
        let visible = self.visible(result);
        match target_kind(result, &visible, path) {
            Some(kind) => paths_to_kind(&visible, path, kind, PATH_HOPS),
            None => Vec::new(),
        }
    }

    /// The path the canvas highlights.
    pub fn highlighted(&self, result: &GraphResult, path: &str) -> Option<GraphPath> {
        let all = self.paths_for(result, path);
        let ix = self.path_ix.min(all.len().saturating_sub(1));
        all.into_iter().nth(ix)
    }
}

/// The kind a path from `path` heads for: where the run's paths end. The
/// run's result fixes it, so expanding a node never moves the target.
/// `None` when `path` is already of that kind, or the run has no edges.
pub fn target_kind(result: &GraphResult, visible: &GraphResult, path: &str) -> Option<NodeKind> {
    let kind = result.sink_kind()?;
    visible.node(path).filter(|n| n.kind != kind).map(|_| kind)
}

#[derive(Debug, Clone, PartialEq)]
pub struct CypherUi {
    pub text: String,
    pub binding: String,
    pub limit: usize,
    pub layout: LayoutKind,
    /// The run on screen. The tab opens with the design query already run.
    pub request: RunRequest,
}

impl Default for CypherUi {
    fn default() -> Self {
        Self {
            text: DEFAULT_CYPHER.to_string(),
            binding: SOCIAL.to_string(),
            limit: 50,
            layout: LayoutKind::Force,
            request: RunRequest {
                seq: 0,
                binding: SOCIAL.to_string(),
                query: DEFAULT_CYPHER.to_string(),
                limit: 50,
            },
        }
    }
}

impl CypherUi {
    /// Run what the editor holds. The caller resets the canvas.
    pub fn run(&mut self) {
        self.request = RunRequest {
            seq: self.request.seq + 1,
            binding: self.binding.clone(),
            query: self.text.clone(),
            limit: self.limit,
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuilderUi {
    pub form: BuilderForm,
    pub binding: String,
    pub layout: LayoutKind,
    pub request: Option<RunRequest>,
}

impl Default for BuilderUi {
    fn default() -> Self {
        let form = BuilderForm::default();
        let request = form.cypher().ok().map(|query| RunRequest {
            seq: 0,
            binding: COMMERCE.to_string(),
            query,
            limit: form.limit as usize,
        });
        Self {
            form,
            binding: COMMERCE.to_string(),
            layout: LayoutKind::Force,
            request,
        }
    }
}

impl BuilderUi {
    /// Run the form's query. Nothing runs while the form has issues.
    /// Returns whether a run started, so the caller resets the canvas.
    pub fn run(&mut self) -> bool {
        let Ok(query) = self.form.cypher() else {
            return false;
        };
        let seq = self.request.as_ref().map_or(0, |r| r.seq) + 1;
        self.request = Some(RunRequest {
            seq,
            binding: self.binding.clone(),
            query,
            limit: self.form.limit as usize,
        });
        true
    }
}

/// Which GraphRAG view is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RagTab {
    #[default]
    Retrieval,
    Answer,
}

impl RagTab {
    pub const ALL: [RagTab; 2] = [RagTab::Retrieval, RagTab::Answer];

    pub fn label(self) -> &'static str {
        match self {
            RagTab::Retrieval => "Retrieval",
            RagTab::Answer => "Answer",
        }
    }
}

/// The retrieval form. `None` fields wait for the setup read to fill them
/// with the first source, model, and binding.
#[derive(Debug, Clone, PartialEq)]
pub struct RagForm {
    pub question: String,
    pub source: Option<String>,
    pub binding: Option<String>,
    pub model: Option<String>,
    pub hops: u8,
    pub max_nodes: String,
}

impl Default for RagForm {
    fn default() -> Self {
        Self {
            question: "Which customers bought related products?".to_string(),
            source: None,
            binding: None,
            model: None,
            hops: 2,
            max_nodes: "50".to_string(),
        }
    }
}

impl RagForm {
    /// The source the form reads: the chosen one, else the first.
    pub fn source<'a>(&self, setup: &'a RagSetup) -> Option<&'a RagSource> {
        self.source
            .as_ref()
            .and_then(|l| setup.sources.iter().find(|s| &s.label() == l))
            .or_else(|| setup.sources.first())
    }

    /// The request this form asks for, over `setup`. `None` while the setup
    /// offers no source, model, or binding to fill the form with.
    pub fn request(&self, setup: &RagSetup) -> Option<RagRequest> {
        let source = self.source(setup)?;
        let model = self
            .model
            .as_ref()
            .and_then(|m| setup.models.iter().find(|x| &x.name == m))
            .or_else(|| setup.models.first())?;
        let binding = self
            .binding
            .as_ref()
            .filter(|b| setup.bindings.contains(b))
            .or_else(|| setup.bindings.first())?;
        Some(RagRequest {
            question: self.question.clone(),
            collection: source.collection.clone(),
            field: source.field.clone(),
            index: source.index.clone(),
            binding: binding.clone(),
            model: model.name.clone(),
            dimension: model.dimension,
            hops: self.hops,
            max_nodes: self.max_nodes.trim().parse().unwrap_or(0),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RagUi {
    pub form: RagForm,
    pub tab: RagTab,
    /// The search on screen, with a `seq` so a repeat is a new read.
    pub request: Option<(u32, RagRequest)>,
    /// The evidence rank the user chose.
    pub selected: Option<u32>,
    /// The opening search has run. It runs once, when the setup loads.
    pub opened: bool,
}

#[derive(Clone, Copy)]
pub struct GraphUi {
    pub cypher: Signal<CypherUi>,
    pub cypher_canvas: Signal<CanvasState>,
    pub builder: Signal<BuilderUi>,
    pub builder_canvas: Signal<CanvasState>,
    pub rag: Signal<RagUi>,
    pub rag_canvas: Signal<CanvasState>,
}

impl GraphUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            cypher: Signal::new(CypherUi::default()),
            cypher_canvas: Signal::new(CanvasState::default()),
            builder: Signal::new(BuilderUi::default()),
            builder_canvas: Signal::new(CanvasState::default()),
            rag: Signal::new(RagUi::default()),
            rag_canvas: Signal::new(CanvasState::default()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::graph::{Degree, GraphEdge, GraphNode, Relation};

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

    fn graph() -> GraphResult {
        GraphResult {
            nodes: vec![
                node("users/u1", NodeKind::User),
                node("posts/p1", NodeKind::Post),
                node("topics/t", NodeKind::Topic),
            ],
            edges: vec![
                edge("a", "users/u1", "posts/p1"),
                edge("b", "posts/p1", "topics/t"),
            ],
            truncated: false,
        }
    }

    #[test]
    fn the_first_node_is_selected_until_the_user_picks() {
        let c = CanvasState::default();
        assert_eq!(
            c.effective_pick(&graph()),
            Some(Pick::Node("users/u1".into()))
        );
        let mut c = CanvasState::default();
        c.select(Pick::Node("posts/p1".into()));
        assert_eq!(
            c.effective_pick(&graph()),
            Some(Pick::Node("posts/p1".into()))
        );
        // A pick the result no longer holds falls back.
        c.select(Pick::Node("gone/x".into()));
        assert_eq!(
            c.effective_pick(&graph()),
            Some(Pick::Node("users/u1".into()))
        );
    }

    #[test]
    fn paths_head_for_the_sink_kind_and_stop_at_it() {
        let c = CanvasState::default();
        let g = graph();
        let paths = c.paths_for(&g, "users/u1");
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].nodes, ["users/u1", "posts/p1", "topics/t"]);
        assert!(c.paths_for(&g, "topics/t").is_empty());
        assert_eq!(target_kind(&g, &g, "topics/t"), None);
        assert_eq!(target_kind(&g, &g, "users/u1"), Some(NodeKind::Topic));
    }

    #[test]
    fn expanding_does_not_move_the_target_kind() {
        let g = graph();
        let mut c = CanvasState::default();
        c.added.nodes.push(node("orders/o1", NodeKind::Order));
        c.added.edges.push(edge("x", "users/u1", "orders/o1"));
        let visible = c.visible(&g);
        assert_eq!(target_kind(&g, &visible, "users/u1"), Some(NodeKind::Topic));
        assert_eq!(c.paths_for(&g, "users/u1").len(), 1);
    }

    #[test]
    fn a_new_pick_drops_the_old_nodes_paths() {
        let mut c = CanvasState {
            paths: vec![GraphPath {
                nodes: vec!["x".into()],
                edges: Vec::new(),
            }],
            searched: true,
            ..CanvasState::default()
        };
        c.select(Pick::Node("posts/p1".into()));
        assert!(c.paths.is_empty() && !c.searched);
    }

    #[test]
    fn zoom_stays_in_bounds_and_fit_returns_to_100() {
        let mut c = CanvasState::default();
        for _ in 0..20 {
            c.zoom_in();
        }
        assert_eq!(c.zoom, ZOOM_MAX);
        for _ in 0..20 {
            c.zoom_out();
        }
        assert_eq!(c.zoom, ZOOM_MIN);
        c.fit();
        assert_eq!(c.zoom, 100);
    }

    #[test]
    fn a_new_run_is_a_new_request() {
        let mut ui = CypherUi::default();
        ui.run();
        assert_eq!(ui.request.seq, 1);
        assert_eq!(ui.request.query, DEFAULT_CYPHER);
    }

    #[test]
    fn a_reset_clears_the_canvas_but_keeps_the_zoom() {
        let mut c = CanvasState {
            zoom: 150,
            ..CanvasState::default()
        };
        c.select(Pick::Node("users/u1".into()));
        c.reset();
        assert_eq!(c.zoom, 150);
        assert!(c.pick.is_none());
    }

    #[test]
    fn the_builder_runs_only_a_valid_form() {
        let mut ui = BuilderUi::default();
        assert!(ui.request.is_some());
        ui.form.end_filter = "total >".into();
        assert!(!ui.run());
        assert_eq!(ui.request.as_ref().map(|r| r.seq), Some(0));
        ui.form.end_filter = "total > 50".into();
        assert!(ui.run());
        assert_eq!(ui.request.as_ref().map(|r| r.seq), Some(1));
    }

    #[test]
    fn absorb_counts_only_what_the_canvas_lacked() {
        let g = graph();
        let mut c = CanvasState::default();
        let mut found = graph();
        found.nodes.push(node("users/u2", NodeKind::User));
        found.edges.push(edge("z", "users/u2", "posts/p1"));
        assert_eq!(c.absorb(&g, &found), (1, 1));
        assert_eq!(c.absorb(&g, &found), (0, 0));
        assert_eq!(c.visible(&g).nodes.len(), 4);
    }

    fn setup() -> RagSetup {
        RagSetup {
            sources: vec![RagSource {
                collection: "articles".into(),
                field: "embedding".into(),
                index: "idx_articles_embedding".into(),
                dimension: 768,
                metric: "cosine".into(),
            }],
            models: vec![
                crate::models::graph_rag::EmbeddingModel {
                    name: "support-embed-v2".into(),
                    dimension: 768,
                },
                crate::models::graph_rag::EmbeddingModel {
                    name: "support-embed-v1".into(),
                    dimension: 384,
                },
            ],
            bindings: vec!["commerce_graph".into()],
            provider: None,
        }
    }

    #[test]
    fn the_default_form_asks_for_the_first_source_model_and_binding() {
        let setup = setup();
        let r = RagForm::default().request(&setup).expect("request");
        assert_eq!(
            (r.collection.as_str(), r.index.as_str()),
            ("articles", "idx_articles_embedding")
        );
        assert_eq!((r.model.as_str(), r.dimension), ("support-embed-v2", 768));
        assert_eq!(
            (r.binding.as_str(), r.hops, r.max_nodes),
            ("commerce_graph", 2, 50)
        );
        assert!(r.issues(Some(768)).is_empty());
    }

    #[test]
    fn a_model_of_another_dimension_makes_the_request_invalid() {
        let setup = setup();
        let form = RagForm {
            model: Some("support-embed-v1".into()),
            ..RagForm::default()
        };
        let r = form.request(&setup).expect("request");
        assert_eq!(r.dimension, 384);
        assert_eq!(r.issues(Some(768)).len(), 1);
        let bad_nodes = RagForm {
            max_nodes: "many".into(),
            ..RagForm::default()
        };
        assert_eq!(bad_nodes.request(&setup).map(|r| r.max_nodes), Some(0));
    }

    #[test]
    fn a_setup_with_no_source_gives_no_request() {
        let mut empty = setup();
        empty.sources.clear();
        assert!(RagForm::default().request(&empty).is_none());
    }

    #[test]
    fn added_nodes_join_the_visible_graph() {
        let mut c = CanvasState::default();
        c.added.nodes.push(node("users/u2", NodeKind::User));
        assert_eq!(c.visible(&graph()).nodes.len(), 4);
    }
}
