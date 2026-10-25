//! The Designer's selection, the open drawers, and the canvas zoom.
//!
//! Provided by `App` above the router, so leaving the page keeps what is
//! open. The staged changes themselves live behind the backend seam: this
//! state only says what the user is looking at.

use std::collections::BTreeSet;

use crate::models::designer_form::{IndexForm, ReferenceForm};
use crate::models::designer_new::{ConstraintKind, NewCollection};

/// What the canvas selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Collection(String),
    Index(String),
    /// A reference, by its id (a staged one by its change id).
    Reference(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Definition,
    Ddl,
}

impl InspectorTab {
    pub const ALL: [InspectorTab; 2] = [InspectorTab::Definition, InspectorTab::Ddl];

    pub fn label(self) -> &'static str {
        match self {
            InspectorTab::Definition => "Definition",
            InspectorTab::Ddl => "DDL",
        }
    }
}

/// What Properties shows: the selection, or a form that stages a new object.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Inspector {
    #[default]
    Selection,
    NewIndex(IndexForm),
    NewReference(ReferenceForm),
}

/// The Create collection drawer.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateState {
    pub form: NewCollection,
    /// The constraint group open for editing.
    pub open: Option<ConstraintKind>,
    /// Whether Review DDL has been pressed, so the form shows its issues.
    pub tried: bool,
    pub busy: bool,
    /// The server's reason when staging failed.
    pub error: Option<String>,
}

impl CreateState {
    pub fn new(database: &str) -> Self {
        Self {
            form: NewCollection::blank(database),
            open: None,
            tried: false,
            busy: false,
            error: None,
        }
    }
}

/// A section of the review panel the tab row jumps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReviewTab {
    #[default]
    Ddl,
    Impact,
    DryRun,
    Permissions,
}

impl ReviewTab {
    pub const ALL: [ReviewTab; 4] = [
        ReviewTab::Ddl,
        ReviewTab::Impact,
        ReviewTab::DryRun,
        ReviewTab::Permissions,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ReviewTab::Ddl => "DDL",
            ReviewTab::Impact => "Impact",
            ReviewTab::DryRun => "Dry run",
            ReviewTab::Permissions => "Permissions",
        }
    }

    /// The element id of the section the tab jumps to.
    pub fn anchor(self) -> &'static str {
        match self {
            ReviewTab::Ddl => "review-ddl",
            ReviewTab::Impact => "review-impact",
            ReviewTab::DryRun => "review-dry-run",
            ReviewTab::Permissions => "review-permissions",
        }
    }
}

/// How the last apply ended, for the review panel to state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyResult {
    /// Every staged change applied.
    Applied(usize),
    /// Some applied, then one did not. The rest stay staged.
    Partial {
        applied: usize,
        failed: String,
        reason: String,
    },
    Failed(String),
    /// The staged set or the catalog changed after the review. Nothing was
    /// applied, and the review is read again on request.
    Stale(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReviewState {
    pub tab: ReviewTab,
    /// Staged changes whose row is open.
    pub open: BTreeSet<String>,
    pub busy: bool,
    pub result: Option<ApplyResult>,
}

pub const ZOOM_STEPS: [u16; 9] = [50, 60, 70, 80, 90, 100, 110, 125, 150];

#[derive(Debug, Clone, PartialEq)]
pub struct DesignerState {
    pub selected: Option<Target>,
    pub tab: InspectorTab,
    pub inspector: Inspector,
    /// The canvas scale, in percent.
    pub zoom: u16,
    pub create: Option<CreateState>,
    pub review: Option<ReviewState>,
    /// Bumped after a write, so the reads that depend on it run again.
    pub epoch: u32,
    /// One line about the last action: a copy, a refusal.
    pub notice: Option<String>,
    /// The error under the Properties form, from the last staging.
    pub form_error: Option<String>,
}

impl Default for DesignerState {
    fn default() -> Self {
        Self {
            selected: None,
            tab: InspectorTab::Definition,
            inspector: Inspector::Selection,
            zoom: 100,
            create: None,
            review: None,
            epoch: 0,
            notice: None,
            form_error: None,
        }
    }
}

impl DesignerState {
    pub fn zoom_in(&mut self) {
        self.zoom = ZOOM_STEPS
            .iter()
            .copied()
            .find(|z| *z > self.zoom)
            .unwrap_or(self.zoom);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = ZOOM_STEPS
            .iter()
            .rev()
            .copied()
            .find(|z| *z < self.zoom)
            .unwrap_or(self.zoom);
    }

    /// The largest step at which a `world` of this size fits `view`, never
    /// above 100%.
    pub fn fit(&mut self, world: (f64, f64), view: (f64, f64)) {
        let scale = (view.0 / world.0).min(view.1 / world.1) * 100.0;
        self.zoom = ZOOM_STEPS
            .iter()
            .rev()
            .copied()
            .find(|z| f64::from(*z) <= scale.min(100.0))
            .unwrap_or(ZOOM_STEPS[0]);
    }

    pub fn select(&mut self, target: Target) {
        self.selected = Some(target);
        self.inspector = Inspector::Selection;
        self.form_error = None;
    }

    /// Note that a write happened, so dependent reads run again.
    pub fn bump(&mut self) {
        self.epoch += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_stop_at_the_ends() {
        let mut s = DesignerState::default();
        for _ in 0..20 {
            s.zoom_in();
        }
        assert_eq!(s.zoom, 150);
        for _ in 0..20 {
            s.zoom_out();
        }
        assert_eq!(s.zoom, 50);
    }

    #[test]
    fn fit_never_enlarges_past_actual_size() {
        let mut s = DesignerState::default();
        s.fit((800.0, 600.0), (1600.0, 1200.0));
        assert_eq!(s.zoom, 100);
        s.fit((1000.0, 800.0), (700.0, 640.0));
        assert_eq!(s.zoom, 70);
        s.fit((5000.0, 4000.0), (700.0, 640.0));
        assert_eq!(s.zoom, 50);
    }

    #[test]
    fn selecting_closes_a_form() {
        let mut s = DesignerState {
            inspector: Inspector::NewReference(ReferenceForm {
                from_collection: String::new(),
                from_field: String::new(),
                to_collection: String::new(),
                to_field: String::new(),
            }),
            form_error: Some("x".into()),
            ..DesignerState::default()
        };
        s.select(Target::Collection("users".into()));
        assert_eq!(s.inspector, Inspector::Selection);
        assert!(s.form_error.is_none());
    }
}
