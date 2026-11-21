//! Access: principals, roles, and the directory that lists them.
//!
//! The grants a role carries, the memberships that give a principal its
//! roles, and the access in effect are in `admin::grant`. Staged edits are
//! in `admin::access_change`. Both are re-exported here.

use crate::models::feeds::Tone;

pub use crate::models::admin::access_change::{AccessChange, PrincipalDraft, RoleDraft};
pub use crate::models::admin::grant::{
    EVERY, EffectiveGrant, HOLDER_DATABASES, Member, Membership, Privilege, RoleGrant,
    SubjectAccess, USER_DATABASES, scope_text, sort_effective,
};

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

/// Who the access views describe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessSubject {
    Principal(String),
    Role(String),
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
}
