//! Access: principals, roles, the grants a role carries, and the memberships
//! that give a principal its roles.
//!
//! A privilege names a collection, or every collection (`EVERY`). A
//! principal that holds every right (a superuser, a tenant administrator, a
//! collection owner) shows as an explicit `ALL` row, never as an empty
//! list. The effective view tells whether each row is held directly or
//! inherited from a parent role.

use crate::models::admin::{identifier_issue, parse_date};
use crate::models::admin_password::{Secret, confirm_issue, password_issue};
use crate::models::feeds::Tone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    User,
    ServiceAccount,
}

impl PrincipalKind {
    pub const ALL: [PrincipalKind; 2] = [PrincipalKind::User, PrincipalKind::ServiceAccount];

    pub fn label(self) -> &'static str {
        match self {
            PrincipalKind::User => "User",
            PrincipalKind::ServiceAccount => "Service account",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalStatus {
    Active,
    // The server reports it. No fixture holds a disabled principal.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::admin builds it from a disabled server principal; that service is a stub today"
        )
    )]
    Disabled,
}

impl PrincipalStatus {
    pub fn label(self) -> &'static str {
        match self {
            PrincipalStatus::Active => "Active",
            PrincipalStatus::Disabled => "Disabled",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            PrincipalStatus::Active => Tone::Ok,
            PrincipalStatus::Disabled => Tone::Idle,
        }
    }
}

/// Whether a second factor is required to sign in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mfa {
    // The server reports it. No fixture requires a second factor.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::admin builds it from the server's principal record; that service is a stub today"
        )
    )]
    Required,
    NotRequired,
    /// A service account signs in with a key, so a second factor has no meaning.
    NotApplicable,
    /// The server does not report it.
    NotReported,
}

impl Mfa {
    pub fn label(self) -> &'static str {
        match self {
            Mfa::Required => "Required",
            Mfa::NotRequired => "Not required",
            Mfa::NotApplicable => "Not applicable",
            Mfa::NotReported => NOT_REPORTED,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// Stable id. A name can be reused after a principal is dropped. An id never is.
    pub id: String,
    pub name: String,
    pub kind: PrincipalKind,
    pub status: PrincipalStatus,
    pub description: String,
    /// When the sign-in credential expires, seconds since the epoch. `None`:
    /// the server does not report it.
    pub credential_expiry: Option<i64>,
    /// `None`: the server does not report it.
    pub created_at: Option<i64>,
    /// A user signs in with a password. A service account has none. `None`:
    /// the server does not report it.
    pub password: Option<bool>,
    pub mfa: Mfa,
}

impl Principal {
    /// Whether `needle` (lowercase) appears in a field the table shows.
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [&self.name, &self.description, &self.id]
                .iter()
                .any(|f| f.to_lowercase().contains(needle))
    }
}

/// What the views print for a value the server does not report.
pub const NOT_REPORTED: &str = "Not reported";

/// Stands for every database, or every collection, in a grant row.
pub const EVERY: &str = "*";

/// One right on a collection. The server's permission set is closed:
/// read, write, create, drop, alter, admin, monitor, execute, backup.
/// `All` is every right at once: what a superuser, a tenant administrator
/// or a collection owner holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privilege {
    Select,
    Insert,
    Update,
    Delete,
    Create,
    Drop,
    Alter,
    Admin,
    Monitor,
    Execute,
    Backup,
    All,
}

impl Privilege {
    pub fn label(self) -> &'static str {
        match self {
            Privilege::Select => "SELECT",
            Privilege::Insert => "INSERT",
            Privilege::Update => "UPDATE",
            Privilege::Delete => "DELETE",
            Privilege::Create => "CREATE",
            Privilege::Drop => "DROP",
            Privilege::Alter => "ALTER",
            Privilege::Admin => "ADMIN",
            Privilege::Monitor => "MONITOR",
            Privilege::Execute => "EXECUTE",
            Privilege::Backup => "BACKUP",
            Privilege::All => "ALL",
        }
    }
}

