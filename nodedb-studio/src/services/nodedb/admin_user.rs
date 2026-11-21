//! Creating a user on NodeDB 0.6.2.
//!
//! `CREATE USER <name> WITH PASSWORD '<pw>' [ROLE <role>]` is the only way
//! to create a user. It takes no bound `$n`, so the password enters the
//! statement text through `sql_string`. Without `ROLE` the server gives the
//! user `readwrite`.

use std::collections::HashSet;

use crate::models::admin::ReviewLine;
use crate::models::admin_access::{AccessChange, PrincipalKind};
use crate::models::admin_password::{Secret, password_issue};
use crate::services::error::StudioError;
use crate::services::nodedb::admin_access::{UserRow, ident};
use crate::services::nodedb::admin_plan::Planned;

/// The role the server gives a new user that names none.
const DEFAULT_ROLE: &str = "readwrite";

/// One statement to run. A statement that holds a password never shows its
/// text: `label` stands for it in an error, and `scrub` removes the text and
/// the password from a message the server sends back.
pub(super) struct Statement {
    sql: Secret,
    password: Option<Secret>,
    pub label: String,
}

impl Statement {
    pub fn plain(sql: String) -> Self {
        Self {
            label: sql.clone(),
            sql: Secret::new(sql),
            password: None,
        }
    }

    pub fn sql(&self) -> &str {
        self.sql.expose()
    }

    /// `error`, unless its text carries the statement or the password. Then a
    /// `Rejected` with the text removed, or with no server text at all when
    /// the password still shows after the statement is removed.
    pub fn scrubbed(&self, error: StudioError) -> StudioError {
        let Some(password) = &self.password else {
            return error;
        };
        let text = error.to_string();
        let clean = text.replace(self.sql(), &self.label);
        let secret = password.expose();
        let leaks = clean.contains(secret) || clean.contains(&sql_string(secret));
        if !leaks && clean == text {
            return error;
        }
        let reason = if leaks {
            format!(
                "NodeDB refused `{}`. Its message is withheld because it repeats the password.",
                self.label
            )
        } else {
            clean
        };
        StudioError::Rejected { reason }
    }
}

/// `text` as a SQL string literal for NodeDB 0.6.2.
///
/// The server's lexer (`nodedb-sql`, `ddl_ast/parse/clause/token.rs`) reads
/// `'...'` with one escape: a doubled `''` is one quote. A backslash is an
/// ordinary character. Only an `E'...'` string decodes backslashes, and this
/// never writes the `E` prefix.
pub(super) fn sql_string(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// The statement that creates `name`. `name` and `role` must pass `ident`.
fn create_user(name: &str, password: &Secret, role: Option<&str>) -> Statement {
    let head = format!("CREATE USER {name} WITH PASSWORD ");
    let tail = role.map(|r| format!(" ROLE {r}")).unwrap_or_default();
    Statement {
        sql: Secret::new(format!("{head}{}{tail}", sql_string(password.expose()))),
        password: Some(password.clone()),
        label: format!("CREATE USER {name}{tail}"),
    }
}

/// The fields of a create-principal change.
pub(super) struct NewUser<'a> {
    pub name: &'a str,
    pub kind: PrincipalKind,
    pub description: &'a str,
    pub credential_expiry: i64,
    pub password: Option<&'a Secret>,
}

