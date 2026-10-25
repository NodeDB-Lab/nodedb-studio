//! Timeseries UI state, provided by `App` above the router so each tab keeps
//! its draft, result, open document, and open sheet while the user is on
//! another page.
//!
//! Work in progress lives here: the query being built, the dashboard being
//! edited, the changes waiting for review. Saved things live behind the
//! seam. A hidden panel keeps all of it, because the panel only stops
//! displaying.

use std::collections::BTreeSet;

use dioxus::prelude::*;

use crate::models::timeseries::{ExploreQuery, ExploreResult, TimeSpan};
use crate::models::ts_backfill::{BackfillPreview, BackfillSpec, RunMode};
use crate::models::ts_dashboard::{Dashboard, PanelDef, Refresh};
use crate::models::ts_lifecycle::{
    ChangeReview, LatePolicy, LifecycleChange, RetentionDraft, RollupDraft,
};
pub use crate::state::events::feeds::Notice;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryTab {
    #[default]
    Sql,
    Fields,
}

impl QueryTab {
    pub const ALL: [QueryTab; 2] = [QueryTab::Sql, QueryTab::Fields];

    pub fn label(self) -> &'static str {
        match self {
            QueryTab::Sql => "SQL",
            QueryTab::Fields => "Fields",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortCol {
    #[default]
    Bucket,
    Group,
    Value,
}

/// Where an exploration stands. Cancel and a failed refresh keep the last
/// result on screen, marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreStatus {
    Draft,
    Running,
    Result,
    Empty,
    Cancelled,
    Error,
}

/// The Explore tab.
#[derive(Debug, Clone, PartialEq)]
pub struct ExploreUi {
    /// The query being built. `None` until the catalog loads.
    pub draft: Option<ExploreQuery>,
    /// Counts runs. A reply for an older number is dropped.
    pub seq: u32,
    /// The run whose reply is awaited.
    pub running: Option<u32>,
    pub shown: Option<ExploreResult>,
    /// The last run's error, shown over the kept result.
    pub error: Option<String>,
    pub cancelled: bool,
    /// Series the legend hides. Never changes the query.
    pub hidden: BTreeSet<String>,
    pub refresh: Refresh,
    /// Ranges before each brush or edit, newest last.
    pub previous_ranges: Vec<TimeSpan>,
    pub sort: (SortCol, bool),
    pub tab: QueryTab,
    pub picker_open: bool,
    pub notice: Option<Notice>,
    /// Another page asked to open this collection and time field.
    pub requested: Option<(String, Option<String>)>,
}

impl Default for ExploreUi {
    fn default() -> Self {
        Self {
            draft: None,
            seq: 0,
            running: None,
            shown: None,
            error: None,
            cancelled: false,
            hidden: BTreeSet::new(),
            refresh: Refresh::Off,
            previous_ranges: Vec::new(),
            sort: (SortCol::Bucket, false),
            tab: QueryTab::Sql,
            picker_open: false,
            notice: None,
            requested: None,
        }
    }
}

impl ExploreUi {
    pub fn status(&self) -> ExploreStatus {
        if self.running.is_some() {
            ExploreStatus::Running
        } else if self.error.is_some() {
            ExploreStatus::Error
        } else if self.cancelled {
            ExploreStatus::Cancelled
        } else {
            match &self.shown {
                None => ExploreStatus::Draft,
                Some(r) if r.is_empty() => ExploreStatus::Empty,
                Some(_) => ExploreStatus::Result,
            }
        }
    }

    /// The draft differs from what the chart shows.
    pub fn changed(&self) -> bool {
        match (&self.draft, &self.shown) {
            (Some(d), Some(s)) => *d != s.query,
            (Some(_), None) => true,
            _ => false,
        }
    }
}

/// A panel under edit. `target` names the panel it replaces, or is `None`
/// for a new one.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelEditor {
    pub target: Option<String>,
    pub draft: PanelDef,
}

/// A dialog over the Dashboards tab.
#[derive(Debug, Clone, PartialEq)]
pub enum DashDialog {
    /// Save under a name. `then` runs once the save succeeds.
    SaveAs {
        name: String,
        error: Option<String>,
        then: Option<After>,
    },
    Rename {
        name: String,
        error: Option<String>,
    },
    Delete,
    /// Modified work is about to be left.
    Unsaved(After),
    /// The query a panel runs.
    Inspect(String),
    /// A save met a newer revision of the document.
    Conflict,
}

/// What a person asked for while a document had unsaved changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum After {
    Open(String),
    New,
}