/// Where a grant row applies, for display. `None` for the database: the
/// server does not report it.
pub fn scope_text(database: Option<&str>, target: &str) -> String {
    let Some(db) = database else {
        return format!("{target} (database not reported)");
    };
    match (db == EVERY, target == EVERY) {
        (true, true) => "Every collection in every database".to_string(),
        (false, true) => format!("Every collection in {db}"),
        (true, false) => format!("{target} in every database"),
        (false, false) => format!("{db}.{target}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub name: String,
    pub description: String,
    /// Roles this one inherits grants from.
    pub parents: Vec<String>,
    /// `None`: the server does not report it (a built-in role).
    pub created_at: Option<i64>,
}

impl Role {
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [&self.name, &self.description]
                .iter()
                .any(|f| f.to_lowercase().contains(needle))
    }
}

/// One privilege a role holds on one collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleGrant {
    pub role: String,
    /// `EVERY` for every database. `None`: the server does not report it.
    pub database: Option<String>,
    /// `EVERY` for every collection.
    pub collection: String,
    pub privilege: Privilege,
    /// `None`: the server does not report it.
    pub granted_by: Option<String>,
    /// `None`: the server does not report it.
    pub granted_at: Option<i64>,
}

/// A principal holds a role within one database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub principal: String,
    pub role: String,
    /// Empty when the role applies in every database.
    pub database: String,
    /// How many collections the role's own grants cover in the database.
    pub collections: u32,
}

/// A principal that holds a role, for the role's side of the membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub principal: String,
    pub name: String,
    pub database: String,
}

/// One privilege a subject holds, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveGrant {
    /// `EVERY` for every database. `None`: the server does not report it.
    pub database: Option<String>,
    /// `EVERY` for every collection.
    pub target: String,
    pub privilege: Privilege,
    /// The role the subject holds that carries the privilege.
    pub source: String,
    /// The parent role the privilege comes from, when it is inherited.
    pub via: Option<String>,
}

impl EffectiveGrant {
    pub fn is_direct(&self) -> bool {
        self.via.is_none()
    }

    pub fn kind_label(&self) -> &'static str {
        if self.is_direct() {
            "Direct"
        } else {
            "Inherited"
        }
    }
}

/// Who the access views describe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessSubject {
    Principal(String),
    Role(String),
}

/// Everything the access views read about one subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectAccess {
    pub subject: AccessSubject,
    /// The roles a principal holds. Empty for a role.
    pub memberships: Vec<Membership>,
    /// The principals that hold a role. Empty for a principal.
    pub members: Vec<Member>,
    /// The grants of the roles involved.
    pub grants: Vec<RoleGrant>,
    pub effective: Vec<EffectiveGrant>,
    /// Access the server holds but does not report, one sentence each.
    /// The lists above leave it out.
    pub unreported: Vec<String>,
    pub observed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessDirectory {
    pub principals: Vec<Principal>,
    pub roles: Vec<Role>,
    pub databases: Vec<String>,
    /// The one tenant the lists cover: other tenants' principals and roles
    /// are left out. `None` when the lists are not cut to a tenant.
    pub tenant: Option<u64>,
    pub observed_at: i64,
}

impl AccessDirectory {
    pub fn principal(&self, id: &str) -> Option<&Principal> {
        self.principals.iter().find(|p| p.id == id)
    }

    pub fn name_of(&self, id: &str) -> String {
        self.principal(id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| id.to_string())
    }
}

/// What the person typed for a new principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalDraft {
    pub name: String,
    pub kind: PrincipalKind,
    pub description: String,
    /// `YYYY-MM-DD`.
    pub expiry: String,
    /// A user signs in with a password. A service account has none.
    pub password: Secret,
    pub confirm: Secret,
    pub tried: bool,
}

