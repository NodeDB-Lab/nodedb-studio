//! Personal workspace layout: sidebar state, panel visibility, and panel sizes.
//!
//! This is Studio-side state, never database state. It persists per user on
//! disk (see `state::persist`), separately from every database object.
//!
//! A panel is named by its page (`Surface`) and a stable id from the page's
//! `PanelSpec` list. Hiding a panel keeps its data: the panel stays mounted
//! and only its visibility changes, so filters and selections survive. Its
//! size is kept too, so showing it again restores its last size.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::routes::tabs::{
    AdminTab, ClusterTab, EventsTab, GraphTab, SearchTab, SpatialTab, SyncTab, TimeseriesTab,
    VectorTab,
};

/// One page whose panels the View menu lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    /// The Connection Manager. It is not a route.
    Connections,
    Explorer,
    DataTransfer,
    Query,
    Designer,
    Events(EventsTab),
    Graph(GraphTab),
    Vector(VectorTab),
    Timeseries(TimeseriesTab),
    Spatial(SpatialTab),
    Search(SearchTab),
    Cluster(ClusterTab),
    Sync(SyncTab),
    Admin(AdminTab),
    Console,
}

impl Surface {
    /// Stable storage key, e.g. `cluster/nodes`.
    pub fn key(self) -> String {
        match self {
            Surface::Connections => "connections".to_string(),
            Surface::Explorer => "explorer".to_string(),
            Surface::DataTransfer => "explorer/transfer".to_string(),
            Surface::Query => "query".to_string(),
            Surface::Designer => "designer".to_string(),
            Surface::Events(t) => format!("events/{t}"),
            Surface::Graph(t) => format!("graph/{t}"),
            Surface::Vector(t) => format!("vector/{t}"),
            Surface::Timeseries(t) => format!("timeseries/{t}"),
            Surface::Spatial(t) => format!("spatial/{t}"),
            Surface::Search(t) => format!("search/{t}"),
            Surface::Cluster(t) => format!("cluster/{t}"),
            Surface::Sync(t) => format!("sync/{t}"),
            Surface::Admin(t) => format!("admin/{t}"),
            Surface::Console => "console".to_string(),
        }
    }
}

/// A closable panel of one page, as the View menu lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelSpec {
    /// Stable id, unique within the page.
    pub id: &'static str,
    /// The View menu label, e.g. "Record details".
    pub label: &'static str,
    /// One line under the label, e.g. "Show inspector panel".
    pub hint: &'static str,
}

fn panel_key(surface: Surface, id: &str) -> String {
    format!("{}:{id}", surface.key())
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(default)]
    nav_hidden: bool,
    #[serde(default)]
    nav_collapsed: bool,
    /// Panel keys the user hid. Every panel is visible by default.
    #[serde(default)]
    hidden: BTreeSet<String>,
    /// Panel sizes in CSS pixels, by panel key.
    #[serde(default)]
    sizes: BTreeMap<String, f64>,
}

impl Workspace {
    /// Whether the sidebar shows. Hidden only through the View menu.
    pub fn nav_visible(&self) -> bool {
        !self.nav_hidden
    }

    pub fn set_nav_visible(&mut self, visible: bool) {
        self.nav_hidden = !visible;
    }

    /// Whether the sidebar shows icons only.
    pub fn nav_collapsed(&self) -> bool {
        self.nav_collapsed
    }

    pub fn set_nav_collapsed(&mut self, collapsed: bool) {
        self.nav_collapsed = collapsed;
    }

    pub fn is_visible(&self, surface: Surface, id: &str) -> bool {
        !self.hidden.contains(&panel_key(surface, id))
    }

    pub fn set_visible(&mut self, surface: Surface, id: &str, visible: bool) {
        let key = panel_key(surface, id);
        if visible {
            self.hidden.remove(&key);
        } else {
            self.hidden.insert(key);
        }
    }

    /// The panel's size, if the user resized it.
    pub fn size(&self, surface: Surface, id: &str) -> Option<f64> {
        self.sizes.get(&panel_key(surface, id)).copied()
    }

    pub fn set_size(&mut self, surface: Surface, id: &str, px: f64) {
        self.sizes.insert(panel_key(surface, id), px);
    }

    /// Shows every panel of the page at its default size, and the sidebar.
    /// Other pages keep their layout.
    pub fn restore_default(&mut self, surface: Surface) {
        let prefix = format!("{}:", surface.key());
        self.hidden.retain(|k| !k.starts_with(&prefix));
        self.sizes.retain(|k, _| !k.starts_with(&prefix));
        self.nav_hidden = false;
        self.nav_collapsed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODES: Surface = Surface::Cluster(ClusterTab::Nodes);
    const RAFT: Surface = Surface::Cluster(ClusterTab::RaftGroups);

    #[test]
    fn panels_start_visible() {
        let ws = Workspace::default();
        assert!(ws.is_visible(NODES, "details"));
        assert!(ws.nav_visible());
    }

    #[test]
    fn hiding_is_scoped_to_one_page() {
        let mut ws = Workspace::default();
        ws.set_visible(NODES, "details", false);
        assert!(!ws.is_visible(NODES, "details"));
        assert!(ws.is_visible(RAFT, "details"));
        ws.set_visible(NODES, "details", true);
        assert!(ws.is_visible(NODES, "details"));
    }

    /// Hiding a panel keeps its size, so showing it again restores it.
    #[test]
    fn hiding_keeps_the_size() {
        let mut ws = Workspace::default();
        ws.set_size(NODES, "details", 420.0);
        ws.set_visible(NODES, "details", false);
        ws.set_visible(NODES, "details", true);
        assert_eq!(ws.size(NODES, "details"), Some(420.0));
    }

    #[test]
    fn restore_default_resets_only_the_page() {
        let mut ws = Workspace::default();
        ws.set_visible(NODES, "details", false);
        ws.set_size(NODES, "nodes", 300.0);
        ws.set_visible(RAFT, "peers", false);
        ws.set_nav_visible(false);
        ws.restore_default(NODES);
        assert!(ws.is_visible(NODES, "details"));
        assert_eq!(ws.size(NODES, "nodes"), None);
        assert!(!ws.is_visible(RAFT, "peers"));
        assert!(ws.nav_visible());
    }

    /// `explorer` is a prefix of `explorer/transfer`; restoring one page must
    /// not reset the other.
    #[test]
    fn restore_default_does_not_match_a_longer_key() {
        let mut ws = Workspace::default();
        ws.set_visible(Surface::DataTransfer, "jobs", false);
        ws.restore_default(Surface::Explorer);
        assert!(!ws.is_visible(Surface::DataTransfer, "jobs"));
    }

    #[test]
    fn layout_round_trips_through_json() {
        let mut ws = Workspace::default();
        ws.set_visible(NODES, "details", false);
        ws.set_size(NODES, "nodes", 300.0);
        ws.set_nav_collapsed(true);
        let json = sonic_rs::to_string(&ws).expect("serializes");
        let back: Workspace = sonic_rs::from_str(&json).expect("parses");
        assert_eq!(back, ws);
    }
}
