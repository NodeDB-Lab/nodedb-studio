//! Vector Space UI state, provided by `App` above the router so the three
//! tabs keep their drafts, the shared selection, and the map view while the
//! user is on another page.
//!
//! Drafts belong to a target (one collection field). Changing the target
//! shows that target's own drafts and results, never the previous target's
//! under a new label. The selected record is shared by every tab, and counts
//! only while it belongs to the current target.

use std::collections::BTreeMap;
use std::rc::Rc;

use dioxus::prelude::*;

use crate::models::vector::{NeighborRequest, NeighborResult, SearchForm};
use crate::models::vector_projection::{Projection, ProjectionForm, ProjectionRequest};

/// A target as a map key: `articles.embedding`.
pub fn target_key(collection: &str, field: &str) -> String {
    format!("{collection}.{field}")
}

/// The collection and field the three tabs work on. `None` until the user
/// picks, which means the first target the connection offers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TargetChoice {
    pub collection: Option<String>,
    pub field: Option<String>,
}

/// The record both tabs inspect, with the target it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub target: String,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Content,
    Vector,
}

impl InspectorTab {
    pub const ALL: [InspectorTab; 2] = [InspectorTab::Content, InspectorTab::Vector];

    pub fn label(self) -> &'static str {
        match self {
            InspectorTab::Content => "Content",
            InspectorTab::Vector => "Vector",
        }
    }
}

/// The Search tab for one target.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SearchUi {
    pub form: SearchForm,
    /// Whether the advanced controls show.
    pub advanced: bool,
    /// The search being run. Cleared when it finishes or is canceled.
    pub pending: Option<(u32, NeighborRequest)>,
    /// Counts searches, so a repeat is a new run.
    pub seq: u32,
    /// The last search that finished, with the request it answered.
    pub shown: Option<NeighborResult>,
    /// The last search's error, shown over the kept draft.
    pub error: Option<String>,
}

/// What a click on the map does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapTool {
    /// Select the record under the pointer.
    #[default]
    Select,
    /// Center the map on the pointer.
    Pan,
}

pub const ZOOM_MIN: f32 = 1.0;
pub const ZOOM_MAX: f32 = 32.0;
pub const ZOOM_STEP: f32 = 1.5;
/// The map shows `-EXTENT..EXTENT` on each axis at zoom 1.
pub const EXTENT: f32 = 1.08;

/// The window onto the projection: a zoom and the center of what shows, in
/// plot units (y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapView {
    pub zoom: f32,
    pub cx: f32,
    pub cy: f32,
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            cx: 0.0,
            cy: 0.0,
        }
    }
}

impl MapView {
    /// Half the width and height of what shows.
    pub fn half(self) -> f32 {
        EXTENT / self.zoom
    }

    /// The window's left, top, and size.
    pub fn window(self) -> (f32, f32, f32) {
        let h = self.half();
        (self.cx - h, self.cy - h, 2.0 * h)
    }

    /// Keep the window inside the plot.
    fn clamped(mut self) -> Self {
        self.zoom = self.zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        let room = (EXTENT - self.half()).max(0.0);
        self.cx = self.cx.clamp(-room, room);
        self.cy = self.cy.clamp(-room, room);
        self
    }

    /// Zoom by `factor`, keeping plot point `(px, py)` where it is on screen.
    pub fn zoomed_about(self, factor: f32, px: f32, py: f32) -> Self {
        let zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let k = self.zoom / zoom;
        Self {
            zoom,
            cx: px + (self.cx - px) * k,
            cy: py + (self.cy - py) * k,
        }
        .clamped()
    }

    pub fn zoomed(self, factor: f32) -> Self {
        self.zoomed_about(factor, self.cx, self.cy)
    }

    pub fn centered(self, x: f32, y: f32) -> Self {
        Self {
            cx: x,
            cy: y,
            ..self
        }
        .clamped()
    }

    /// Where a plot point sits in the window, as 0 to 100 on each axis.
    pub fn percent(self, x: f32, y: f32) -> (f32, f32) {
        let (left, top, size) = self.window();
        ((x - left) / size * 100.0, (y - top) / size * 100.0)
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        let (px, py) = self.percent(x, y);
        (0.0..=100.0).contains(&px) && (0.0..=100.0).contains(&py)
    }

    /// The zoom as a whole percent: `100`, `150`.
    pub fn percent_label(self) -> u32 {
        (self.zoom * 100.0).round() as u32
    }
}

/// The Projection tab for one target.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProjectionUi {
    pub form: ProjectionForm,
    /// Records the next run must include. Set by an explicit action.
    pub include: Vec<String>,
    pub pending: Option<(u32, ProjectionRequest)>,
    pub seq: u32,
    /// The last projection that finished. Kept while a new one runs.
    pub shown: Option<Rc<Projection>>,
    pub error: Option<String>,
    pub tool: MapTool,
    pub view: MapView,
    /// A legend category to emphasize, by index.
    pub focus: Option<u8>,
    /// The record id typed into Go to record.
    pub goto: String,
    pub goto_note: Option<String>,
}

/// Every target's drafts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Drafts {
    search: BTreeMap<String, SearchUi>,
    projection: BTreeMap<String, ProjectionUi>,
}

