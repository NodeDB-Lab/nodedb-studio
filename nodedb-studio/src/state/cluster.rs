//! Cluster page state, one struct per tab, provided by `App` above the
//! router. A tab keeps its selection, filters, and open review when the
//! person leaves the page, and another tab can point at an object here (an
//! activity event opens its Raft group) by writing the selection.

use dioxus::prelude::*;

use crate::models::cluster_activity::{ActivityQuery, ActivityRange, ClusterEvent, Severity};
use crate::models::cluster_monitor::TelemetryRange;
use crate::models::cluster_place::{MoveRequest, Placement, RebalancePlan, ShardStatus, VShard};
use crate::models::cluster_raft::TransferPreview;
use crate::models::ts_dashboard::Refresh;
use crate::state::events::feeds::Notice;

/// Nodes: the chosen member and group. `None` falls back to the first.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodesUi {
    pub node: Option<String>,
    pub group: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RaftTab {
    #[default]
    Overview,
    Configuration,
    Snapshots,
}

impl RaftTab {
    pub const ALL: [RaftTab; 3] = [
        RaftTab::Overview,
        RaftTab::Configuration,
        RaftTab::Snapshots,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RaftTab::Overview => "Overview",
            RaftTab::Configuration => "Configuration",
            RaftTab::Snapshots => "Snapshots",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// The open leadership transfer review.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferReview {
    pub preview: TransferPreview,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RaftUi {
    pub filter: String,
    /// The chosen group id. `None` falls back to the first needing attention.
    pub group: Option<String>,
    pub tab: RaftTab,
    /// The member id chosen as the leadership target.
    pub target: Option<String>,
    pub review: Option<TransferReview>,
    pub notice: Option<Notice>,
    /// A snapshot is being taken.
    pub snapshotting: bool,
    /// Bumps after a write, so reads run again.
    pub epoch: u32,
}

/// The open rebalance plan review.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanReview {
    pub plan: RebalancePlan,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlaceUi {
    pub filter: String,
    pub database: Option<String>,
    pub collection: Option<String>,
    pub vshard: Option<u32>,
    /// The replica move being composed: member ids.
    pub from: Option<String>,
    pub to: Option<String>,
    /// Moves staged into the plan, one per group.
    pub staged: Vec<MoveRequest>,
    pub review: Option<PlanReview>,
    pub notice: Option<Notice>,
    pub epoch: u32,
}

impl PlaceUi {
    /// Pick a database. A collection of another database no longer applies.
    pub fn set_database(&mut self, database: Option<String>, placement: &Placement) {
        self.database = database;
        let keep = self
            .collection
            .as_ref()
            .is_some_and(|c| placement.collections(self.database.as_deref()).contains(c));
        if !keep {
            self.collection = None;
        }
    }

    /// Stage the composed move. A second move in the same group replaces the
    /// first: a group changes membership once per plan.
    pub fn stage(&mut self, request: MoveRequest, placement: &Placement) {
        let group = placement.vshard(request.vshard).map(|v| v.group.clone());
        self.staged
            .retain(|m| placement.vshard(m.vshard).map(|v| v.group.clone()) != group);
        self.staged.push(request);
        self.from = None;
        self.to = None;
    }

    /// The vShards the filters keep, by number.
    pub fn visible<'a>(&self, placement: &'a Placement) -> Vec<&'a VShard> {
        let needle = self.filter.trim().to_lowercase();
        let mut out: Vec<&VShard> = placement
            .vshards
            .iter()
            .filter(|v| self.database.as_deref().is_none_or(|d| v.database == d))
            .filter(|v| self.collection.as_deref().is_none_or(|c| v.collection == c))
            .filter(|v| {
                needle.is_empty()
                    || [
                        v.number.to_string(),
                        v.group.clone(),
                        v.leader.clone(),
                        v.source(),
                    ]
                    .iter()
                    .any(|f| f.to_lowercase().contains(&needle))
            })
            .collect();
        out.sort_by_key(|v| v.number);
        out
    }
}

/// The vShard to show: the chosen one while it is listed, else the first that
/// needs attention, else the first.
pub fn pick_vshard<'a>(list: &[&'a VShard], chosen: Option<u32>) -> Option<&'a VShard> {
    chosen
        .and_then(|n| list.iter().find(|v| v.number == n))
        .or_else(|| list.iter().find(|v| v.status() != ShardStatus::Placed))
        .or_else(|| list.first())
        .copied()
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MonitorUi {
    /// A member id. `None` is every node together.
    pub node: Option<String>,
    pub range: TelemetryRange,
    pub refresh: Refresh,
    /// Bumps on each refresh tick and on Refresh.
    pub tick: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityUi {
    pub range: ActivityRange,
    pub severity: Option<Severity>,
    pub node: Option<String>,
    pub group: Option<String>,
    pub text: String,
    pub selected: Option<String>,
    pub auto_refresh: bool,
    pub raw_open: bool,
    pub tick: u32,
}

impl Default for ActivityUi {
    fn default() -> Self {
        Self {
            range: ActivityRange::default(),
            severity: None,
            node: None,
            group: None,
            text: String::new(),
            selected: None,
            auto_refresh: true,
            raw_open: false,
            tick: 0,
        }
    }
}

impl ActivityUi {
    pub fn query(&self) -> ActivityQuery {
        ActivityQuery {
            range: self.range,
            severity: self.severity,
            node: self.node.clone(),
            group: self.group.clone(),
        }
    }
}

/// The events the search text keeps.
pub fn matching<'a>(events: &'a [ClusterEvent], text: &str) -> Vec<&'a ClusterEvent> {
    let needle = text.trim().to_lowercase();
    events.iter().filter(|e| e.matches(&needle)).collect()
}

