//! Row policies: which rows a role sees or writes in one collection.
//!
//! `USING` filters the rows an operation reads or touches. `CHECK` limits
//! the rows an operation writes, so it applies to INSERT and UPDATE only.
//! A test runs the policy as a named user with a session setting, reads
//! rows, and writes nothing. It never assumes a bypass privilege.

use crate::models::admin::identifier_issue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyOperation {
    Select,
    Insert,
    Update,
    Delete,
    InsertUpdate,
    All,
}

impl PolicyOperation {
    pub const ALL: [PolicyOperation; 6] = [
        PolicyOperation::Select,
        PolicyOperation::Insert,
        PolicyOperation::Update,
        PolicyOperation::Delete,
        PolicyOperation::InsertUpdate,
        PolicyOperation::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PolicyOperation::Select => "SELECT",
            PolicyOperation::Insert => "INSERT",
            PolicyOperation::Update => "UPDATE",
            PolicyOperation::Delete => "DELETE",
            PolicyOperation::InsertUpdate => "INSERT, UPDATE",
            PolicyOperation::All => "ALL",
        }
    }

    /// Whether the operation writes rows a `CHECK` can limit.
    pub fn has_check(self) -> bool {
        match self {
            PolicyOperation::Insert
            | PolicyOperation::Update
            | PolicyOperation::InsertUpdate
            | PolicyOperation::All => true,
            PolicyOperation::Select | PolicyOperation::Delete => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Stable id. A policy can be renamed.
    pub id: String,
    pub name: String,
    pub database: String,
    pub collection: String,
    pub operation: PolicyOperation,
    pub role: String,
    pub using_expr: String,
    pub check_expr: Option<String>,
    pub enabled: bool,
}

impl Policy {
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [&self.name, &self.collection, &self.role]
                .iter()
                .any(|f| f.to_lowercase().contains(needle))
    }

    pub fn draft(&self) -> PolicyDraft {
        PolicyDraft {
            id: Some(self.id.clone()),
            name: self.name.clone(),
            database: self.database.clone(),
            collection: self.collection.clone(),
            operation: self.operation,
            role: self.role.clone(),
            using_expr: self.using_expr.clone(),
            check_expr: self.check_expr.clone().unwrap_or_default(),
            enabled: self.enabled,
            tried: false,
        }
    }
}

/// A user the test can run as, with the session settings it starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestUser {
    pub name: String,
    pub session: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySet {
    pub database: String,
    pub databases: Vec<String>,
    /// The collections of the database, for the editor.
    pub collections: Vec<String>,
    pub roles: Vec<String>,
    pub test_users: Vec<TestUser>,
    pub policies: Vec<Policy>,
    pub observed_at: i64,
}

/// An edit of one policy, or a new one when `id` is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDraft {
    pub id: Option<String>,
    pub name: String,
    pub database: String,
    pub collection: String,
    pub operation: PolicyOperation,
    pub role: String,
    pub using_expr: String,
    pub check_expr: String,
    pub enabled: bool,
    /// Review changes was pressed with a field missing.
    pub tried: bool,
}

impl PolicyDraft {
    pub fn blank(database: &str, collection: &str, role: &str) -> Self {
        Self {
            id: None,
            name: String::new(),
            database: database.to_string(),
            collection: collection.to_string(),
            operation: PolicyOperation::Select,
            role: role.to_string(),
            using_expr: String::new(),
            check_expr: String::new(),
            enabled: false,
            tried: false,
        }
    }

    pub fn name_issue(&self, others: &[&Policy]) -> Option<String> {
        identifier_issue(&self.name, "policy").or_else(|| {
            others
                .iter()
                .any(|p| p.name == self.name.trim() && Some(&p.id) != self.id.as_ref())
                .then(|| format!("`{}` exists. Choose another name.", self.name.trim()))
        })
    }

    pub fn using_issue(&self) -> Option<String> {
        self.using_expr
            .trim()
            .is_empty()
            .then(|| "Enter the row filter.".to_string())
    }

    pub fn check_issue(&self) -> Option<String> {
        (self.operation.has_check() && self.check_expr.trim().is_empty())
            .then(|| "Enter the row constraint for writes.".to_string())
    }

    pub fn role_issue(&self) -> Option<String> {
        self.role
            .trim()
            .is_empty()
            .then(|| "Choose the role the policy applies to.".to_string())
    }

    pub fn is_valid(&self, others: &[&Policy]) -> bool {
        self.name_issue(others).is_none()
            && self.using_issue().is_none()
            && self.check_issue().is_none()
            && self.role_issue().is_none()
    }

    /// The `CHECK` text the server stores: none when the operation has none.
    pub fn stored_check(&self) -> Option<String> {
        (self.operation.has_check() && !self.check_expr.trim().is_empty())
            .then(|| self.check_expr.trim().to_string())
    }

    /// Whether the draft differs from what `policy` stores.
    pub fn differs_from(&self, policy: &Policy) -> bool {
        self.name.trim() != policy.name
            || self.operation != policy.operation
            || self.role.trim() != policy.role
            || self.using_expr.trim() != policy.using_expr
            || self.stored_check() != policy.check_expr
            || self.enabled != policy.enabled
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyChange {
    Create(PolicyDraft),
    Update(PolicyDraft),
}

impl PolicyChange {
    pub fn draft(&self) -> &PolicyDraft {
        match self {
            PolicyChange::Create(d) | PolicyChange::Update(d) => d,
        }
    }
}

/// A read-only test of one policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyTest {
    pub database: String,
    pub collection: String,
    pub using_expr: String,
    pub user: String,
    pub session: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyTestResult {
    pub user: String,
    pub matched_rows: u64,
    /// Rows the filter excluded stay hidden. The test never counts them.
    pub others_hidden: bool,
    /// A test writes nothing. The field exists so a screen can say so.
    pub wrote: bool,
    pub bypass_assumed: bool,
    pub ran_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_writing_operations_carry_a_check() {
        assert!(!PolicyOperation::Select.has_check());
        assert!(!PolicyOperation::Delete.has_check());
        assert!(PolicyOperation::InsertUpdate.has_check());
        assert!(PolicyOperation::All.has_check());
    }

    #[test]
    fn a_select_policy_needs_no_check_and_stores_none() {
        let mut d = PolicyDraft::blank("analytics", "sales", "tenant_reader");
        d.name = "sales_tenant_read".into();
        d.using_expr = "tenant_id = current_setting('app.tenant_id')".into();
        d.check_expr = "stale text".into();
        assert!(d.is_valid(&[]));
        assert_eq!(d.stored_check(), None);
        d.operation = PolicyOperation::Insert;
        assert!(d.check_issue().is_none());
        d.check_expr.clear();
        assert!(d.check_issue().is_some());
    }
}
