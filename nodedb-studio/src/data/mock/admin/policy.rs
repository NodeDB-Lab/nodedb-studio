//! Row policies and the read-only test of one.

use crate::data::mock::admin::store::{AdminStore, DATABASE, missing};
use crate::models::admin::{AdminReview, ReviewLine};
use crate::models::admin_audit::TargetType;
use crate::models::admin_policy::{
    Policy, PolicyChange, PolicyDraft, PolicyOperation, PolicySet, PolicyTest, PolicyTestResult,
    TestUser,
};
use crate::services::error::StudioError;

const POLICY_WRITE: &str = "auth.policy.write";
const TENANT_FILTER: &str = "tenant_id = current_setting('app.tenant_id')";

/// The collections of the analytics database.
const COLLECTIONS: [&str; 9] = [
    "users",
    "orders",
    "sales",
    "places",
    "articles",
    "sessions",
    "inventory",
    "climate_cube",
    "shipments",
];

pub(super) fn seeded_policies() -> Vec<Policy> {
    vec![
        Policy {
            id: "pol_01".into(),
            name: "sales_tenant_read".into(),
            database: DATABASE.into(),
            collection: "analytics.sales".into(),
            operation: PolicyOperation::Select,
            role: "tenant_reader".into(),
            using_expr: TENANT_FILTER.into(),
            check_expr: None,
            enabled: true,
        },
        Policy {
            id: "pol_02".into(),
            name: "sales_tenant_write".into(),
            database: DATABASE.into(),
            collection: "analytics.sales".into(),
            operation: PolicyOperation::InsertUpdate,
            role: "tenant_writer".into(),
            using_expr: TENANT_FILTER.into(),
            check_expr: Some(TENANT_FILTER.into()),
            enabled: true,
        },
    ]
}

/// How many `sales` rows each tenant owns. Other tenants' rows are never
/// counted for the person running a test.
fn tenant_rows(tenant: &str) -> u64 {
    match tenant {
        "tA" => 842,
        "tB" => 611,
        "tC" => 305,
        _ => 0,
    }
}

impl AdminStore {
    pub fn policy_set(&self, database: &str) -> Result<PolicySet, StudioError> {
        if !self.databases.iter().any(|d| d == database) {
            return Err(missing("database", database));
        }
        let analytics = database == DATABASE;
        Ok(PolicySet {
            database: database.to_string(),
            databases: self.databases.clone(),
            collections: if analytics {
                COLLECTIONS
                    .iter()
                    .map(|c| format!("{DATABASE}.{c}"))
                    .collect()
            } else {
                Vec::new()
            },
            roles: self.roles.iter().map(|r| r.name.clone()).collect(),
            test_users: ["tenant_a", "tenant_b"]
                .iter()
                .zip(["tA", "tB"])
                .map(|(name, tenant)| TestUser {
                    name: (*name).to_string(),
                    session: vec![("app.tenant_id".to_string(), tenant.to_string())],
                })
                .collect(),
            policies: self
                .policies
                .iter()
                .filter(|p| p.database == database)
                .cloned()
                .collect(),
            observed_at: self.clock,
        })
    }

    /// Run the filter as the user, read-only. Nothing is written.
    pub fn test_policy(&self, test: &PolicyTest) -> Result<PolicyTestResult, StudioError> {
        let known = ["tenant_a", "tenant_b"];
        if !known.contains(&test.user.as_str()) {
            return Err(missing("test user", &test.user));
        }
        if test.using_expr.trim().is_empty() {
            return Err(StudioError::Rejected {
                reason: "the policy has no row filter to test".to_string(),
            });
        }
        let tenant = test
            .session
            .iter()
            .find(|(k, _)| k == "app.tenant_id")
            .map(|(_, v)| v.trim())
            .unwrap_or("");
        Ok(PolicyTestResult {
            user: test.user.clone(),
            matched_rows: tenant_rows(tenant),
            others_hidden: true,
            wrote: false,
            bypass_assumed: false,
            ran_at: self.clock,
        })
    }

    fn policy_lines(p: &PolicyDraft) -> Vec<String> {
        vec![
            format!("Operation: {}", p.operation.label()),
            format!("Role: {}", p.role.trim()),
            format!("USING: {}", p.using_expr.trim()),
            format!(
                "CHECK: {}",
                p.stored_check().unwrap_or_else(|| "none".into())
            ),
            format!("Enabled: {}", p.enabled),
        ]
    }

