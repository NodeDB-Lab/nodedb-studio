//! Staged access edits: what the person typed for a new principal or role,
//! and the changes waiting for review.

use crate::models::admin::access::PrincipalKind;
use crate::models::admin::password::{Secret, confirm_issue, password_issue};
use crate::models::admin::{identifier_issue, parse_date};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