/// What persists a document right now. Loading data is separate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SaveState {
    #[default]
    Idle,
    Saving,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DashUi {
    /// The library row that is selected. Opening is a separate step.
    pub selected: Option<String>,
    /// The document the page shows: stored, or an unsaved draft.
    pub open_id: Option<String>,
    /// Counts opens, so reopening the same document reloads it.
    pub open_epoch: u32,
    /// The working copy, and the document as stored when it was opened.
    pub open: Option<Dashboard>,
    pub baseline: Option<Dashboard>,
    pub editor: Option<PanelEditor>,
    pub dialog: Option<DashDialog>,
    /// A panel menu or the library's Manage menu.
    pub menu: Option<String>,
    pub filter: String,
    pub save: SaveState,
    /// The last removed panel and where it was, for Undo.
    pub removed: Option<(usize, PanelDef)>,
    pub notice: Option<Notice>,
    /// Counts library changes, so the list re-reads.
    pub library_epoch: u32,
    /// A scene's request, applied once the document loads.
    pub after_open: Option<AfterOpen>,
    /// A panel to add to whichever document opens next. Explore sets it.
    pub pending_add: Option<PanelDef>,
    /// The selected document should open now.
    pub pending_open: bool,
    /// A new document should open now.
    pub pending_new: bool,
    /// A stored document to copy. The page runs it.
    pub pending_duplicate: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AfterOpen {
    pub resize: Option<(String, u32)>,
    pub edit: Option<String>,
}

impl DashUi {
    pub fn modified(&self) -> bool {
        self.open != self.baseline
    }
}

/// The Lifecycle tab's two dialogs and sheets.
#[derive(Debug, Clone, PartialEq)]
pub enum LifeSheet {
    Policy(RetentionDraft),
    Rollup(RollupDraft),
    Late(LatePolicy),
    Review {
        review: Option<ChangeReview>,
        busy: bool,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RollupTab {
    #[default]
    Definition,
    Runs,
}

impl RollupTab {
    pub const ALL: [RollupTab; 2] = [RollupTab::Definition, RollupTab::Runs];

    pub fn label(self) -> &'static str {
        match self {
            RollupTab::Definition => "Definition",
            RollupTab::Runs => "Runs",
        }
    }
}

/// The backfill fields on the overview, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickBackfill {
    pub start: String,
    pub end: String,
    pub target: String,
}

impl Default for QuickBackfill {
    fn default() -> Self {
        Self {
            start: "2026-06-12 00:00".to_string(),
            end: "2026-06-13 00:00".to_string(),
            target: "sales_hourly".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LifeUi {
    pub collection: Option<String>,
    pub selected: Option<String>,
    pub tab: RollupTab,
    /// Changes waiting for review, in the order they were made.
    pub pending: Vec<LifecycleChange>,
    pub sheet: Option<LifeSheet>,
    pub quick: QuickBackfill,
    pub notice: Option<Notice>,
    /// Counts applied changes and queued refreshes, so reads repeat.
    pub epoch: u32,
}

/// The state of a dry run.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PreviewState {
    /// No preview for the current fields.
    #[default]
    Draft,
    /// Counts dry runs: a reply for another number is dropped.
    Running(u32),
    Ready(BackfillPreview),
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewKind {
    /// Read the job. Nothing can start from here.
    Read,
    /// Read the job, then start it.
    Run,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackfillUi {
    pub spec: BackfillSpec,
    /// The range fields as typed. They parse into `spec.range`.
    pub start: String,
    pub end: String,
    pub preview: PreviewState,
    /// Counts dry runs.
    pub seq: u32,
    pub mode: RunMode,
    /// The review sheet, when open, and whether it offers to run the job.
    pub review: Option<ReviewKind>,
    pub busy: bool,
    pub error: Option<String>,
    /// A job whose details are open.
    pub detail: Option<String>,
    pub notice: Option<Notice>,
    /// Counts job writes, so the history re-reads.
    pub epoch: u32,
    /// Whether the page should run a dry run on its own when it opens.
    pub auto_preview: bool,
}

impl BackfillUi {
    /// The Backfill page before anyone touches it.
    pub fn standard() -> Self {
        let spec = BackfillSpec::standard();
        Self {
            start: crate::models::format::date_minute(spec.range.start, 0),
            end: crate::models::format::date_minute(spec.range.end, 0),
            spec,
            preview: PreviewState::Draft,
            seq: 0,
            mode: RunMode::DryRun,
            review: None,
            busy: false,
            error: None,
            detail: None,
            notice: None,
            epoch: 0,
            auto_preview: true,
        }
    }
}

/// What a screenshot scene sets. `None` keeps a tab's own state.
#[cfg(all(feature = "screenshots", target_os = "linux"))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimeseriesSeed {
    pub explore: Option<ExploreUi>,
    pub dash: Option<DashUi>,
    pub life: Option<LifeUi>,
    pub backfill: Option<BackfillUi>,
}

#[derive(Clone, Copy)]
pub struct TimeseriesUi {
    pub explore: Signal<ExploreUi>,
    pub dash: Signal<DashUi>,
    pub life: Signal<LifeUi>,
    pub backfill: Signal<BackfillUi>,
}

impl TimeseriesUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            explore: Signal::new(ExploreUi::default()),
            dash: Signal::new(DashUi::default()),
            life: Signal::new(LifeUi::default()),
            backfill: Signal::new(BackfillUi::standard()),
        })
    }

    /// Replace the tabs' state with a seed's.
    #[cfg(all(feature = "screenshots", target_os = "linux"))]
    pub fn apply(mut self, seed: TimeseriesSeed) {
        self.explore.set(seed.explore.unwrap_or_default());
        self.dash.set(seed.dash.unwrap_or_default());
        self.life.set(seed.life.unwrap_or_default());
        self.backfill
            .set(seed.backfill.unwrap_or_else(BackfillUi::standard));
    }

    /// Ask the Explore tab to open `collection`, on `time_field` when given.
    /// Another page calls this before it navigates: the tab applies it once
    /// the catalog is read. Explorer's Open in Timeseries is the caller.
    pub fn open_collection(mut self, collection: &str, time_field: Option<&str>) {
        self.explore.write().requested =
            Some((collection.to_string(), time_field.map(str::to_string)));
    }
}
