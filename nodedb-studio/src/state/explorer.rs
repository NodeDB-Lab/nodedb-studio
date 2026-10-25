//! The Explorer's selection and how it looks at the selected collection.
//!
//! Provided by `Studio` above the router, because the Data Transfer page
//! reads the same filter the Explorer applies, and a pick survives leaving
//! the Explorer and coming back.

use std::collections::BTreeSet;

use dioxus::prelude::{Signal, WritableExt};

use crate::models::array_data::SliceRequest;
use crate::models::collection::{Attached, Collection, Profile};
use crate::models::records::{Bbox, Filter, RecordQuery, TimeRange};
use crate::models::schema::CollectionSchema;

/// The selected collection.
#[derive(Clone, PartialEq, Debug)]
pub struct Selected {
    pub name: String,
}

/// Bumped after a write, so reads that depend on stored data run again.
#[derive(Clone, Copy)]
pub struct RecordsEpoch(pub Signal<u32>);

/// A tab of the collection panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollectionTab {
    #[default]
    Data,
    FullText,
    Schema,
    Indexes,
    Graph,
    Dimensions,
    Attributes,
    Metadata,
}

impl CollectionTab {
    pub fn label(self) -> &'static str {
        match self {
            CollectionTab::Data => "Data",
            CollectionTab::FullText => "Full-text",
            CollectionTab::Schema => "Schema",
            CollectionTab::Indexes => "Indexes",
            CollectionTab::Graph => "Graph",
            CollectionTab::Dimensions => "Dimensions",
            CollectionTab::Attributes => "Attributes",
            CollectionTab::Metadata => "Metadata",
        }
    }

    /// The tabs a collection offers, in order. A capability adds its tab.
    pub fn for_collection(c: &Collection) -> Vec<CollectionTab> {
        if c.profile == Profile::Array {
            return vec![
                CollectionTab::Data,
                CollectionTab::Dimensions,
                CollectionTab::Attributes,
                CollectionTab::Metadata,
            ];
        }
        let mut tabs = vec![CollectionTab::Data];
        if c.has(Attached::FullText) {
            tabs.push(CollectionTab::FullText);
        }
        tabs.push(CollectionTab::Schema);
        tabs.push(CollectionTab::Indexes);
        if c.has(Attached::Graph) {
            tabs.push(CollectionTab::Graph);
        }
        tabs
    }
}

/// A tab of the record details panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsTab {
    Document,
    Json,
    Raw,
    FullText,
    Vector,
    Indexes,
    Fields,
    History,
    Properties,
    Geometry,
}

impl DetailsTab {
    pub fn label(self) -> &'static str {
        match self {
            DetailsTab::Document => "Document",
            DetailsTab::Json => "JSON",
            DetailsTab::Raw => "Raw",
            DetailsTab::FullText => "Full-text",
            DetailsTab::Vector => "Vector",
            DetailsTab::Indexes => "Indexes",
            DetailsTab::Fields => "Fields",
            DetailsTab::History => "History",
            DetailsTab::Properties => "Properties",
            DetailsTab::Geometry => "Geometry",
        }
    }

    /// The tabs of a collection's details panel. Empty when the panel is one
    /// view with no tabs. The first is the default.
    pub fn for_collection(c: &Collection) -> Vec<DetailsTab> {
        match c.profile {
            Profile::Document if c.has(Attached::FullText) || c.has(Attached::Vector) => {
                let mut tabs = vec![DetailsTab::Document];
                if c.has(Attached::FullText) {
                    tabs.push(DetailsTab::FullText);
                }
                if c.has(Attached::Vector) {
                    tabs.push(DetailsTab::Vector);
                }
                tabs.push(DetailsTab::Indexes);
                tabs
            }
            Profile::Document => vec![DetailsTab::Json, DetailsTab::Raw],
            Profile::Columnar => vec![DetailsTab::Fields, DetailsTab::Json, DetailsTab::History],
            Profile::Spatial => vec![
                DetailsTab::Properties,
                DetailsTab::Geometry,
                DetailsTab::Indexes,
            ],
            Profile::DocumentStrict | Profile::KeyValue | Profile::Timeseries | Profile::Array => {
                Vec::new()
            }
        }
    }
}

