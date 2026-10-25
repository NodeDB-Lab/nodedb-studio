//! The scene model: what a scene shows, and the builders the scene lists use.

use crate::models::records::Filter;
use crate::models::workbench::TxAccess;
use crate::routes::Route;
use crate::screenshots::designer_scenes::DesignerSeed;
use crate::state::admin::AdminSeed;
use crate::state::cluster::ClusterSeed;
use crate::state::connection_form::ConnectionForm;
use crate::state::console::ConsoleState;
use crate::state::events::EventsSeed;
use crate::state::explorer::{
    CollectionTab, DetailsTab, GraphSub, RecordView, Selected, TransferTab,
};
use crate::state::query::QueryWorkspace;
use crate::state::record_editor::RecordEditor;
use crate::state::search::SearchSeed;
use crate::state::spatial::SpatialSeed;
use crate::state::sync::SyncSeed;
use crate::state::timeseries::TimeseriesSeed;
use crate::state::ui::{ModalKind, Popover};
use crate::state::vector::VectorSeed;

/// The saved connection every studio scene opens. It holds every capability,
/// so every destination renders.
pub const STUDIO_CONNECTION: &str = "staging-cluster";

/// The saved connection whose credentials cannot write.
pub const READ_ONLY_CONNECTION: &str = "Production";

/// How a scene leaves the Explorer: the collection, and how it is viewed.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplorerSeed {
    pub collection: &'static str,
    pub tab: CollectionTab,
    /// A `field = value` filter on the Data tab.
    pub filter: Option<(&'static str, &'static str)>,
    pub details_tab: Option<DetailsTab>,
    /// The relation filter of the Graph tab.
    pub relation: Option<&'static str>,
    /// The Data Transfer tab. `None` is Export.
    pub transfer: Option<TransferTab>,
}

impl ExplorerSeed {
    /// The selection and view the Explorer starts the scene with.
    pub fn target(&self) -> (Selected, RecordView) {
        let view = RecordView {
            tab: self.tab,
            filter: self.filter.map(|(field, value)| Filter {
                field: field.to_string(),
                value: value.to_string(),
            }),
            details_tab: self.details_tab,
            relation: self.relation.map(str::to_string),
            graph_sub: GraphSub::Edges,
            transfer_tab: self.transfer.unwrap_or_default(),
            ..RecordView::default()
        };
        (
            Selected {
                name: self.collection.to_string(),
            },
            view,
        )
    }
}

/// What the window shows under any overlay.
#[derive(Clone, PartialEq)]
pub enum Screen {
    /// The Connection Manager.
    Disconnected,
    /// The studio at `route`. `explorer` sets the Explorer's selection.
    Studio {
        route: Route,
        explorer: Option<ExplorerSeed>,
    },
}

/// What opens on top of the screen.
#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    None,
    Modal(ModalKind),
    /// The connection dialog, open with these fields.
    NewConnection(Box<ConnectionForm>),
    Popover(Popover),
    /// The record editor, open on this edit.
    RecordEditor(Box<RecordEditor>),
    /// The palette, open with this text typed.
    Palette(&'static str),
}

#[derive(Clone, PartialEq)]
pub struct Scene {
    /// File stem of the PNG. Lowercase ASCII, digits and `-`.
    pub name: &'static str,
    pub screen: Screen,
    /// The saved connection the scene runs on.
    pub connection: &'static str,
    pub overlay: Overlay,
    /// Panels of the scene's page to hide, by `PanelSpec` id. Every other
    /// panel shows at its default size.
    pub hidden: &'static [&'static str],
    /// Panel and region sizes to set, by id, in CSS pixels.
    pub sizes: &'static [(&'static str, f64)],
    /// The Query tabs the scene opens with. `None` keeps the current ones.
    pub query: Option<Box<QueryWorkspace>>,
    /// The Console session the scene opens with. `None` keeps the current one.
    pub console: Option<Box<ConsoleState>>,
    /// A transaction to open before the scene. `None` leaves the session idle.
    pub transaction: Option<TxAccess>,
    /// The Designer's staged changes and open drawers. `None` keeps them.
    pub designer: Option<Box<DesignerSeed>>,
    /// The Events & Jobs tabs' state. `None` keeps the current state.
    pub events: Option<Box<EventsSeed>>,
    /// The Vector Space drafts and selection. `None` keeps the current ones.
    pub vector: Option<Box<VectorSeed>>,
    /// The Timeseries drafts and open documents. `None` keeps the current ones.
    pub timeseries: Option<Box<TimeseriesSeed>>,
    /// The Spatial view, drafts, and selection. `None` keeps the current ones.
    pub spatial: Option<Box<SpatialSeed>>,
    /// The Search queries, results, and selections. `None` keeps the current ones.
    pub search: Option<Box<SearchSeed>>,
    /// The Cluster tabs' selections and open reviews. `None` keeps the current ones.
    pub cluster: Option<Box<ClusterSeed>>,
    /// The Sync tabs' selections and open sheets. `None` keeps the current ones.
    pub sync: Option<Box<SyncSeed>>,
    /// The Administration tabs' selections, drafts, and open reviews. `None` keeps the current ones.
    pub admin: Option<Box<AdminSeed>>,
}

pub fn scene(name: &'static str, screen: Screen, overlay: Overlay) -> Scene {
    Scene {
        name,
        screen,
        connection: STUDIO_CONNECTION,
        overlay,
        hidden: &[],
        sizes: &[],
        query: None,
        console: None,
        transaction: None,
        designer: None,
        events: None,
        vector: None,
        timeseries: None,
        spatial: None,
        search: None,
        cluster: None,
        sync: None,
        admin: None,
    }
}

pub fn at(route: Route) -> Screen {
    Screen::Studio {
        route,
        explorer: None,
    }
}

pub fn seed(collection: &'static str) -> ExplorerSeed {
    ExplorerSeed {
        collection,
        tab: CollectionTab::Data,
        filter: None,
        details_tab: None,
        relation: None,
        transfer: None,
    }
}

/// The Explorer on `collection`, with its default view.
pub fn explorer(collection: &'static str) -> Screen {
    explorer_with(seed(collection))
}

pub fn explorer_with(seed: ExplorerSeed) -> Screen {
    Screen::Studio {
        route: Route::Explorer {},
        explorer: Some(seed),
    }
}
