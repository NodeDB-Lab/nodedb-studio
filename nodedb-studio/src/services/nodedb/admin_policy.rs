//! Row policies from `SHOW RLS POLICIES`.
//!
//! NodeDB 0.6.2 reports `name, collection, type, mode, has_auth_refs,
//! enabled, created_by`. It does not report the `USING` text, a role, or a
//! write constraint, and a policy has no role: it applies to every user.
//! `type` is `Read`, `Write` or `All`, and `Write` covers INSERT, UPDATE and
//! DELETE together. The model has no operation for plain `Write`, so a
//! `Write` policy is an error and never a guess.

use crate::models::admin_policy::{Policy, PolicyChange, PolicyOperation};
use crate::services::decode::{Table, decode_rows};
use crate::services::error::StudioError;

use nodedb_client::NodeDbError;

pub(super) fn policies(table: &Table, database: &str) -> Result<Vec<Policy>, StudioError> {
    decode_rows(table, &["name", "collection", "type", "enabled"], |r| {
        let name = r.field("name")?;
        let collection = r.field("collection")?;
        let operation = match r.field("type")? {
            "Read" => PolicyOperation::Select,
            "All" => PolicyOperation::All,
            other => {
                return Err(StudioError::from(NodeDbError::bad_request(format!(
                    "policy `{name}` on `{collection}` has type `{other}`. Studio cannot show it: the server applies it to INSERT, UPDATE and DELETE together, and a policy operation has no such value."
                ))));
            }
        };
        Ok(Policy {
            id: format!("{collection}.{name}"),
            name: name.to_string(),
            database: database.to_string(),
            collection: collection.to_string(),
            operation,
            role: String::new(),
            using_expr: String::new(),
            check_expr: None,
            enabled: matches!(r.field("enabled")?, "t" | "true"),
        })
    })
}

/// The names `SHOW COLLECTIONS` lists.
pub(super) fn collection_names(table: &Table) -> Result<Vec<String>, StudioError> {
    decode_rows(table, &["name"], |r| Ok(r.field("name")?.to_string()))
}

/// Why the server cannot hold this change. A change that reaches the server
/// would drop a fact the person set, which is a different policy than the
/// one they reviewed.
pub(super) fn refusals(change: &PolicyChange) -> Vec<String> {
    let d = change.draft();
    let mut out = vec![
        "NodeDB 0.6.2 stores no role on a row policy: it applies to every user, so the role cannot be kept."
            .to_string(),
        "NodeDB 0.6.2 has no way to edit a policy or to create one disabled, so the review cannot apply."
            .to_string(),
    ];
    match d.operation {
        PolicyOperation::Select | PolicyOperation::All => {}
        PolicyOperation::Insert
        | PolicyOperation::Update
        | PolicyOperation::Delete
        | PolicyOperation::InsertUpdate => out.push(format!(
            "NodeDB 0.6.2 has no {} policy: a write policy covers INSERT, UPDATE and DELETE together.",
            d.operation.label()
        )),
    }
    if d.stored_check().is_some() {
        out.push("NodeDB 0.6.2 stores no separate write constraint on a policy.".to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_policy::PolicyDraft;

    fn table(rows: &[[&str; 4]]) -> Table {
        Table {
            columns: ["name", "collection", "type", "enabled"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|s| (*s).to_string()).collect())
                .collect(),
        }
    }

    #[test]
    fn read_and_all_policies_decode_without_invented_fields() {
        let p = policies(
            &table(&[["p1", "c", "Read", "true"], ["p2", "c", "All", "false"]]),
            "default",
        )
        .expect("decodes");
        assert_eq!(p[0].operation, PolicyOperation::Select);
        assert_eq!(p[1].operation, PolicyOperation::All);
        assert!(!p[1].enabled);
        assert_eq!(p[0].id, "c.p1");
        assert_eq!((p[0].role.as_str(), p[0].using_expr.as_str()), ("", ""));
        assert_eq!(p[0].check_expr, None);
    }

    #[test]
    fn a_write_policy_is_an_error_not_a_guess() {
        let e = policies(&table(&[["w", "c", "Write", "true"]]), "default").expect_err("write");
        assert!(e.to_string().contains("INSERT, UPDATE and DELETE"), "{e}");
    }

    #[test]
    fn every_policy_change_is_refused_with_its_reasons() {
        let d = PolicyDraft::blank("default", "c", "r");
        assert!(refusals(&PolicyChange::Create(d.clone())).len() >= 2);
        let mut w = d;
        w.operation = PolicyOperation::Insert;
        assert!(refusals(&PolicyChange::Update(w)).len() >= 3);
    }
}