/// The two views of a graph overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphSub {
    Nodes,
    #[default]
    Edges,
}

/// The two ways to read an array slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliceMode {
    #[default]
    Heatmap,
    Table,
}

/// The array slice controls. The inputs are text until Run slice parses
/// them, so a half-typed range never reads the server.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceForm {
    pub time_index: u32,
    pub lat_from: String,
    pub lat_to: String,
    pub lon_from: String,
    pub lon_to: String,
    pub attribute: String,
    /// What the last Run slice asked for. The result shows this.
    pub applied: SliceRequest,
    /// The selected cell, as latitude and longitude in tenths of a degree.
    pub cell: Option<(i64, i64)>,
    pub mode: SliceMode,
    /// Why the inputs do not parse.
    pub error: Option<String>,
}

impl Default for SliceForm {
    fn default() -> Self {
        Self {
            time_index: 12,
            lat_from: "40.0".into(),
            lat_to: "41.0".into(),
            lon_from: "-74.0".into(),
            lon_to: "-73.0".into(),
            attribute: "temperature_c".into(),
            applied: SliceRequest {
                time_index: 12,
                lat: (40.0, 41.0),
                lon: (-74.0, -73.0),
                attribute: "temperature_c".into(),
            },
            cell: Some((405, -735)),
            mode: SliceMode::Heatmap,
            error: None,
        }
    }
}

impl SliceForm {
    /// Parses the inputs into a request. Names the field that fails.
    pub fn parse(&self) -> Result<SliceRequest, String> {
        let num = |label: &str, text: &str| {
            text.trim()
                .parse::<f64>()
                .map_err(|_| format!("{label} must be a number."))
        };
        Ok(SliceRequest {
            time_index: self.time_index,
            lat: (
                num("Latitude", &self.lat_from)?,
                num("Latitude", &self.lat_to)?,
            ),
            lon: (
                num("Longitude", &self.lon_from)?,
                num("Longitude", &self.lon_to)?,
            ),
            attribute: self.attribute.clone(),
        })
    }
}

/// A tab of the Data Transfer panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransferTab {
    Import,
    #[default]
    Export,
    History,
}

impl TransferTab {
    pub const ALL: [TransferTab; 3] = [
        TransferTab::Import,
        TransferTab::Export,
        TransferTab::History,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TransferTab::Import => "Import",
            TransferTab::Export => "Export",
            TransferTab::History => "History",
        }
    }
}

/// How the Explorer looks at one collection. Reset on a collection switch.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordView {
    pub tab: CollectionTab,
    pub filter: Option<Filter>,
    /// Text search, matched against every text field.
    pub text: String,
    /// A field and whether it sorts descending. `None` uses the schema's.
    pub sort: Option<(String, bool)>,
    /// Zero-based.
    pub page: usize,
    /// `None` uses the schema's default.
    pub page_size: Option<usize>,
    pub range: TimeRange,
    /// What the bbox input holds, valid or not.
    pub bbox_text: String,
    pub bbox: Option<Bbox>,
    /// Column headers the user turned off.
    pub hidden_columns: BTreeSet<String>,
    /// The chosen record. `None` shows the first row of the page.
    pub record: Option<String>,
    pub details_tab: Option<DetailsTab>,
    pub graph_sub: GraphSub,
    pub relation: Option<String>,
    pub edge_text: String,
    pub edge_page: usize,
    pub edge: Option<String>,
    pub slice: SliceForm,
    pub transfer_tab: TransferTab,
}

impl Default for RecordView {
    fn default() -> Self {
        Self {
            tab: CollectionTab::Data,
            filter: None,
            text: String::new(),
            sort: None,
            page: 0,
            page_size: None,
            range: TimeRange::default(),
            bbox_text: String::new(),
            bbox: None,
            hidden_columns: BTreeSet::new(),
            record: None,
            details_tab: None,
            graph_sub: GraphSub::default(),
            relation: None,
            edge_text: String::new(),
            edge_page: 0,
            edge: None,
            slice: SliceForm::default(),
            transfer_tab: TransferTab::default(),
        }
    }
}

