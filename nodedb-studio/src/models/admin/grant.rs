//! Grants: the privileges a role carries, the memberships that give a
//! principal its roles, and the access in effect.
//!
//! A privilege names a collection, or every collection (`EVERY`). A
//! principal that holds every right (a superuser, a tenant administrator, a
//! collection owner) shows as an explicit `ALL` row, never as an empty
//! list. The effective view tells whether each row is held directly or
//! inherited from a parent role.

use crate::models::admin::access::AccessSubject;

/// Stands for every database, or every collection, in a grant row.
pub const EVERY: &str = "*";

/// Stands for each database a user who is not a superuser can open, in a
/// grant row of that user: the default database and each database granted
/// to it. The server lists no database grant, so Studio cannot name them.
pub const USER_DATABASES: &str = "+user";

/// `USER_DATABASES` in a grant row of a role: each database its holder can
/// open.
pub const HOLDER_DATABASES: &str = "+holder";

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

/// Where a grant row applies, for display. `None` for the database: no
/// database lists the target, so it is a function or a collection in a
/// database Studio could not read. An EXECUTE row names a function.
pub fn scope_text(database: Option<&str>, target: &str, privilege: Privilege) -> String {
    let databases = match database {
        None if privilege == Privilege::Execute => {
            return format!("function {target} (database not reported)");
        }
        None => return format!("collection or function {target} (database not reported)"),
        Some(EVERY) => "every database",
        Some(USER_DATABASES) => "each database this user can open",
        Some(HOLDER_DATABASES) => "each database its holder can open",
        Some(db) if target == EVERY => return format!("Every collection in {db}"),
        Some(db) => return format!("{db}.{target}"),
    };
    if target == EVERY {
        format!("Every collection in {databases}")
    } else {
        format!("{target} in {databases}")
    }
}

/// One privilege a role holds on one collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleGrant {
    pub role: String,
    /// `EVERY` for every database, `USER_DATABASES` or `HOLDER_DATABASES`
    /// for the databases the holder can open. `None`: the server does not
    /// report it.
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
    /// Empty when the role is not tied to a database: it applies in each
    /// database the principal can open.
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
    /// As `RoleGrant::database`.
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

/// Direct rows first. The sort is stable, so grants keep their stored order.
pub fn sort_effective(rows: &mut [EffectiveGrant]) {
    rows.sort_by_key(|r| !r.is_direct());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scope_names_its_database_or_says_it_is_not_reported() {
        let s = Privilege::Select;
        assert_eq!(scope_text(Some("db2"), "users", s), "db2.users");
        assert_eq!(
            scope_text(None, "users", s),
            "collection or function users (database not reported)"
        );
        assert_eq!(
            scope_text(None, "score", Privilege::Execute),
            "function score (database not reported)"
        );
        assert_eq!(
            scope_text(Some(EVERY), EVERY, s),
            "Every collection in every database"
        );
        assert_eq!(scope_text(Some("db2"), EVERY, s), "Every collection in db2");
        assert_eq!(
            scope_text(Some(USER_DATABASES), EVERY, s),
            "Every collection in each database this user can open"
        );
        assert_eq!(
            scope_text(Some(HOLDER_DATABASES), EVERY, s),
            "Every collection in each database its holder can open"
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