impl PrincipalDraft {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            kind: PrincipalKind::User,
            description: String::new(),
            expiry: String::new(),
            password: Secret::default(),
            confirm: Secret::default(),
            tried: false,
        }
    }

    /// Whether this kind of principal signs in with a password.
    pub fn needs_password(&self) -> bool {
        self.kind == PrincipalKind::User
    }

    pub fn password_issue(&self) -> Option<String> {
        self.needs_password()
            .then(|| password_issue(&self.password))
            .flatten()
    }

    pub fn confirm_issue(&self) -> Option<String> {
        self.needs_password()
            .then(|| confirm_issue(&self.password, &self.confirm))
            .flatten()
    }

    /// Forget what was typed for the password.
    pub fn clear_password(&mut self) {
        self.password = Secret::default();
        self.confirm = Secret::default();
    }

    pub fn name_issue(&self, taken: &[String]) -> Option<String> {
        identifier_issue(&self.name, "principal").or_else(|| {
            taken
                .iter()
                .any(|n| *n == self.name.trim())
                .then(|| format!("`{}` exists. Choose another name.", self.name.trim()))
        })
    }

    pub fn expiry_issue(&self) -> Option<String> {
        match parse_date(&self.expiry) {
            Some(_) => None,
            None if self.expiry.trim().is_empty() => {
                Some("Enter the date the credential expires.".to_string())
            }
            None => Some("Enter a date as YYYY-MM-DD.".to_string()),
        }
    }

    /// The change the draft stages, when every field is valid.
    pub fn change(&self, taken: &[String]) -> Option<AccessChange> {
        if self.name_issue(taken).is_some()
            || self.expiry_issue().is_some()
            || self.password_issue().is_some()
            || self.confirm_issue().is_some()
        {
            return None;
        }
        Some(AccessChange::CreatePrincipal {
            name: self.name.trim().to_string(),
            kind: self.kind,
            description: self.description.trim().to_string(),
            credential_expiry: parse_date(&self.expiry)?,
            password: self.needs_password().then(|| self.password.clone()),
        })
    }
}

impl Default for PrincipalDraft {
    fn default() -> Self {
        Self::new()
    }
}

/// What the person typed for a new role.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoleDraft {
    pub name: String,
    pub description: String,
    pub tried: bool,
}

impl RoleDraft {
    pub fn name_issue(&self, taken: &[String]) -> Option<String> {
        identifier_issue(&self.name, "role").or_else(|| {
            taken
                .iter()
                .any(|n| *n == self.name.trim())
                .then(|| format!("`{}` exists. Choose another name.", self.name.trim()))
        })
    }

    pub fn change(&self, taken: &[String]) -> Option<AccessChange> {
        self.name_issue(taken)
            .is_none()
            .then(|| AccessChange::CreateRole {
                name: self.name.trim().to_string(),
                description: self.description.trim().to_string(),
            })
    }
}

/// One edit to who has access. Edits stage until the person reviews them.
///
/// `CreatePrincipal` carries a password, so the type has no `Serialize` and
/// `Debug` redacts it. The change holds it until it is applied or withdrawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessChange {
    CreatePrincipal {
        name: String,
        kind: PrincipalKind,
        description: String,
        credential_expiry: i64,
        /// `Some` for a user, which signs in with it. `None` for a service
        /// account.
        password: Option<Secret>,
    },
    CreateRole {
        name: String,
        description: String,
    },
    AddMembership {
        principal: String,
        role: String,
        database: String,
    },
    RemoveMembership {
        principal: String,
        role: String,
        database: String,
    },
}

impl AccessChange {
    /// One line for the staged list. `names` maps a principal id to its name.
    pub fn describe(&self, names: impl Fn(&str) -> String) -> String {
        match self {
            AccessChange::CreatePrincipal { name, kind, .. } => {
                format!("Create {} {name}", kind.label().to_lowercase())
            }
            AccessChange::CreateRole { name, .. } => format!("Create role {name}"),
            AccessChange::AddMembership {
                principal,
                role,
                database,
            } => format!("Give {} role {role} on {database}", names(principal)),
            AccessChange::RemoveMembership {
                principal,
                role,
                database,
            } => format!("Remove role {role} on {database} from {}", names(principal)),
        }
    }
}

