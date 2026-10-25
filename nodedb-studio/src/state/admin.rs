//! Administration page state, one struct per tab, provided by `App` above
//! the router. A tab keeps its selection, filters, drafts, and open review
//! when the person leaves the page. A draft belongs to the object it edits:
//! choosing another object never discards it. Another surface can point at
//! an object here (the bell opens an audit event) by writing the selection.

use dioxus::prelude::*;

use crate::models::admin::{AdminChange, AdminReview};
use crate::models::admin_access::{AccessChange, PrincipalDraft, PrincipalKind, RoleDraft};
use crate::models::admin_audit::{AuditQuery, AuditRange, AuditResult, TargetType};
use crate::models::admin_backup::ScheduleDraft;
use crate::models::admin_config::{ConfigScope, ConfigTarget};
use crate::models::admin_credential::{CredentialDraft, CredentialTest};
use crate::models::admin_policy::{PolicyDraft, PolicyTestResult};
use crate::state::events::feeds::Notice;

/// An open review: what would change, and whether it is applying.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewState {
    pub change: AdminChange,
    pub review: AdminReview,
    pub busy: bool,
    pub error: Option<String>,
}

impl ReviewState {
    pub fn new(change: AdminChange, review: AdminReview) -> Self {
        Self {
            change,
            review,
            busy: false,
            error: None,
        }
    }
}

/// What the Access list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessMode {
    #[default]
    Principals,
    Roles,
}

impl AccessMode {
    pub const ALL: [AccessMode; 2] = [AccessMode::Principals, AccessMode::Roles];

    pub fn label(self) -> &'static str {
        match self {
            AccessMode::Principals => "Principals",
            AccessMode::Roles => "Roles",
        }
    }
}

/// A form sheet open over the Access page.
#[derive(Debug, Clone, PartialEq)]
pub enum AccessSheet {
    Principal(PrincipalDraft),
    Role(RoleDraft),
    /// Give a principal a role in a database. The side the page is on is
    /// fixed, and the other side is chosen.
    Membership {
        principal: String,
        role: String,
        database: String,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessUi {
    pub mode: AccessMode,
    pub filter: String,
    pub kind: Option<PrincipalKind>,
    /// The chosen principal id. `None` falls back to the first.
    pub principal: Option<String>,
    /// The chosen role name. `None` falls back to the first.
    pub role: Option<String>,
    pub grants_filter: String,
    pub members_filter: String,
    /// Edits waiting for review.
    pub staged: Vec<AccessChange>,
    pub sheet: Option<AccessSheet>,
    pub review: Option<ReviewState>,
    pub notice: Option<Notice>,
    /// Bumps after a write, so reads run again.
    pub epoch: u32,
}

/// A read-only policy test, and where it stands.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TestRun {
    #[default]
    Idle,
    Running,
    Done(PolicyTestResult),
    Failed(String),
}

/// The `PolicyUi::selected` value that means the policy being created.
pub const NEW_POLICY: &str = "+new";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PolicyUi {
    /// `None` is the connection's current database.
    pub database: Option<String>,
    pub filter: String,
    /// The chosen policy id. `None` falls back to the first.
    pub selected: Option<String>,
    /// Edits by policy id. A draft stays while another policy is chosen.
    pub drafts: Vec<PolicyDraft>,
    /// A policy being created. It has no id yet.
    pub creating: Option<PolicyDraft>,
    /// The user the test runs as. `None` falls back to the first.
    pub user: Option<String>,
    /// The `app.tenant_id` value of the test session. `None` is the user's own.
    pub tenant: Option<String>,
    pub test: TestRun,
    pub review: Option<ReviewState>,
    pub notice: Option<Notice>,
    pub epoch: u32,
}

impl PolicyUi {
    pub fn draft_of(&self, id: &str) -> Option<&PolicyDraft> {
        self.drafts.iter().find(|d| d.id.as_deref() == Some(id))
    }