/// The event to show: the chosen one while it is listed, else the first.
pub fn pick_event<'a>(list: &[&'a ClusterEvent], chosen: Option<&str>) -> Option<&'a ClusterEvent> {
    chosen
        .and_then(|id| list.iter().find(|e| e.id == id))
        .or_else(|| list.first())
        .copied()
}

#[derive(Clone, Copy)]
pub struct ClusterUi {
    pub nodes: Signal<NodesUi>,
    pub raft: Signal<RaftUi>,
    pub place: Signal<PlaceUi>,
    pub monitor: Signal<MonitorUi>,
    pub activity: Signal<ActivityUi>,
}

/// What a screenshot scene sets. `None` resets a tab to its default.
#[cfg(all(feature = "screenshots", target_os = "linux"))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClusterSeed {
    pub nodes: Option<NodesUi>,
    pub raft: Option<RaftUi>,
    pub place: Option<PlaceUi>,
    pub monitor: Option<MonitorUi>,
    pub activity: Option<ActivityUi>,
}

impl ClusterUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            nodes: Signal::new(NodesUi::default()),
            raft: Signal::new(RaftUi::default()),
            place: Signal::new(PlaceUi::default()),
            monitor: Signal::new(MonitorUi::default()),
            activity: Signal::new(ActivityUi::default()),
        })
    }

    /// Replace every tab's state with a scene's seed.
    #[cfg(all(feature = "screenshots", target_os = "linux"))]
    pub fn apply(mut self, seed: ClusterSeed) {
        self.nodes.set(seed.nodes.unwrap_or_default());
        self.raft.set(seed.raft.unwrap_or_default());
        self.place.set(seed.place.unwrap_or_default());
        self.monitor.set(seed.monitor.unwrap_or_default());
        self.activity.set(seed.activity.unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::ClusterStore;

    fn placement() -> Placement {
        ClusterStore::seeded().placement()
    }

    #[test]
    fn filters_narrow_the_vshards_and_sort_by_number() {
        let p = placement();
        let mut ui = PlaceUi::default();
        let all = ui.visible(&p).len();
        ui.set_database(Some("analytics".into()), &p);
        ui.collection = Some("orders".into());
        let orders = ui.visible(&p);
        assert!(orders.len() < all);
        assert!(orders.windows(2).all(|w| w[0].number < w[1].number));
        ui.filter = "DATA-12".into();
        assert!(ui.visible(&p).iter().all(|v| v.group == "data-12"));
    }

    #[test]
    fn a_collection_of_another_database_is_dropped() {
        let p = placement();
        let mut ui = PlaceUi {
            collection: Some("accounts".into()),
            ..PlaceUi::default()
        };
        ui.set_database(Some("analytics".into()), &p);
        assert_eq!(ui.collection, None);
        ui.set_database(Some("crm".into()), &p);
        ui.collection = Some("accounts".into());
        ui.set_database(Some("crm".into()), &p);
        assert_eq!(ui.collection.as_deref(), Some("accounts"));
    }

    #[test]
    fn the_default_vshard_is_the_first_needing_attention() {
        let p = placement();
        let ui = PlaceUi::default();
        let list = ui.visible(&p);
        let pick = pick_vshard(&list, None).map(|v| v.number);
        // vShard 3 is placed. vShard 8 is the first with an unreachable replica.
        assert_eq!(pick, Some(8));
        assert_eq!(pick_vshard(&list, Some(17)).map(|v| v.number), Some(17));
        // A vShard no longer listed falls back, never shows stale.
        assert_eq!(pick_vshard(&list, Some(999)).map(|v| v.number), Some(8));
    }

    #[test]
    fn staging_replaces_a_move_in_the_same_group() {
        let p = placement();
        let mut ui = PlaceUi::default();
        let mv = |vshard, from: &str, to: &str| MoveRequest {
            vshard,
            from_id: from.into(),
            to_id: to.into(),
        };
        ui.stage(mv(17, "nd_05", "nd_01"), &p);
        // vShard 21 is in the same group as 17.
        ui.stage(mv(21, "nd_04", "nd_03"), &p);
        assert_eq!(ui.staged.len(), 1);
        assert_eq!(ui.staged[0].vshard, 21);
        ui.stage(mv(3, "nd_04", "nd_03"), &p);
        assert_eq!(ui.staged.len(), 2);
    }

    #[test]
    fn search_and_selection_over_events() {
        let log = ClusterStore::seeded().activity(&ActivityQuery::default());
        let hits = matching(&log.events, "snapshot");
        assert!(hits.iter().all(|e| e.kind.label().contains("Snapshot")));
        let pick = pick_event(&hits, Some("evt_none"));
        assert_eq!(
            pick.map(|e| e.id.clone()),
            hits.first().map(|e| e.id.clone())
        );
        let all = matching(&log.events, " ");
        assert_eq!(all.len(), log.events.len());
    }

    #[test]
    fn raft_tabs_index_themselves() {
        for (i, t) in RaftTab::ALL.iter().enumerate() {
            assert_eq!(t.index(), i);
        }
    }
}
