//! Row policies from `SHOW RLS POLICIES`.
//!
//! NodeDB 0.6.2 reports `name, collection, type, mode, has_auth_refs,
//! enabled, created_by`. The catalog stores the `USING` text, but no
//! statement returns it, a role, or a write constraint, so the filter and
//! the role are not reported. A policy stores no role: it
//! applies to every user. Bare `SHOW RLS POLICIES` lists the policies of
//! every database of the tenant by collection name only, so the policies
//! of one database come from `SHOW RLS POLICIES ON <collection>` there.
//! `type` is `Read`, `Write` or `All`, and `Write` covers INSERT, UPDATE and
//! DELETE together. The model has no operation for plain `Write`, so a
//! `Write` policy is an error and never a guess.

use crate::models::admin::policy::{Policy, PolicyChange, PolicyOperation};
use crate::services::decode::{Table, decode_rows};
use crate::services::error::StudioError;
use crate::services::nodedb::admin::access::ident;

use nodedb_client::NodeDbError;

pub(in crate::services::nodedb) fn policies(
    table: &Table,
    database: &str,
) -> Result<Vec<Policy>, StudioError> {
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
            role: None,
            using_expr: None,
            check_expr: None,
            enabled: matches!(r.field("enabled")?, "t" | "true"),
        })
    })
}

/// The names `SHOW COLLECTIONS` lists.
pub(in crate::services::nodedb) fn collection_names(
    table: &Table,
) -> Result<Vec<String>, StudioError> {
    decode_rows(table, &["name"], |r| Ok(r.field("name")?.to_string()))
}

/// The collections of this database that bare `SHOW RLS POLICIES` names,
/// each once. A name only another database holds is left out.
pub(in crate::services::nodedb) fn policy_collections(
    listed: &Table,
    collections: &[String],
) -> Result<Vec<String>, StudioError> {
    let mut names = decode_rows(listed, &["collection"], |r| {
        Ok(r.field("collection")?.to_string())
    })?;
    names.retain(|n| collections.contains(n) && ident(n, "collection").is_ok());
    names.sort();
    names.dedup();
    Ok(names)
}

/// Why the server cannot hold this change. A change that reaches the server
/// would drop a fact the person set, which is a different policy than the
/// one they reviewed.
pub(in crate::services::nodedb) fn refusals(change: &PolicyChange) -> Vec<String> {
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
    use crate::models::admin::policy::PolicyDraft;

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
        assert_eq!(
            (p[0].role.as_deref(), p[0].using_expr.as_deref()),
            (None, None)
        );
        assert!(!p[0].is_reported(), "the filter text is not reported");
        assert_eq!(p[0].check_expr, None);
    }

    #[test]
    fn only_this_databases_collections_are_asked_again() {
        let listed = Table {
            columns: vec!["collection".into()],
            rows: vec![vec!["a".into()], vec!["b".into()], vec!["a".into()]],
        };
        let names = policy_collections(&listed, &["a".to_string()]).expect("decodes");
        assert_eq!(names, ["a"]);
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