impl Drafts {
    pub fn search(&self, key: &str) -> SearchUi {
        self.search.get(key).cloned().unwrap_or_default()
    }

    pub fn search_mut(&mut self, key: &str) -> &mut SearchUi {
        self.search.entry(key.to_string()).or_default()
    }

    pub fn projection(&self, key: &str) -> ProjectionUi {
        self.projection.get(key).cloned().unwrap_or_default()
    }

    pub fn projection_mut(&mut self, key: &str) -> &mut ProjectionUi {
        self.projection.entry(key.to_string()).or_default()
    }
}

/// What a screenshot scene sets. `None` keeps the current state.
#[cfg(all(feature = "screenshots", target_os = "linux"))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VectorSeed {
    pub target: Option<String>,
    pub search: Option<SearchUi>,
    pub projection: Option<ProjectionUi>,
    pub selected: Option<Selection>,
    pub inspector: Option<InspectorTab>,
}

#[derive(Clone, Copy)]
pub struct VectorUi {
    pub choice: Signal<TargetChoice>,
    pub drafts: Signal<Drafts>,
    pub selected: Signal<Option<Selection>>,
    pub inspector: Signal<InspectorTab>,
    /// Whether the Vector tab lists every value.
    pub full_vector: Signal<bool>,
    /// Counts Refresh presses on the Index tab, so a repeat re-reads.
    pub index_epoch: Signal<u32>,
}

impl VectorUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            choice: Signal::new(TargetChoice::default()),
            drafts: Signal::new(Drafts::default()),
            selected: Signal::new(None),
            inspector: Signal::new(InspectorTab::default()),
            full_vector: Signal::new(false),
            index_epoch: Signal::new(0),
        })
    }

    /// Replace the state with a scene's seed.
    #[cfg(all(feature = "screenshots", target_os = "linux"))]
    pub fn apply(mut self, seed: VectorSeed, target: &str) {
        let key = seed.target.unwrap_or_else(|| target.to_string());
        if let Some((collection, field)) = key.split_once('.') {
            self.choice.set(TargetChoice {
                collection: Some(collection.to_string()),
                field: Some(field.to_string()),
            });
        }
        let mut drafts = Drafts::default();
        if let Some(s) = seed.search {
            *drafts.search_mut(&key) = s;
        }
        if let Some(p) = seed.projection {
            *drafts.projection_mut(&key) = p;
        }
        self.drafts.set(drafts);
        self.selected.set(seed.selected);
        self.inspector.set(seed.inspector.unwrap_or_default());
        self.full_vector.set(false);
    }

    /// The selected record's id, when it belongs to `target`.
    pub fn selected_id(&self, target: &str) -> Option<String> {
        self.selected
            .read()
            .as_ref()
            .filter(|s| s.target == target)
            .map(|s| s.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_view_shows_the_whole_plot() {
        let v = MapView::default();
        assert_eq!(v.percent_label(), 100);
        assert_eq!(v.percent(0.0, 0.0), (50.0, 50.0));
        assert!(v.contains(1.0, -1.0));
    }

    #[test]
    fn zoom_stays_between_its_limits() {
        let mut v = MapView::default();
        for _ in 0..30 {
            v = v.zoomed(ZOOM_STEP);
        }
        assert_eq!(v.zoom, ZOOM_MAX);
        for _ in 0..30 {
            v = v.zoomed(1.0 / ZOOM_STEP);
        }
        assert_eq!(v.zoom, ZOOM_MIN);
        assert_eq!((v.cx, v.cy), (0.0, 0.0));
    }

    #[test]
    fn zooming_about_a_point_keeps_it_under_the_pointer() {
        let v = MapView::default();
        let before = v.percent(0.4, -0.3);
        let z = v.zoomed_about(2.0, 0.4, -0.3);
        let after = z.percent(0.4, -0.3);
        assert!((before.0 - after.0).abs() < 1e-3 && (before.1 - after.1).abs() < 1e-3);
        assert_eq!(z.percent_label(), 200);
    }

    #[test]
    fn the_window_never_leaves_the_plot() {
        let v = MapView::default().zoomed(4.0).centered(9.0, -9.0);
        let (left, top, size) = v.window();
        assert!(left >= -EXTENT - 1e-4 && top >= -EXTENT - 1e-4);
        assert!(left + size <= EXTENT + 1e-4 && top + size <= EXTENT + 1e-4);
    }

    #[test]
    fn a_point_outside_the_window_is_not_contained() {
        let v = MapView::default().zoomed(8.0);
        assert!(!v.contains(0.9, 0.9));
        assert!(v.contains(0.0, 0.0));
    }

    #[test]
    fn drafts_are_kept_per_target() {
        let mut d = Drafts::default();
        d.search_mut("articles.embedding").form.record_id = "art104".into();
        d.search_mut("articles.title_vec").form.record_id = "art001".into();
        assert_eq!(d.search("articles.embedding").form.record_id, "art104");
        assert_eq!(d.search("articles.title_vec").form.record_id, "art001");
        assert_eq!(d.search("other.field"), SearchUi::default());
    }
}