/// What a create-principal change does. `at` is its place in `batch`. A role
/// the batch gives the new user joins the `CREATE USER` as its `ROLE`, so the
/// user never holds `readwrite` by default beside it. The matching grant
/// then runs no statement: its index goes into `folded`.
pub(super) fn plan_create(
    new: NewUser<'_>,
    (at, batch): (usize, &[AccessChange]),
    users: &mut Vec<UserRow>,
    roles: &[String],
    folded: &mut HashSet<usize>,
) -> Planned {
    let NewUser {
        name,
        kind,
        description,
        credential_expiry,
        password,
    } = new;
    let label = format!("Principal {name}");
    let fail = |error: String| Planned {
        line: ReviewLine::new(label.clone(), None, "created"),
        statement: None,
        errors: vec![error],
        warnings: vec![],
    };
    match kind {
        PrincipalKind::User => {}
        PrincipalKind::ServiceAccount => {
            return fail(
                "Studio does not create service accounts on NodeDB 0.6.2: the server lists none, so Studio could not show or check one."
                    .to_string(),
            );
        }
    }
    if let Err(e) = ident(name, "user") {
        return fail(e.to_string());
    }
    if users.iter().any(|u| u.name == name) {
        return fail(format!("User `{name}` exists."));
    }
    let Some(password) = password else {
        return fail(format!("`{name}` needs a password."));
    };
    if let Some(issue) = password_issue(password) {
        return fail(issue);
    }
    let gives_role = |c: &AccessChange| matches!(c, AccessChange::AddMembership { principal, .. } if principal == name);
    let later = batch.iter().enumerate().skip(at + 1);
    let initial = later.clone().find_map(|(j, c)| match c {
        AccessChange::AddMembership {
            principal, role, ..
        } if principal == name
            && roles.contains(role)
            && *role == role.to_lowercase()
            && ident(role, "role").is_ok() =>
        {
            Some((j, role.clone()))
        }
        AccessChange::AddMembership { .. }
        | AccessChange::CreatePrincipal { .. }
        | AccessChange::CreateRole { .. }
        | AccessChange::RemoveMembership { .. } => None,
    });
    if initial.is_none() && later.clone().any(|(_, c)| gives_role(c)) {
        return fail(format!(
            "The role staged for `{name}` does not exist yet at this point, and NodeDB gives a new user `{DEFAULT_ROLE}`. Stage the role before the user, with a lowercase name."
        ));
    }
    let mut warnings = vec![];
    match &initial {
        Some((j, _)) => {
            folded.insert(*j);
        }
        None => warnings.push(format!(
            "NodeDB gives `{name}` the `{DEFAULT_ROLE}` role. Stage another role in this review to give it that role instead."
        )),
    }
    if !description.trim().is_empty() {
        warnings.push("NodeDB does not store a user description. It is dropped.".to_string());
    }
    if credential_expiry != 0 {
        warnings.push(
            "NodeDB takes no credential expiry when it creates a user. It is dropped.".to_string(),
        );
    }
    // The batch state tracks names and roles only: the tenant is unused.
    users.push(UserRow {
        name: name.to_string(),
        tenant: 0,
        superuser: false,
        roles: if initial.is_some() {
            vec![]
        } else {
            vec![DEFAULT_ROLE.to_string()]
        },
    });
    Planned {
        line: ReviewLine::new(label, None, "created"),
        statement: Some(create_user(
            name,
            password,
            initial.as_ref().map(|(_, r)| r.as_str()),
        )),
        errors: vec![],
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_literal_doubles_the_quote_and_leaves_the_rest() {
        assert_eq!(sql_string("abc"), "'abc'");
        assert_eq!(sql_string("a'b"), "'a''b'");
        assert_eq!(sql_string("''"), "''''''");
        assert_eq!(sql_string(r"a\b"), r"'a\b'", "a backslash is plain text");
        assert_eq!(sql_string("x\"y;z--w\n é"), "'x\"y;z--w\n é'");
        assert_eq!(sql_string(""), "''");
    }

    #[test]
    fn the_statement_text_stays_out_of_its_label_and_debug_free_output() {
        let s = create_user("ann", &Secret::new("p'w"), Some("readonly"));
        assert_eq!(
            s.sql(),
            "CREATE USER ann WITH PASSWORD 'p''w' ROLE readonly"
        );
        assert_eq!(s.label, "CREATE USER ann ROLE readonly");
    }

    #[test]
    fn a_server_message_that_repeats_the_statement_or_password_is_cleaned() {
        let s = create_user("ann", &Secret::new("zz-9"), None);
        let echo = |m: String| StudioError::Rejected { reason: m };
        let cleaned = s.scrubbed(echo(format!("syntax error near `{}`", s.sql())));
        assert!(!cleaned.to_string().contains("zz-9"), "{cleaned}");
        assert!(cleaned.to_string().contains("CREATE USER ann"));
        let bare = s.scrubbed(echo("bad value 'zz-9'".to_string()));
        assert!(!bare.to_string().contains("zz-9"), "{bare}");
        let fine = s.scrubbed(echo("user 'ann' already exists".to_string()));
        assert_eq!(
            fine.to_string(),
            echo("user 'ann' already exists".into()).to_string()
        );
    }
}

#[cfg(test)]
pub(super) mod live {
    use super::*;
    use crate::models::admin::AdminChange;
    use crate::models::cluster_op::OpResult;
    use crate::services::admin_data::AdminData;
    use crate::services::nodedb::NodeDbConnectionService;
    use crate::services::nodedb::admin_access::users;
    use crate::services::nodedb::service::{live, live_as};

    /// Passwords the statement must carry exactly. Never printed: a failure
    /// names the index.
    const HARD: [&str; 14] = [
        "plain-pw-1",
        "it's",
        "end'",
        "''",
        r"back\slash",
        r"\'",
        "dq\"x",
        "semi;colon",
        "'; DROP USER nodedb; --",
        "dash--dash",
        "line1\nline2",
        "tab\there\rcr",
        "héllo-日本語-😀",
        "E'x' /* c */ ROLE superuser",
    ];

    /// Held for a whole test. Users and roles share one revision, so a test
    /// that creates or drops them would make another test's apply stale. Each
    /// test is its own process, so the lock is a file.
    pub(in crate::services::nodedb) fn users_lock() -> std::fs::File {
        let path = std::env::temp_dir().join("studio_probe_users.lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .expect("the lock file opens");
        file.lock().expect("the lock is taken");
        file
    }

    fn user(i: usize) -> String {
        format!("studio_probe_u_h{i}")
    }

    async fn quietly(svc: &NodeDbConnectionService, sqls: &[String]) {
        let c = svc.client(None).expect("a session");
        for sql in sqls {
            let _ = c.query(sql).await;
        }
    }

    async fn run(
        svc: &NodeDbConnectionService,
        changes: Vec<AccessChange>,
    ) -> Result<OpResult, StudioError> {
        let change = AdminChange::Access(changes);
        let review = svc.preview_admin_change(&change).await?;
        if !review.can_apply() {
            return Err(StudioError::Rejected {
                reason: review.errors.join(" "),
            });
        }
        svc.apply_admin_change(&change, review.revision).await
    }

    fn create(name: &str, password: &str) -> AccessChange {
        AccessChange::CreatePrincipal {
            name: name.into(),
            kind: PrincipalKind::User,
            description: String::new(),
            credential_expiry: 0,
            password: Some(Secret::new(password)),
        }
    }

    fn grant(name: &str, role: &str) -> AccessChange {
        AccessChange::AddMembership {
            principal: name.into(),
            role: role.into(),
            database: String::new(),
        }
    }

    async fn roles_of(svc: &NodeDbConnectionService, name: &str) -> Vec<String> {
        let t = svc.table("SHOW USERS").await.expect("SHOW USERS");
        users(&t)
            .expect("rows")
            .into_iter()
            .find(|u| u.name == name)
            .map(|u| u.roles)
            .unwrap_or_default()
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_create_user_keeps_every_hard_password_exact() {
        let _lock = users_lock();
        let svc = live().await;
        let names: Vec<String> = (0..HARD.len()).map(user).collect();
        let drops: Vec<String> = names
            .iter()
            .map(|n| format!("DROP USER IF EXISTS {n}"))
            .collect();
        quietly(&svc, &drops).await;
        for (i, pw) in HARD.iter().enumerate() {
            let name = user(i);
            run(&svc, vec![create(&name, pw)])
                .await
                .unwrap_or_else(|e| panic!("create with password #{i}: {e}"));
            assert_eq!(roles_of(&svc, &name).await, ["readwrite"], "#{i}");
            // The login proves the server stored exactly these characters.
            let (as_user, _) = live_as(&name, pw).await;
            let who = as_user.table("SHOW SESSION").await.expect("SHOW SESSION");
            assert!(
                who.rows.iter().flatten().any(|c| *c == name),
                "#{i}: the session is not {name}"
            );
        }
        // No server record repeats a password.
        let audit = svc
            .table("SHOW AUDIT LOG LIMIT 500")
            .await
            .expect("audit")
            .rows
            .concat()
            .join("\n");
        let users_text = svc
            .table("SHOW USERS")
            .await
            .expect("users")
            .rows
            .concat()
            .join("\n");
        for (i, pw) in HARD.iter().enumerate().filter(|(_, p)| p.len() >= 6) {
            assert!(!audit.contains(pw), "#{i} is in the audit log");
            assert!(!users_text.contains(pw), "#{i} is in SHOW USERS");
        }
        quietly(&svc, &drops).await;
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_create_user_roles_replace_the_default() {
        let _lock = users_lock();
        let svc = live().await;
        let (ro, mixed, custom) = (
            "studio_probe_u_ro",
            "studio_probe_u_mix",
            "studio_probe_u_role",
        );
        let reset = [
            format!("DROP USER IF EXISTS {ro}"),
            format!("DROP USER IF EXISTS {mixed}"),
            format!("DROP ROLE IF EXISTS {custom}"),
        ];
        quietly(&svc, &reset).await;

        run(&svc, vec![create(ro, "pw-ro-1"), grant(ro, "readonly")])
            .await
            .expect("readonly only");
        assert_eq!(roles_of(&svc, ro).await, ["readonly"]);

        let made = run(
            &svc,
            vec![
                AccessChange::CreateRole {
                    name: custom.into(),
                    description: String::new(),
                },
                create(mixed, "pw-mix-1"),
                grant(mixed, custom),
                grant(mixed, "monitor"),
            ],
        )
        .await
        .expect("custom role first, then another");
        assert!(made.summary.contains("3 statement"), "{}", made.summary);
        let mut got = roles_of(&svc, mixed).await;
        got.sort();
        assert_eq!(got, ["monitor", custom], "no readwrite beside them");

        // The role must exist before the user: refused, nothing written.
        let again = run(
            &svc,
            vec![
                create("studio_probe_u_late", "pw-late-1"),
                grant("studio_probe_u_late", "readonly"),
                grant("studio_probe_u_late", "nonexistent_role"),
            ],
        )
        .await;
        assert!(again.is_err());
        assert!(roles_of(&svc, "studio_probe_u_late").await.is_empty());
        quietly(
            &svc,
            &[
                format!("DROP USER IF EXISTS {ro}"),
                format!("DROP USER IF EXISTS {mixed}"),
                format!("DROP ROLE IF EXISTS {custom}"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs a live server: see service::live"]
    async fn live_admin_create_user_server_errors_never_show_the_password() {
        let _lock = users_lock();
        let svc = live().await;
        let marker = "zzq-marker-4471";
        let name = "studio_probe_u_err";
        quietly(&svc, &[format!("DROP USER IF EXISTS {name}")]).await;
        let client = svc.client(None).expect("a session");
        // A role the server does not know: the statement fails on the server.
        let stmt = create_user(name, &Secret::new(marker), Some("studio_probe_u_norole"));
        let raw = client
            .query(stmt.sql())
            .await
            .expect_err("the server refuses");
        let raw = StudioError::from(raw);
        let safe = stmt.scrubbed(raw);
        let text = safe.to_string();
        assert!(!text.contains(marker), "the error repeats the password");
        assert!(!format!("{safe:?}").contains(marker));
        // What the server said, with the marker standing in: shown on purpose.
        println!("server error as shown: {text}");
        assert!(roles_of(&svc, name).await.is_empty(), "nothing was created");

        // A statement the parser refuses may be quoted back by the server.
        let bad = Statement {
            sql: Secret::new(format!(
                "CREATE USER {name} WITH PASSWORD '{marker}' BOGUS 1"
            )),
            password: Some(Secret::new(marker)),
            label: format!("CREATE USER {name}"),
        };
        let raw = StudioError::from(client.query(bad.sql()).await.expect_err("a syntax error"));
        let shown = raw.to_string();
        println!(
            "raw syntax error repeats the password: {}",
            shown.contains(marker)
        );
        let safe = bad.scrubbed(raw);
        assert!(!safe.to_string().contains(marker));
        assert!(!format!("{safe:?}").contains(marker));
        println!("syntax error as shown: {safe}");
        quietly(&svc, &[format!("DROP USER IF EXISTS {name}")]).await;
    }
}