    /// Replace or add the draft of `id`.
    pub fn set_draft(&mut self, draft: PolicyDraft) {
        match draft.id.clone() {
            Some(id) => match self
                .drafts
                .iter_mut()
                .find(|d| d.id.as_deref() == Some(id.as_str()))
            {
                Some(slot) => *slot = draft,
                None => self.drafts.push(draft),
            },
            None => self.creating = Some(draft),
        }
    }

    pub fn drop_draft(&mut self, id: Option<&str>) {
        match id {
            Some(id) => self.drafts.retain(|d| d.id.as_deref() != Some(id)),
            None => self.creating = None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuditUi {
    pub range: AuditRange,
    pub actor: Option<String>,
    pub action: Option<String>,
    pub target: Option<TargetType>,
    pub result: Option<AuditResult>,
    /// Zero-based.
    pub page: usize,
    pub page_size: usize,
    /// The chosen event id. `None` falls back to the first on the page.
    pub selected: Option<String>,
}

impl Default for AuditUi {
    fn default() -> Self {
        Self {
            range: AuditRange::default(),
            actor: None,
            action: None,
            target: None,
            result: None,
            page: 0,
            page_size: 10,
            selected: None,
        }
    }
}

impl AuditUi {
    pub fn query(&self) -> AuditQuery {
        AuditQuery {
            range: self.range,
            actor: self.actor.clone(),
            action: self.action.clone(),
            target: self.target,
            result: self.result,
            page: self.page,
            page_size: self.page_size,
        }
    }

    /// A filter changed: the first page of the new result shows.
    pub fn refilter(&mut self) {
        self.page = 0;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackupsUi {
    /// The chosen snapshot id. `None` falls back to the first.
    pub selected: Option<String>,
    /// The name of the database a restore would create.
    pub restore_target: String,
    /// The schedule form, while it is open.
    pub schedule: Option<ScheduleDraft>,
    pub review: Option<ReviewState>,
    pub notice: Option<Notice>,
    pub epoch: u32,
}

impl Default for BackupsUi {
    fn default() -> Self {
        Self {
            selected: None,
            restore_target: "analytics_restore".to_string(),
            schedule: None,
            review: None,
            notice: None,
            epoch: 0,
        }
    }
}

/// A proposed value for one setting of one target.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposed {
    pub target: ConfigTarget,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConfigUi {
    pub scope: ConfigScope,
    /// A member id, for the Node scope. `None` falls back to the first.
    pub node: Option<String>,
    pub filter: String,
    pub selected: Option<String>,
    /// Proposals of every target. A target keeps its own when another is chosen.
    pub proposed: Vec<Proposed>,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<Notice>,
    pub epoch: u32,
}

impl ConfigUi {
    /// The proposals that belong to `target`, as key and value.
    pub fn changes_for(&self, target: &ConfigTarget) -> Vec<(String, String)> {
        self.proposed
            .iter()
            .filter(|p| p.target == *target)
            .map(|p| (p.key.clone(), p.value.clone()))
            .collect()
    }

    pub fn propose(&mut self, target: &ConfigTarget, key: &str, value: &str, current: &str) {
        self.proposed
            .retain(|p| !(p.target == *target && p.key == key));
        // Choosing the stored value again withdraws the proposal.
        if value != current {
            self.proposed.push(Proposed {
                target: target.clone(),
                key: key.to_string(),
                value: value.to_string(),
            });
        }
    }

    pub fn clear(&mut self, target: &ConfigTarget) {
        self.proposed.retain(|p| p.target != *target);
    }
}

/// A connection test of one credential.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CredTestRun {
    #[default]
    Idle,
    Running,
    Done(CredentialTest),
    Failed(String),
}

/// The `CredUi::selected` value that means the credential being created.
pub const NEW_CREDENTIAL: &str = "+new";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CredUi {
    pub filter: String,
    /// The chosen alias. `None` falls back to the first.
    pub selected: Option<String>,
    /// Edits by alias. A draft stays while another credential is chosen.
    pub drafts: Vec<CredentialDraft>,
    /// A credential being created. It has no stored copy yet.
    pub creating: Option<CredentialDraft>,
    pub test: CredTestRun,
    pub review: Option<ReviewState>,
    pub notice: Option<Notice>,
    pub epoch: u32,
}

impl CredUi {
    pub fn draft_of(&self, alias: &str) -> Option<&CredentialDraft> {
        self.drafts.iter().find(|d| d.alias == alias)
    }

    pub fn set_draft(&mut self, draft: CredentialDraft) {
        match self.drafts.iter_mut().find(|d| d.alias == draft.alias) {
            Some(slot) => *slot = draft,
            None => self.drafts.push(draft),
        }
    }

    pub fn drop_draft(&mut self, alias: &str) {
        self.drafts.retain(|d| d.alias != alias);
    }
}

/// A tab that opens reviews. The shared tasks open, apply, and close a
/// review through this, so no tab repeats them.
pub trait Reviewing {
    fn review(&self) -> &Option<ReviewState>;
    fn review_mut(&mut self) -> &mut Option<ReviewState>;
    fn notice_mut(&mut self) -> &mut Option<Notice>;
    /// A change applied. Drop what it consumed and read again.
    fn applied(&mut self, change: &AdminChange);
}

impl Reviewing for AccessUi {
    fn review(&self) -> &Option<ReviewState> {
        &self.review
    }
    fn review_mut(&mut self) -> &mut Option<ReviewState> {
        &mut self.review
    }
    fn notice_mut(&mut self) -> &mut Option<Notice> {
        &mut self.notice
    }
    fn applied(&mut self, _change: &AdminChange) {
        self.staged.clear();
        self.sheet = None;
        self.epoch += 1;
    }
}

impl Reviewing for PolicyUi {
    fn review(&self) -> &Option<ReviewState> {
        &self.review
    }
    fn review_mut(&mut self) -> &mut Option<ReviewState> {
        &mut self.review
    }
    fn notice_mut(&mut self) -> &mut Option<Notice> {
        &mut self.notice
    }
    fn applied(&mut self, change: &AdminChange) {
        if let AdminChange::Policy(c) = change {
            let draft = c.draft();
            match &draft.id {
                Some(id) => self.drop_draft(Some(id)),
                None => {
                    self.creating = None;
                    // Show the policy that was created.
                    self.selected = None;
                }
            }
        }
        self.test = TestRun::Idle;
        self.epoch += 1;
    }
}

impl Reviewing for BackupsUi {
    fn review(&self) -> &Option<ReviewState> {
        &self.review
    }
    fn review_mut(&mut self) -> &mut Option<ReviewState> {
        &mut self.review
    }
    fn notice_mut(&mut self) -> &mut Option<Notice> {
        &mut self.notice
    }
    fn applied(&mut self, _change: &AdminChange) {
        self.schedule = None;
        self.epoch += 1;
    }
}

impl Reviewing for CredUi {
    fn review(&self) -> &Option<ReviewState> {
        &self.review
    }
    fn review_mut(&mut self) -> &mut Option<ReviewState> {
        &mut self.review
    }
    fn notice_mut(&mut self) -> &mut Option<Notice> {
        &mut self.notice
    }
    fn applied(&mut self, change: &AdminChange) {
        if let AdminChange::Credential(c) = change {
            self.drop_draft(c.alias());
            if matches!(
                c,
                crate::models::admin_credential::CredentialChange::Create(_)
            ) {
                self.creating = None;
                self.selected = Some(c.alias().to_string());
            }
        }
        self.test = CredTestRun::Idle;
        self.epoch += 1;
    }
}

#[derive(Clone, Copy)]
pub struct AdminUi {
    pub access: Signal<AccessUi>,
    pub policies: Signal<PolicyUi>,
    pub audit: Signal<AuditUi>,
    pub backups: Signal<BackupsUi>,
    pub config: Signal<ConfigUi>,
    pub credentials: Signal<CredUi>,
}

/// What a screenshot scene sets. `None` resets a tab to its default.
#[cfg(all(feature = "screenshots", target_os = "linux"))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AdminSeed {
    pub access: Option<AccessUi>,
    pub policies: Option<PolicyUi>,
    pub audit: Option<AuditUi>,
    pub backups: Option<BackupsUi>,
    pub config: Option<ConfigUi>,
    pub credentials: Option<CredUi>,
}

impl AdminUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            access: Signal::new(AccessUi::default()),
            policies: Signal::new(PolicyUi::default()),
            audit: Signal::new(AuditUi::default()),
            backups: Signal::new(BackupsUi::default()),
            config: Signal::new(ConfigUi::default()),
            credentials: Signal::new(CredUi::default()),
        })
    }