/// Direct rows first. The sort is stable, so grants keep their stored order.
pub fn sort_effective(rows: &mut [EffectiveGrant]) {
    rows.sort_by_key(|r| !r.is_direct());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_reported_values_have_labels() {
        assert_eq!(PrincipalStatus::Disabled.label(), "Disabled");
        assert_eq!(PrincipalStatus::Disabled.tone(), Tone::Idle);
        assert_eq!(Mfa::Required.label(), "Required");
        assert_eq!(Privilege::Delete.label(), "DELETE");
    }

    #[test]
    fn a_scope_names_its_database_or_says_it_is_not_reported() {
        assert_eq!(scope_text(Some("db2"), "users"), "db2.users");
        assert_eq!(scope_text(None, "users"), "users (database not reported)");
        assert_eq!(
            scope_text(Some(EVERY), EVERY),
            "Every collection in every database"
        );
        assert_eq!(scope_text(Some("db2"), EVERY), "Every collection in db2");
    }

    #[test]
    fn a_new_principal_needs_a_free_name_and_an_expiry() {
        let mut d = PrincipalDraft::new();
        assert!(d.change(&[]).is_none());
        d.name = "etl_bot".into();
        d.expiry = "2027-01-10".into();
        d.password = Secret::new("pw-1");
        d.confirm = Secret::new("pw-1");
        assert!(d.change(&[]).is_some());
        assert!(d.change(&["etl_bot".to_string()]).is_none());
        d.expiry = "2027-02-30".into();
        assert!(d.expiry_issue().is_some());
    }

    #[test]
    fn a_user_needs_a_matching_password_and_a_service_account_has_none() {
        let mut d = PrincipalDraft::new();
        d.name = "ann".into();
        d.expiry = "2027-01-10".into();
        assert!(d.password_issue().is_some());
        assert!(d.change(&[]).is_none(), "no password");
        d.password = Secret::new("pw-1");
        assert!(d.confirm_issue().is_some());
        assert!(d.change(&[]).is_none(), "not confirmed");
        d.confirm = Secret::new("pw-2");
        assert!(d.change(&[]).is_none(), "differs");
        d.confirm = Secret::new("pw-1");
        let Some(AccessChange::CreatePrincipal { password, .. }) = d.change(&[]) else {
            panic!("a valid user stages");
        };
        assert_eq!(password, Some(Secret::new("pw-1")));
        d.kind = PrincipalKind::ServiceAccount;
        d.clear_password();
        let Some(AccessChange::CreatePrincipal { password, .. }) = d.change(&[]) else {
            panic!("a service account needs no password");
        };
        assert_eq!(password, None);
    }

    #[test]
    fn debug_of_a_draft_or_a_change_hides_the_password() {
        let mut d = PrincipalDraft::new();
        d.name = "ann".into();
        d.expiry = "2027-01-10".into();
        d.password = Secret::new("s3cr3t-pw-Q");
        d.confirm = Secret::new("s3cr3t-pw-Q");
        let change = d.change(&[]);
        for text in [
            format!("{d:?}"),
            format!("{change:?}"),
            format!("{:#?}", change),
        ] {
            assert!(!text.contains("s3cr3t"), "{text}");
        }
    }

    #[test]
    fn staged_changes_read_as_sentences() {
        let c = AccessChange::AddMembership {
            principal: "pr_02".into(),
            role: "analytics".into(),
            database: "analytics".into(),
        };
        assert_eq!(
            c.describe(|_| "analyst".to_string()),
            "Give analyst role analytics on analytics"
        );
    }

    #[test]
    fn direct_rows_sort_before_inherited_ones() {
        let row = |t: &str, via: Option<&str>| EffectiveGrant {
            database: Some("analytics".into()),
            target: t.into(),
            privilege: Privilege::Select,
            source: "analytics".into(),
            via: via.map(str::to_string),
        };
        let mut rows = vec![row("inventory", Some("catalog_reader")), row("sales", None)];
        sort_effective(&mut rows);
        assert_eq!(rows[0].target, "sales");
    }
}
