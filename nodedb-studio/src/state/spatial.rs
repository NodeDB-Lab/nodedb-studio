//! Spatial UI state, provided by `App` above the router so the three tabs
//! keep their view, drafts, results, and selection while the user is on
//! another page.
//!
//! Work in progress lives here: the shape being drawn, the analysis being
//! built, the cell being inspected. What the server holds does not: every
//! tab reads it through the backend seam. A result is bound to the request
//! that made it, and a changed draft marks it stale, never replaces it.

use std::collections::{BTreeMap, BTreeSet};

use dioxus::prelude::*;

use crate::models::geo_ops::{Bbox, Ring, Shape, Validity};
use crate::models::geo_proj::PlaneView;
use crate::models::geo_raster::{Palette, RasterCrs, ValueMode};
use crate::models::spatial::{FeatureRef, SourceRef, Viewport, ViewportRequest};
use crate::models::spatial_analyze::{
    AnalysisForm, AnalysisRequest, AnalysisResult, AreaKind, Operation,
};

/// What a pointer does on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapTool {
    /// Pick the feature under the pointer.
    #[default]
    Select,
    /// Drag to move the view.
    Pan,
    /// Drag a box to make draft query geometry.
    Rectangle,
    /// Click vertices to make draft query geometry.
    Polygon,
    /// Click two points to measure between them.
    Measure,
}

impl MapTool {
    pub const ALL: [MapTool; 5] = [
        MapTool::Select,
        MapTool::Pan,
        MapTool::Rectangle,
        MapTool::Polygon,
        MapTool::Measure,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MapTool::Select => "Select",
            MapTool::Pan => "Pan",
            MapTool::Rectangle => "Rectangle",
            MapTool::Polygon => "Polygon",
            MapTool::Measure => "Measure",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            MapTool::Select => "Click a feature to inspect it",
            MapTool::Pan => "Drag the map to move it",
            MapTool::Rectangle => "Drag a box. It makes draft query geometry, not data",
            MapTool::Polygon => {
                "Click vertices, then Finish. It makes draft query geometry, not data"
            }
            MapTool::Measure => "Click two points to measure between them",
        }
    }
}

/// The Feature panel's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeatureTab {
    #[default]
    Attributes,
    Geometry,
    Source,
}

impl FeatureTab {
    pub const ALL: [FeatureTab; 3] = [
        FeatureTab::Attributes,
        FeatureTab::Geometry,
        FeatureTab::Source,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FeatureTab::Attributes => "Attributes",
            FeatureTab::Geometry => "Geometry",
            FeatureTab::Source => "Source",
        }
    }
}

/// The Cell panel's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellTab {
    #[default]
    Value,
    Source,
}

impl CellTab {
    pub const ALL: [CellTab; 2] = [CellTab::Value, CellTab::Source];

    pub fn label(self) -> &'static str {
        match self {
            CellTab::Value => "Value",
            CellTab::Source => "Source",
        }
    }
}

/// Query geometry the user draws on the map. It is never stored.
#[derive(Debug, Clone, PartialEq)]
pub enum Draft {
    Rectangle {
        from: (f64, f64),
        to: (f64, f64),
        done: bool,
    },
    Polygon {
        points: Vec<(f64, f64)>,
        done: bool,
    },
}

impl Draft {
    pub fn done(&self) -> bool {
        match self {
            Draft::Rectangle { done, .. } | Draft::Polygon { done, .. } => *done,
        }
    }

    /// The closed ring, once the draft has an area.
    pub fn ring(&self) -> Option<Ring> {
        match self {
            Draft::Rectangle { from, to, .. } => {
                let b = Bbox::new(
                    from.0.min(to.0),
                    from.1.min(to.1),
                    from.0.max(to.0),
                    from.1.max(to.1),
                );
                (b.west < b.east && b.south < b.north).then(|| b.ring())
            }
            Draft::Polygon { points, .. } => {
                if points.len() < 3 {
                    return None;
                }
                let mut ring = points.clone();
                ring.push(points[0]);
                Some(ring)
            }
        }
    }

    pub fn label(&self) -> String {
        match self {
            Draft::Rectangle { .. } => "Rectangle".to_string(),
            Draft::Polygon { points, .. } => format!("Polygon, {} vertices", points.len()),
        }
    }
}

/// An analysis result drawn over the map: the exact query geometry and the
/// rows it returned.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultLayer {
    pub title: String,
    pub query: Shape,
    /// `(id, (lon, lat))` per returned row.
    pub rows: Vec<(String, (f64, f64))>,
    pub count: usize,
}

