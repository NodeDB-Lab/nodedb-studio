//! Dashboards: named documents of query panels.
//!
//! A dashboard is a workspace document, not a database object. It stores the
//! query of each panel, never a result: opening one runs fresh queries.
//! Panels inherit the dashboard's range and filters; the dashboard's own
//! range and filters are what a panel's query is overridden with.

use crate::models::records::Filter;
use crate::models::timeseries::{ChartKind, ExploreQuery, TimeSpan};

/// Where documents live until a server-hosted store is chosen.
pub const WORKSPACE_LOCATION: &str = "Workspace · ~/NodeDB Studio/dashboards";

/// How often a view re-runs its queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Refresh {
    #[default]
    Off,
    Seconds10,
    Seconds30,
    Minute1,
    Minutes5,
}

impl Refresh {
    pub const ALL: [Refresh; 5] = [
        Refresh::Off,
        Refresh::Seconds10,
        Refresh::Seconds30,
        Refresh::Minute1,
        Refresh::Minutes5,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Refresh::Off => "Off",
            Refresh::Seconds10 => "10s",
            Refresh::Seconds30 => "30s",
            Refresh::Minute1 => "1m",
            Refresh::Minutes5 => "5m",
        }
    }

    /// The interval in seconds. `None` is Off.
    pub fn seconds(self) -> Option<u64> {
        match self {
            Refresh::Off => None,
            Refresh::Seconds10 => Some(10),
            Refresh::Seconds30 => Some(30),
            Refresh::Minute1 => Some(60),
            Refresh::Minutes5 => Some(300),
        }
    }
}

/// One panel: a title, a query, and how to draw it.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelDef {
    /// Unique within the dashboard. Stable across edits and moves.
    pub id: String,
    pub title: String,
    /// The panel's own query. Its range and the dashboard's filters are
    /// replaced when it runs: see `Dashboard::run_query`.
    pub query: ExploreQuery,
    pub kind: ChartKind,
    /// The panel's height in pixels.
    pub height: u32,
}

pub const MIN_PANEL_HEIGHT: u32 = 160;
pub const MAX_PANEL_HEIGHT: u32 = 720;

/// A saved or unsaved dashboard document.
#[derive(Debug, Clone, PartialEq)]
pub struct Dashboard {
    /// `None` until the first save gives the document an identity.
    pub id: Option<String>,
    pub name: String,
    /// Counts saves. A save names the revision it read.
    pub revision: u64,
    /// When the stored revision was saved.
    pub saved_at: Option<i64>,
    pub range: TimeSpan,
    pub refresh: Refresh,
    /// Filters every panel applies on top of its own.
    pub filters: Vec<Filter>,
    pub panels: Vec<PanelDef>,
}

/// The library row of a document.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardSummary {
    pub id: String,
    pub name: String,
    /// The collections its panels read, in panel order, once each.
    pub collections: Vec<String>,
    pub panels: usize,
    pub revision: u64,
    pub saved_at: i64,
}

impl Dashboard {
    /// An empty document that is not stored yet.
    pub fn untitled(range: TimeSpan) -> Self {
        Self {
            id: None,
            name: "Untitled dashboard".to_string(),
            revision: 0,
            saved_at: None,
            range,
            refresh: Refresh::Off,
            filters: Vec::new(),
            panels: Vec::new(),
        }
    }

    pub fn panel(&self, id: &str) -> Option<&PanelDef> {
        self.panels.iter().find(|p| p.id == id)
    }

    /// An id no panel of this document uses.
    pub fn next_panel_id(&self) -> String {
        let n = self
            .panels
            .iter()
            .filter_map(|p| p.id.strip_prefix('p').and_then(|n| n.parse::<u32>().ok()))
            .max()
            .map_or(1, |m| m + 1);
        format!("p{n}")
    }

    /// The query a panel runs: its own, over the dashboard's range, with the
    /// dashboard's filters added to the panel's.
    pub fn run_query(&self, panel: &PanelDef) -> ExploreQuery {
        let mut q = panel.query.clone();
        q.span = self.range;
        for f in &self.filters {
            if !q.filters.iter().any(|own| own.field == f.field) {
                q.filters.push(f.clone());
            }
        }
        q
    }

    /// The collections the panels read, once each.
    pub fn collections(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in &self.panels {
            if !out.contains(&p.query.collection) {
                out.push(p.query.collection.clone());
            }
        }
        out
    }

    /// Move a panel `delta` places. A move past either end stays put.
    /// Returns whether the order changed.
    pub fn move_panel(&mut self, id: &str, delta: isize) -> bool {
        let Some(from) = self.panels.iter().position(|p| p.id == id) else {
            return false;
        };
        let to = from as isize + delta;
        if to < 0 || to as usize >= self.panels.len() || delta == 0 {
            return false;
        }
        let p = self.panels.remove(from);
        self.panels.insert(to as usize, p);
        true
    }

    /// Remove a panel from the document. Returns it with its place, so an
    /// undo puts it back where it was.
    pub fn remove_panel(&mut self, id: &str) -> Option<(usize, PanelDef)> {
        let at = self.panels.iter().position(|p| p.id == id)?;
        Some((at, self.panels.remove(at)))
    }