    /// Replace every tab's state with a scene's seed.
    #[cfg(all(feature = "screenshots", target_os = "linux"))]
    pub fn apply(mut self, seed: AdminSeed) {
        self.access.set(seed.access.unwrap_or_default());
        self.policies.set(seed.policies.unwrap_or_default());
        self.audit.set(seed.audit.unwrap_or_default());
        self.backups.set(seed.backups.unwrap_or_default());
        self.config.set(seed.config.unwrap_or_default());
        self.credentials.set(seed.credentials.unwrap_or_default());
    }
}

/// The item to show: the chosen one while it is listed, else the first.
pub fn pick<'a, T>(list: &'a [T], chosen: Option<&str>, key: impl Fn(&T) -> &str) -> Option<&'a T> {
    chosen
        .and_then(|c| list.iter().find(|x| key(x) == c))
        .or_else(|| list.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_policy::PolicyOperation;

    fn target(node: &str) -> ConfigTarget {
        ConfigTarget {
            scope: ConfigScope::Node,
            node: Some(node.into()),
        }
    }

    #[test]
    fn a_proposal_belongs_to_its_target() {
        let mut ui = ConfigUi::default();
        ui.propose(&target("nd_02"), "query_timeout", "60s", "30s");
        ui.propose(&target("nd_03"), "log_level", "debug", "info");
        assert_eq!(ui.changes_for(&target("nd_02")).len(), 1);
        assert_eq!(ui.changes_for(&target("nd_03")).len(), 1);
        ui.clear(&target("nd_02"));
        assert!(ui.changes_for(&target("nd_02")).is_empty());
        assert_eq!(ui.changes_for(&target("nd_03")).len(), 1);
    }

    #[test]
    fn choosing_the_stored_value_withdraws_the_proposal() {
        let mut ui = ConfigUi::default();
        ui.propose(&target("nd_02"), "query_timeout", "60s", "30s");
        ui.propose(&target("nd_02"), "query_timeout", "30s", "30s");
        assert!(ui.changes_for(&target("nd_02")).is_empty());
    }

    #[test]
    fn a_policy_draft_survives_choosing_another_policy() {
        let mut ui = PolicyUi::default();
        let mut d = PolicyDraft::blank("analytics", "analytics.sales", "tenant_reader");
        d.id = Some("pol_01".into());
        d.operation = PolicyOperation::Delete;
        ui.set_draft(d.clone());
        ui.selected = Some("pol_02".into());
        assert_eq!(ui.draft_of("pol_01"), Some(&d));
        ui.drop_draft(Some("pol_01"));
        assert!(ui.draft_of("pol_01").is_none());
    }

    #[test]
    fn the_first_item_stands_in_for_a_missing_choice() {
        let list = ["a", "b"];
        assert_eq!(pick(&list, Some("b"), |s| s), Some(&"b"));
        assert_eq!(pick(&list, Some("gone"), |s| s), Some(&"a"));
        assert_eq!(pick(&list, None, |s| s), Some(&"a"));
    }
}