impl RecordView {
    /// The query this view asks of a collection with `schema`.
    pub fn query(&self, schema: &CollectionSchema) -> RecordQuery {
        RecordQuery {
            filter: self.filter.clone(),
            text: self.text.clone(),
            sort: self.sort.clone().or_else(|| schema.default_sort.clone()),
            page: self.page,
            page_size: self.page_size.unwrap_or(schema.default_page_size),
            range: self.range,
            bbox: self.bbox,
        }
    }

    /// Back to the first row of the first page. A filter or sort change
    /// moves the result, so a page number and a pick no longer mean anything.
    pub fn restart_paging(&mut self) {
        self.page = 0;
        self.record = None;
    }
}

/// Selects a collection and resets how it is viewed.
pub fn select_collection(
    selected: &mut Signal<Option<Selected>>,
    view: &mut Signal<RecordView>,
    name: &str,
) {
    selected.set(Some(Selected {
        name: name.to_string(),
    }));
    view.set(RecordView::default());
}

/// Opens one record of a collection in the Data tab.
pub fn select_record(
    selected: &mut Signal<Option<Selected>>,
    view: &mut Signal<RecordView>,
    collection: &str,
    id: &str,
) {
    select_collection(selected, view, collection);
    view.write().record = Some(id.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::collection::Collection;

    fn collection(profile: Profile, attached: &[Attached]) -> Collection {
        Collection {
            name: "c".into(),
            profile,
            attached: attached.to_vec(),
            description: String::new(),
            rows: None,
        }
    }

    #[test]
    fn a_capability_adds_its_own_tab() {
        let plain = CollectionTab::for_collection(&collection(Profile::Document, &[]));
        assert_eq!(
            plain,
            [
                CollectionTab::Data,
                CollectionTab::Schema,
                CollectionTab::Indexes
            ]
        );
        let rich = CollectionTab::for_collection(&collection(
            Profile::Document,
            &[Attached::FullText, Attached::Graph],
        ));
        assert_eq!(rich[1], CollectionTab::FullText);
        assert_eq!(rich.last(), Some(&CollectionTab::Graph));
    }

    #[test]
    fn an_array_has_its_own_tabs() {
        let tabs = CollectionTab::for_collection(&collection(Profile::Array, &[]));
        assert_eq!(tabs[1], CollectionTab::Dimensions);
        assert!(!tabs.contains(&CollectionTab::Schema));
    }

    #[test]
    fn details_tabs_follow_the_profile() {
        let tabs = |p, a: &[Attached]| DetailsTab::for_collection(&collection(p, a));
        assert_eq!(
            tabs(Profile::Document, &[]),
            [DetailsTab::Json, DetailsTab::Raw]
        );
        assert!(tabs(Profile::DocumentStrict, &[]).is_empty());
        assert_eq!(
            tabs(Profile::Document, &[Attached::Vector]),
            [
                DetailsTab::Document,
                DetailsTab::Vector,
                DetailsTab::Indexes
            ]
        );
    }

    #[test]
    fn a_view_falls_back_to_the_schema_defaults() {
        use crate::data::mock::collection_schema;
        let schema = collection_schema("users").expect("users");
        let q = RecordView::default().query(&schema);
        assert_eq!(q.page_size, 5);
        assert_eq!(q.sort, Some(("updated_at".to_string(), true)));
        let view = RecordView {
            page_size: Some(50),
            sort: Some(("name".into(), false)),
            ..RecordView::default()
        };
        let q = view.query(&schema);
        assert_eq!(
            (q.page_size, q.sort),
            (50, Some(("name".to_string(), false)))
        );
    }

    #[test]
    fn slice_inputs_name_the_field_that_fails() {
        let mut f = SliceForm::default();
        assert!(f.parse().is_ok());
        f.lon_to = "east".into();
        assert_eq!(f.parse(), Err("Longitude must be a number.".to_string()));
    }

    #[test]
    fn restarting_paging_clears_the_pick() {
        let mut v = RecordView {
            page: 4,
            record: Some("u1".into()),
            ..RecordView::default()
        };
        v.restart_paging();
        assert_eq!((v.page, v.record), (0, None));
    }
}

/// Why the last import or export action failed. A root signal, because the
/// task that writes it outlives the page that started it.
#[derive(Clone, Copy)]
pub struct TransferNotice(pub Signal<Option<String>>);
