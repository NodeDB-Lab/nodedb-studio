//! Sync page state, one struct per tab, provided by `App` above the router.
//! A tab keeps its selection, filters, and open sheet when the person leaves
//! the page, and another surface can point at an object here (the bell opens
//! a rejected write, an activity event opens its device) by writing the
//! selection.

use dioxus::prelude::*;

use crate::models::cluster_activity::ActivityRange;
use crate::models::sync::{Device, DeviceState};
use crate::models::sync_activity::{SyncEvent, SyncEventKind, SyncQuery};
use crate::models::sync_enroll::EnrollDraft;
use crate::models::sync_reject::{RejectReason, RejectState, Rejection};
use crate::models::sync_sub::{SubChange, Subscription};
use crate::state::events::feeds::Notice;

/// Devices: the chosen device, the filters, and the open enrollment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DevicesUi {
    pub filter: String,
    pub state: Option<DeviceState>,
    /// The chosen device id. `None` falls back to the first.
    pub selected: Option<String>,
    pub enroll: Option<EnrollDraft>,
    pub notice: Option<Notice>,
    /// Bumps after a write, so reads run again.
    pub epoch: u32,
}

/// The device the details panel shows: the chosen one while it is listed,
/// else the first.
pub fn pick_device<'a>(list: &'a [Device], chosen: Option<&str>) -> Option<&'a Device> {
    chosen
        .and_then(|id| list.iter().find(|d| d.id == id))
        .or_else(|| list.first())
}

/// The devices the filters keep.
pub fn matching_devices<'a>(
    list: &'a [Device],
    text: &str,
    state: Option<DeviceState>,
) -> Vec<&'a Device> {
    let needle = text.trim().to_lowercase();
    list.iter()
        .filter(|d| state.is_none_or(|s| d.state == s))
        .filter(|d| d.matches(&needle))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubTab {
    #[default]
    Definition,
    EstimatedScope,
    ReviewChange,
}

impl SubTab {
    pub const ALL: [SubTab; 3] = [
        SubTab::Definition,
        SubTab::EstimatedScope,
        SubTab::ReviewChange,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SubTab::Definition => "Definition",
            SubTab::EstimatedScope => "Estimated scope",
            SubTab::ReviewChange => "Review change",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// An edit of one subscription's definition, kept until applied or reset.
#[derive(Debug, Clone, PartialEq)]
pub struct SubDraft {
    pub id: String,
    pub change: SubChange,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SubsUi {
    pub filter: String,
    /// A device id. `None` lists every device's subscriptions.
    pub device: Option<String>,
    pub selected: Option<String>,
    pub tab: SubTab,
    pub draft: Option<SubDraft>,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<Notice>,
    pub epoch: u32,
}

impl SubsUi {
    /// The draft of `id`, if one is open for it.
    pub fn draft_of(&self, id: &str) -> Option<&SubChange> {
        self.draft
            .as_ref()
            .filter(|d| d.id == id)
            .map(|d| &d.change)
    }

    /// Whether `sub` has an edit that differs from what it stores.
    pub fn dirty(&self, sub: &Subscription) -> bool {
        self.draft_of(&sub.id)
            .is_some_and(|c| *c != sub.as_change())
    }
}

/// The subscriptions the filters keep.
pub fn matching_subs<'a>(
    list: &'a [Subscription],
    text: &str,
    device: Option<&str>,
) -> Vec<&'a Subscription> {
    let needle = text.trim().to_lowercase();
    list.iter()
        .filter(|s| device.is_none_or(|d| s.device_id == d))
        .filter(|s| s.matches(&needle))
        .collect()
}

/// The subscription to show: the chosen one while it is listed, else the first.
pub fn pick_sub<'a>(list: &[&'a Subscription], chosen: Option<&str>) -> Option<&'a Subscription> {
    chosen
        .and_then(|id| list.iter().find(|s| s.id == id))
        .or_else(|| list.first())
        .copied()
}

/// An open sheet over the Rejected writes page.
#[derive(Debug, Clone, PartialEq)]
pub enum RepairSheet {
    /// Edit the payload and submit it as a new write.
    Edit {
        mutation: String,
        /// The revision the rejection read when the sheet opened.
        revision: u64,
        text: String,
        busy: bool,
        error: Option<String>,
    },
    /// Review what discarding the pending copy does.
    Discard {
        mutation: String,
        revision: u64,
        busy: bool,
        error: Option<String>,
    },
    /// Where the repair stands.
    Compensation { mutation: String },
}

