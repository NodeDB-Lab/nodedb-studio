use crate::models::records::{CollectionAccess, Denial, Grant};

/// The session's rights over `collection`. A read-only session reads and
/// nothing else. A timeseries collection takes inserts and never rewrites
/// or removes a row, whoever asks.
pub fn collection_access(
    collection: &str,
    user: &str,
    read_only: bool,
) -> Option<CollectionAccess> {
    let profile = super::catalog::collections()
        .into_iter()
        .find(|c| c.name == collection)?
        .profile;
    let write = |policy: bool| {
        if read_only {
            Grant::Denied(Denial::Privilege)
        } else if policy {
            Grant::Denied(Denial::Policy("Append-only collection".to_string()))
        } else {
            Grant::Allowed
        }
    };
    let append_only = profile.append_only();
    Some(CollectionAccess {
        role: user.to_string(),
        create: write(false),
        read: Grant::Allowed,
        update: write(append_only),
        delete: write(append_only),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_only_session_cannot_insert() {
        let a = collection_access("shipments", "analyst", true).expect("known");
        assert!(a.read.allowed());
        assert_eq!(a.create, Grant::Denied(Denial::Privilege));
        assert_eq!(
            a.create.headline("INSERT").as_deref(),
            Some("INSERT permission required")
        );
    }

    #[test]
    fn timeseries_rows_are_never_rewritten() {
        let a = collection_access("sales", "admin", false).expect("known");
        assert!(a.create.allowed());
        assert!(!a.update.allowed() && !a.delete.allowed());
    }

    #[test]
    fn an_unknown_collection_has_no_access_record() {
        assert!(collection_access("nope", "admin", false).is_none());
    }
}