    pub(super) fn preview_policy(&self, change: &PolicyChange) -> AdminReview {
        let d = change.draft();
        let others: Vec<&Policy> = self.policies.iter().collect();
        let mut errors: Vec<String> = [
            d.name_issue(&others),
            d.role_issue(),
            d.using_issue(),
            d.check_issue(),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !self.roles.iter().any(|r| r.name == d.role.trim()) && d.role_issue().is_none() {
            errors.push(format!("No role {}.", d.role.trim()));
        }
        let stored =
            d.id.as_ref()
                .and_then(|id| self.policies.iter().find(|p| p.id == *id));
        if matches!(change, PolicyChange::Update(_)) && stored.is_none() {
            errors.push("The policy no longer exists.".to_string());
        }
        let mut lines = Vec::new();
        let line = |label: &str, before: Option<String>, after: String| {
            ReviewLine::new(label, before.as_deref(), after)
        };
        match stored {
            Some(p) => {
                if d.name.trim() != p.name {
                    lines.push(line("Name", Some(p.name.clone()), d.name.trim().into()));
                }
                if d.operation != p.operation {
                    lines.push(line(
                        "Operation",
                        Some(p.operation.label().into()),
                        d.operation.label().into(),
                    ));
                }
                if d.role.trim() != p.role {
                    lines.push(line("Role", Some(p.role.clone()), d.role.trim().into()));
                }
                if d.using_expr.trim() != p.using_expr {
                    lines.push(line(
                        "USING",
                        Some(p.using_expr.clone()),
                        d.using_expr.trim().into(),
                    ));
                }
                if d.stored_check() != p.check_expr {
                    lines.push(line(
                        "CHECK",
                        Some(p.check_expr.clone().unwrap_or_else(|| "none".into())),
                        d.stored_check().unwrap_or_else(|| "none".into()),
                    ));
                }
                if d.enabled != p.enabled {
                    lines.push(line(
                        "State",
                        Some(state(p.enabled).into()),
                        state(d.enabled).into(),
                    ));
                }
            }
            None => {
                lines.push(line("Policy", None, d.name.trim().into()));
                lines.push(line("Collection", None, d.collection.clone()));
                lines.push(line("Operation", None, d.operation.label().into()));
                lines.push(line("Role", None, d.role.trim().into()));
                lines.push(line("USING", None, d.using_expr.trim().into()));
                if let Some(c) = d.stored_check() {
                    lines.push(line("CHECK", None, c));
                }
                lines.push(line("State", None, state(d.enabled).into()));
            }
        }
        if lines.is_empty() && errors.is_empty() {
            errors.push("The policy has no change to apply.".to_string());
        }
        let mut warnings = Vec::new();
        let now_enabled = d.enabled;
        let was_enabled = stored.is_some_and(|p| p.enabled);
        if now_enabled && !was_enabled {
            warnings.push(format!(
                "Enabling filters every {} of {} by role {}.",
                d.operation.label(),
                d.collection,
                d.role.trim()
            ));
        }
        if !now_enabled && was_enabled {
            warnings.push(format!(
                "Disabling lets role {} reach every row of {} that its privileges allow.",
                d.role.trim(),
                d.collection
            ));
        }
        AdminReview {
            title: "Review policy changes".to_string(),
            target: format!("{} on {}", d.name.trim(), d.collection),
            lines,
            notes: vec![
                "Applying does not grant a bypass privilege to any role.".to_string(),
                "Applying records the change in the audit trail.".to_string(),
            ],
            warnings,
            errors,
            permission: self.permission(POLICY_WRITE),
            revision: self.rev.policies,
            apply_label: "Apply policy".to_string(),
        }
    }

    pub(super) fn apply_policy(&mut self, change: &PolicyChange) -> Result<String, StudioError> {
        let d = change.draft().clone();
        let before =
            d.id.as_ref()
                .and_then(|id| self.policies.iter().find(|p| p.id == *id))
                .map(|p| Self::policy_lines(&p.draft()))
                .unwrap_or_default();
        let (action, id) = match change {
            PolicyChange::Create(_) => {
                let id = format!("pol_{:02}", self.next_policy);
                self.next_policy += 1;
                self.policies.push(Policy {
                    id: id.clone(),
                    name: d.name.trim().to_string(),
                    database: d.database.clone(),
                    collection: d.collection.clone(),
                    operation: d.operation,
                    role: d.role.trim().to_string(),
                    using_expr: d.using_expr.trim().to_string(),
                    check_expr: d.stored_check(),
                    enabled: d.enabled,
                });
                ("Create policy", id)
            }
            PolicyChange::Update(_) => {
                let id = d.id.clone().unwrap_or_default();
                let p = self
                    .policies
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or_else(|| missing("policy", &id))?;
                p.name = d.name.trim().to_string();
                p.operation = d.operation;
                p.role = d.role.trim().to_string();
                p.using_expr = d.using_expr.trim().to_string();
                p.check_expr = d.stored_check();
                p.enabled = d.enabled;
                ("Update policy", id)
            }
        };
        self.rev.policies += 1;
        let audit = self.record(
            action,
            TargetType::Policy,
            &format!("policy:{}", d.name.trim()),
            &d.collection,
            before,
            Self::policy_lines(&d),
        );
        Ok(format!(
            "Applied policy {} ({id}). Recorded as {audit}.",
            d.name.trim()
        ))
    }
}

fn state(on: bool) -> &'static str {
    if on { "Enabled" } else { "Disabled" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_as(user: &str, tenant: &str) -> PolicyTest {
        PolicyTest {
            database: DATABASE.into(),
            collection: "analytics.sales".into(),
            using_expr: TENANT_FILTER.into(),
            user: user.into(),
            session: vec![("app.tenant_id".into(), tenant.into())],
        }
    }

    #[test]
    fn tenant_a_sees_842_rows_and_nothing_is_written() {
        let r = AdminStore::seeded()
            .test_policy(&test_as("tenant_a", "tA"))
            .expect("test");
        assert_eq!(r.matched_rows, 842);
        assert!(r.others_hidden && !r.wrote && !r.bypass_assumed);
    }

    #[test]
    fn an_unknown_tenant_sees_nothing() {
        let r = AdminStore::seeded()
            .test_policy(&test_as("tenant_a", "tZ"))
            .expect("test");
        assert_eq!(r.matched_rows, 0);
    }
}