    pub fn insert_panel(&mut self, at: usize, panel: PanelDef) {
        let at = at.min(self.panels.len());
        self.panels.insert(at, panel);
    }

    /// A copy of a panel right after it, under a new id and title.
    pub fn duplicate_panel(&mut self, id: &str) -> Option<String> {
        let at = self.panels.iter().position(|p| p.id == id)?;
        let mut copy = self.panels[at].clone();
        copy.id = self.next_panel_id();
        copy.title = format!("{} (copy)", copy.title);
        let new_id = copy.id.clone();
        self.panels.insert(at + 1, copy);
        Some(new_id)
    }

    /// Set a panel's height inside its limits.
    pub fn resize_panel(&mut self, id: &str, height: u32) {
        if let Some(p) = self.panels.iter_mut().find(|p| p.id == id) {
            p.height = height.clamp(MIN_PANEL_HEIGHT, MAX_PANEL_HEIGHT);
        }
    }

    /// Why this document cannot be saved under `name`, if it cannot.
    pub fn name_issue(name: &str) -> Option<&'static str> {
        let n = name.trim();
        if n.is_empty() {
            Some("Name the dashboard.")
        } else if n.chars().count() > 80 {
            Some("Keep the name to 80 characters.")
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::models::timeseries::{Aggregate, Bucket, GapPolicy, Measure};

    fn range() -> TimeSpan {
        TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0))
    }

    fn panel(id: &str, collection: &str) -> PanelDef {
        PanelDef {
            id: id.into(),
            title: id.to_uppercase(),
            query: ExploreQuery {
                collection: collection.into(),
                time_field: "sold_at".into(),
                measure: Measure::of(Aggregate::Sum, "revenue"),
                filters: Vec::new(),
                bucket: Bucket::Day1,
                group_by: None,
                gap: GapPolicy::Missing,
                span: TimeSpan::new(0, 1),
            },
            kind: ChartKind::Line,
            height: 300,
        }
    }

    fn doc() -> Dashboard {
        let mut d = Dashboard::untitled(range());
        d.panels = vec![
            panel("p1", "sales"),
            panel("p2", "sales"),
            panel("p3", "shipments"),
        ];
        d
    }

    #[test]
    fn a_panel_runs_over_the_dashboard_range_and_filters() {
        let mut d = doc();
        d.filters.push(Filter {
            field: "currency".into(),
            value: "USD".into(),
        });
        let q = d.run_query(&d.panels[0]);
        assert_eq!(q.span, range());
        assert_eq!(q.filters.len(), 1);
    }

    #[test]
    fn a_panel_filter_on_the_same_field_wins_over_the_dashboard_one() {
        let mut d = doc();
        d.filters.push(Filter {
            field: "currency".into(),
            value: "USD".into(),
        });
        d.panels[0].query.filters.push(Filter {
            field: "currency".into(),
            value: "EUR".into(),
        });
        let q = d.run_query(&d.panels[0]);
        assert_eq!(q.filters.len(), 1);
        assert_eq!(q.filters[0].value, "EUR");
    }

    #[test]
    fn new_panel_ids_never_repeat() {
        let mut d = doc();
        assert_eq!(d.next_panel_id(), "p4");
        d.remove_panel("p2");
        assert_eq!(d.next_panel_id(), "p4");
        d.panels.clear();
        assert_eq!(d.next_panel_id(), "p1");
    }

    #[test]
    fn moving_a_panel_stops_at_either_end() {
        let mut d = doc();
        assert!(!d.move_panel("p1", -1));
        assert!(d.move_panel("p1", 1));
        assert_eq!(d.panels[1].id, "p1");
        assert!(!d.move_panel("p3", 1));
        assert!(!d.move_panel("nope", 1));
    }

    #[test]
    fn removing_a_panel_can_be_undone_in_place() {
        let mut d = doc();
        let before = d.clone();
        let (at, p) = d.remove_panel("p2").expect("panel");
        assert_eq!(d.panels.len(), 2);
        d.insert_panel(at, p);
        assert_eq!(d, before);
    }

    #[test]
    fn a_duplicate_follows_its_original_under_a_new_id() {
        let mut d = doc();
        let id = d.duplicate_panel("p1").expect("copy");
        assert_eq!(d.panels[1].id, id);
        assert_eq!(d.panels[1].title, "P1 (copy)");
        let mut ids: Vec<&str> = d.panels.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), d.panels.len());
    }

    #[test]
    fn heights_stay_inside_their_limits() {
        let mut d = doc();
        d.resize_panel("p1", 10);
        assert_eq!(d.panels[0].height, MIN_PANEL_HEIGHT);
        d.resize_panel("p1", 5_000);
        assert_eq!(d.panels[0].height, MAX_PANEL_HEIGHT);
    }

    #[test]
    fn collections_list_each_once_in_panel_order() {
        assert_eq!(doc().collections(), ["sales", "shipments"]);
    }

    #[test]
    fn names_are_checked() {
        assert!(Dashboard::name_issue("  ").is_some());
        assert!(Dashboard::name_issue("Sales analysis").is_none());
        assert!(Dashboard::name_issue(&"x".repeat(81)).is_some());
    }
}