/// The Map tab.
#[derive(Debug, Clone, PartialEq)]
pub struct MapUi {
    /// Layers the user turned off, by key: a source's `collection.field`,
    /// `result`, or `draft`.
    pub hidden: BTreeSet<String>,
    pub view: Option<PlaneView>,
    /// The map's size in screen pixels, as last measured.
    pub size: (f64, f64),
    /// Whether `size` was measured. The first frame waits for it.
    pub measured: bool,
    pub tool: MapTool,
    pub selected: Option<FeatureRef>,
    /// The viewport read being run. Cleared when it finishes or is canceled.
    pub pending: Option<(u32, ViewportRequest)>,
    /// Counts reads, so a repeat is a new run.
    pub seq: u32,
    pub shown: Option<Viewport>,
    pub error: Option<String>,
    /// Whether the view moved since `shown` was read.
    pub moved: bool,
    pub draft: Option<Draft>,
    /// The points of a measurement: none, one, or two.
    pub measure: Vec<(f64, f64)>,
    pub result: Option<ResultLayer>,
    pub feature_tab: FeatureTab,
    /// A feature another tool asked the map to open. The page frames it,
    /// then clears this.
    pub open: Option<FeatureRef>,
    /// A box the page frames next, in degrees.
    pub frame: Option<Bbox>,
    /// The validity check of the selected feature, once run.
    pub validity: Option<(FeatureRef, Validity)>,
    pub validity_error: Option<String>,
}

impl Default for MapUi {
    fn default() -> Self {
        Self {
            hidden: BTreeSet::new(),
            view: None,
            size: (900.0, 520.0),
            measured: false,
            tool: MapTool::default(),
            selected: None,
            pending: None,
            seq: 0,
            shown: None,
            error: None,
            moved: false,
            draft: None,
            measure: Vec::new(),
            result: None,
            feature_tab: FeatureTab::default(),
            open: None,
            frame: None,
            validity: None,
            validity_error: None,
        }
    }
}

impl MapUi {
    pub fn visible(&self, key: &str) -> bool {
        !self.hidden.contains(key)
    }

    /// Show or hide a layer.
    pub fn set_visible(&mut self, key: &str, visible: bool) {
        if visible {
            self.hidden.remove(key);
        } else {
            self.hidden.insert(key.to_string());
        }
    }

    /// Select a feature. Its validity check belongs to the old selection.
    pub fn select(&mut self, feature: Option<FeatureRef>) {
        if self.selected != feature {
            self.validity = None;
            self.validity_error = None;
        }
        self.selected = feature;
    }

    /// Begin a read of `request`. A later read supersedes an earlier one.
    pub fn request(&mut self, request: ViewportRequest) {
        self.seq += 1;
        self.pending = Some((self.seq, request));
        self.error = None;
        self.moved = false;
    }
}

/// One source's analysis draft, with its last result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnalyzeDraft {
    pub form: AnalysisForm,
    /// Set after Run, so a form that was never checked shows no issues.
    pub tried: bool,
    pub pending: Option<(u32, AnalysisRequest)>,
    pub seq: u32,
    pub shown: Option<AnalysisResult>,
    pub error: Option<String>,
    /// The selected row's id.
    pub selected: Option<String>,
    pub source_open: bool,
}

/// The Analyze tab: a draft per source.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnalyzeUi {
    pub drafts: BTreeMap<String, AnalyzeDraft>,
}

impl AnalyzeUi {
    pub fn draft(&self, key: &str) -> AnalyzeDraft {
        self.drafts.get(key).cloned().unwrap_or_default()
    }

    pub fn draft_mut(&mut self, key: &str) -> &mut AnalyzeDraft {
        self.drafts.entry(key.to_string()).or_default()
    }
}

/// The Raster tab.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterUi {
    pub array: Option<String>,
    pub slice_index: u32,
    pub band: Option<String>,
    pub values: ValueMode,
    pub palette: Palette,
    pub crs: RasterCrs,
    /// The selected cell, `(row, col)`.
    pub cell: Option<(u32, u32)>,
    pub view: Option<PlaneView>,
    pub size: (f64, f64),
    /// Whether `size` was measured. The first fit waits for it.
    pub measured: bool,
    pub tab: CellTab,
    /// Frame the grid at the next draw: set by Fit and by a change of CRS.
    pub fit: bool,
}

impl Default for RasterUi {
    fn default() -> Self {
        Self {
            array: None,
            slice_index: 12,
            band: None,
            values: ValueMode::default(),
            palette: Palette::default(),
            crs: RasterCrs::default(),
            // 40.5 N, 73.5 W: the cell the Explorer reads as 22.4 °C.
            cell: Some((85, 185)),
            view: None,
            size: (900.0, 520.0),
            measured: false,
            tab: CellTab::default(),
            fit: true,
        }
    }
}

/// What a screenshot scene sets. `None` keeps the default of a tab.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpatialSeed {
    pub source: Option<SourceRef>,
    pub map: Option<MapUi>,
    pub analyze: Option<AnalyzeUi>,
    pub raster: Option<RasterUi>,
}

#[derive(Clone, Copy)]
pub struct SpatialUi {
    /// The active layer, shared by Map and Analyze. `None` is the first
    /// source the connection offers.
    pub source: Signal<Option<SourceRef>>,
    pub map: Signal<MapUi>,
    pub analyze: Signal<AnalyzeUi>,
    pub raster: Signal<RasterUi>,
}