impl RepairSheet {
    pub fn mutation(&self) -> &str {
        match self {
            RepairSheet::Edit { mutation, .. }
            | RepairSheet::Discard { mutation, .. }
            | RepairSheet::Compensation { mutation } => mutation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RejectUi {
    pub filter: String,
    /// `database.collection`.
    pub source: Option<String>,
    pub reason: Option<RejectReason>,
    pub state: Option<RejectState>,
    pub selected: Option<String>,
    pub sheet: Option<RepairSheet>,
    /// The authoritative record section is open.
    pub existing_open: bool,
    pub notice: Option<Notice>,
    pub epoch: u32,
}

/// The rejections the filters keep.
pub fn matching_rejects<'a>(list: &'a [Rejection], ui: &RejectUi) -> Vec<&'a Rejection> {
    let needle = ui.filter.trim().to_lowercase();
    list.iter()
        .filter(|r| ui.source.as_deref().is_none_or(|s| r.source() == s))
        .filter(|r| ui.reason.is_none_or(|x| r.reason == x))
        .filter(|r| ui.state.is_none_or(|x| r.state == x))
        .filter(|r| r.matches(&needle))
        .collect()
}

/// The rejection to show: the chosen one while it is listed, else the first.
pub fn pick_reject<'a>(list: &[&'a Rejection], chosen: Option<&str>) -> Option<&'a Rejection> {
    chosen
        .and_then(|m| list.iter().find(|r| r.mutation == m))
        .or_else(|| list.first())
        .copied()
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncActivityUi {
    pub range: ActivityRange,
    pub device: Option<String>,
    pub kind: Option<SyncEventKind>,
    pub text: String,
    pub selected: Option<String>,
    pub tick: u32,
}

impl SyncActivityUi {
    pub fn query(&self) -> SyncQuery {
        SyncQuery {
            range: self.range,
            device: self.device.clone(),
            kind: self.kind,
        }
    }
}

/// The events the search text keeps.
pub fn matching_events<'a>(events: &'a [SyncEvent], text: &str) -> Vec<&'a SyncEvent> {
    let needle = text.trim().to_lowercase();
    events.iter().filter(|e| e.matches(&needle)).collect()
}

/// The event to show: the chosen one while it is listed, else the first.
pub fn pick_event<'a>(list: &[&'a SyncEvent], chosen: Option<&str>) -> Option<&'a SyncEvent> {
    chosen
        .and_then(|id| list.iter().find(|e| e.id == id))
        .or_else(|| list.first())
        .copied()
}

#[derive(Clone, Copy)]
pub struct SyncUi {
    pub devices: Signal<DevicesUi>,
    pub subs: Signal<SubsUi>,
    pub rejects: Signal<RejectUi>,
    pub activity: Signal<SyncActivityUi>,
}

/// What a screenshot scene sets. `None` resets a tab to its default.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncSeed {
    pub devices: Option<DevicesUi>,
    pub subs: Option<SubsUi>,
    pub rejects: Option<RejectUi>,
    pub activity: Option<SyncActivityUi>,
}

impl SyncUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            devices: Signal::new(DevicesUi::default()),
            subs: Signal::new(SubsUi::default()),
            rejects: Signal::new(RejectUi::default()),
            activity: Signal::new(SyncActivityUi::default()),
        })
    }

    /// Replace every tab's state with a scene's seed.
    pub fn apply(mut self, seed: SyncSeed) {
        self.devices.set(seed.devices.unwrap_or_default());
        self.subs.set(seed.subs.unwrap_or_default());
        self.rejects.set(seed.rejects.unwrap_or_default());
        self.activity.set(seed.activity.unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::SyncStore;

    #[test]
    fn a_state_filter_and_text_narrow_the_devices() {
        let all = SyncStore::seeded().devices().devices;
        assert_eq!(matching_devices(&all, "", None).len(), all.len());
        let offline = matching_devices(&all, "", Some(DeviceState::Offline));
        assert!(offline.iter().all(|d| d.state == DeviceState::Offline));
        assert_eq!(matching_devices(&all, "CHECKOUT-IPAD", None).len(), 1);
    }

    #[test]
    fn a_missing_selection_falls_back_never_shows_stale() {
        let all = SyncStore::seeded().devices().devices;
        assert_eq!(
            pick_device(&all, Some("d17")).map(|d| d.id.as_str()),
            Some("d17")
        );
        assert_eq!(
            pick_device(&all, Some("gone")).map(|d| d.id.as_str()),
            Some("lite_007")
        );
    }

    #[test]
    fn a_draft_belongs_to_one_subscription() {
        let store = SyncStore::seeded();
        let subs = store.subscriptions().items;
        let mut change = subs[0].as_change();
        change.parameters[0].1 = "sf".into();
        let ui = SubsUi {
            draft: Some(SubDraft {
                id: subs[0].id.clone(),
                change,
            }),
            ..SubsUi::default()
        };
        assert!(ui.dirty(&subs[0]));
        assert!(!ui.dirty(&subs[1]));
    }

    #[test]
    fn rejection_filters_combine() {
        let rows = SyncStore::seeded().rejected().rows;
        let ui = RejectUi {
            reason: Some(RejectReason::UniqueViolation),
            source: Some("analytics.orders".into()),
            ..RejectUi::default()
        };
        let kept = matching_rejects(&rows, &ui);
        assert_eq!(kept.len(), 2);
        assert_eq!(
            pick_reject(&kept, None).map(|r| r.mutation.as_str()),
            Some("m104")
        );
    }
}
