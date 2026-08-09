//! Shared fixture-invariant helpers used by more than one mock module's
//! tests. Test-only: this whole module is behind `#[cfg(test)]` at the
//! declaration site in `mod.rs`.

/// Every id in the fixture must be unique — a list keyed by a duplicate id
/// would silently overwrite one row with another in the UI.
pub(crate) fn assert_unique_ids(ids: &[&str]) {
    assert!(!ids.is_empty(), "fixture must not be empty");
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "ids must be unique");
}

/// A list keyed by the wrong field (e.g. `name` instead of `id`) still
/// passes `assert_unique_ids` when the fixture happens to have `id == name`,
/// so that mistake stays invisible. Fixtures should give every row a
/// distinct `id`/`name` pair to make it visible.
pub(crate) fn assert_ids_distinct_from_names<'a>(rows: impl Iterator<Item = (&'a str, &'a str)>) {
    let mut saw_any = false;
    for (id, name) in rows {
        saw_any = true;
        assert_ne!(id, name, "id must not equal name: {id}");
    }
    assert!(saw_any, "fixture must not be empty");
}