impl SpatialUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            source: Signal::new(None),
            map: Signal::new(MapUi::default()),
            analyze: Signal::new(AnalyzeUi::default()),
            raster: Signal::new(RasterUi::default()),
        })
    }

    /// Replace the state with a scene's seed.
    pub fn apply(mut self, seed: SpatialSeed) {
        self.source.set(seed.source);
        self.map.set(seed.map.unwrap_or_default());
        self.analyze.set(seed.analyze.unwrap_or_default());
        self.raster.set(seed.raster.unwrap_or_default());
    }

    /// Ask the Map to open `id` of `source`. Another page calls this before
    /// it navigates: the map frames the feature once it has read it.
    /// Explorer's Open in Spatial is the caller.
    pub fn open_feature(mut self, source: SourceRef, id: &str) {
        let feature = FeatureRef {
            source: source.clone(),
            id: id.to_string(),
        };
        self.source.set(Some(source));
        let mut map = self.map.write();
        map.select(Some(feature.clone()));
        map.open = Some(feature);
        map.feature_tab = FeatureTab::Attributes;
    }

    /// Start an analysis near a point. Nothing runs.
    pub fn analyze_nearby(mut self, source: &SourceRef, lon: f64, lat: f64) {
        let mut analyze = self.analyze.write();
        let draft = analyze.draft_mut(&source.key());
        draft.form.op = Operation::Nearby;
        draft.form.lon = format!("{lon}");
        draft.form.lat = format!("{lat}");
        draft.tried = false;
        self.source.set(Some(source.clone()));
    }

    /// Start an analysis inside the drawn area. Nothing runs.
    pub fn analyze_area(mut self, source: &SourceRef, ring: Ring, rectangle: Option<Bbox>) {
        let mut analyze = self.analyze.write();
        let draft = analyze.draft_mut(&source.key());
        draft.form.op = Operation::Within;
        match rectangle {
            Some(b) => {
                draft.form.area = AreaKind::Rectangle;
                draft.form.west = format!("{}", b.west);
                draft.form.south = format!("{}", b.south);
                draft.form.east = format!("{}", b.east);
                draft.form.north = format!("{}", b.north);
            }
            None => {
                draft.form.area = AreaKind::Polygon;
                draft.form.polygon = Some(ring);
            }
        }
        draft.tried = false;
        self.source.set(Some(source.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_draft_has_a_ring_only_with_an_area() {
        let flat = Draft::Rectangle {
            from: (1.0, 2.0),
            to: (1.0, 5.0),
            done: true,
        };
        assert!(flat.ring().is_none());
        let d = Draft::Rectangle {
            from: (3.0, 5.0),
            to: (1.0, 2.0),
            done: true,
        };
        let ring = d.ring().expect("ring");
        assert_eq!(ring.first(), ring.last());
        assert_eq!(
            Shape::Polygon(vec![ring]).bbox(),
            Some(Bbox::new(1.0, 2.0, 3.0, 5.0))
        );
    }

    #[test]
    fn a_polygon_draft_needs_three_vertices_and_closes_itself() {
        let mut d = Draft::Polygon {
            points: vec![(0.0, 0.0), (1.0, 0.0)],
            done: false,
        };
        assert!(d.ring().is_none() && !d.done());
        if let Draft::Polygon { points, done } = &mut d {
            points.push((1.0, 1.0));
            *done = true;
        }
        let ring = d.ring().expect("ring");
        assert_eq!(ring.len(), 4);
        assert_eq!(ring.first(), ring.last());
        assert_eq!(d.label(), "Polygon, 3 vertices");
    }

    #[test]
    fn layers_hide_and_show_by_key() {
        let mut m = MapUi::default();
        assert!(m.visible("places.geometry"));
        m.set_visible("places.geometry", false);
        assert!(!m.visible("places.geometry"));
        m.set_visible("places.geometry", true);
        assert!(m.visible("places.geometry"));
    }

    #[test]
    fn a_new_selection_drops_the_old_validity_check() {
        let a = FeatureRef {
            source: SourceRef::new("places", "geometry"),
            id: "place104".into(),
        };
        let b = FeatureRef {
            id: "place103".into(),
            ..a.clone()
        };
        let mut m = MapUi::default();
        m.select(Some(a.clone()));
        m.validity = Some((a.clone(), Validity::Valid));
        m.select(Some(a));
        assert!(m.validity.is_some(), "the same feature keeps its check");
        m.select(Some(b));
        assert!(m.validity.is_none());
    }

    #[test]
    fn a_request_supersedes_the_last_and_clears_the_moved_mark() {
        let mut m = MapUi {
            moved: true,
            ..MapUi::default()
        };
        let req = ViewportRequest {
            sources: vec![SourceRef::new("places", "geometry")],
            bbox: Bbox::new(0.0, 0.0, 1.0, 1.0),
            limit: 10,
        };
        m.request(req.clone());
        m.request(req);
        assert_eq!(m.pending.as_ref().map(|p| p.0), Some(2));
        assert!(!m.moved);
    }

    #[test]
    fn the_raster_starts_on_the_cell_the_explorer_reads_as_22_4() {
        let r = RasterUi::default();
        assert_eq!(r.cell, Some((85, 185)));
        assert_eq!(r.slice_index, 12);
    }
}
