use crate::models::collection::{Attached, Collection, Profile};

fn entry(
    name: &str,
    profile: Profile,
    attached: &[Attached],
    description: &str,
    rows: Option<u64>,
) -> Collection {
    Collection {
        name: name.to_string(),
        profile,
        attached: attached.to_vec(),
        description: description.to_string(),
        rows,
    }
}

/// The catalog, in the order the Explorer lists it.
pub fn collections() -> Vec<Collection> {
    vec![
        entry(
            "users",
            Profile::Document,
            &[Attached::Graph],
            "Source collection for user accounts and profiles.",
            Some(1284),
        ),
        entry(
            "orders",
            Profile::DocumentStrict,
            &[],
            "Customer orders with enforced field types.",
            Some(5),
        ),
        entry(
            "sales",
            Profile::Timeseries,
            &[],
            "Point-of-sale line items ordered by sale time.",
            Some(12_482),
        ),
        entry(
            "places",
            Profile::Spatial,
            &[],
            "Points of interest with WGS 84 geometry.",
            Some(104),
        ),
        entry(
            "articles",
            Profile::Document,
            &[Attached::FullText, Attached::Vector],
            "Articles with text content and embeddings for semantic search.",
            Some(104),
        ),
        entry(
            "sessions",
            Profile::KeyValue,
            &[],
            "Session state, counters, and feature flags.",
            Some(51_003),
        ),
        entry(
            "inventory",
            Profile::Columnar,
            &[],
            "Stock levels and prices per warehouse.",
            Some(9482),
        ),
        entry(
            "climate_cube",
            Profile::Array,
            &[],
            "Hourly air temperature on a latitude and longitude grid.",
            None,
        ),
        entry(
            "shipments",
            Profile::DocumentStrict,
            &[],
            "Shipments between warehouses and customers.",
            Some(312),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let mut names: Vec<String> = collections().into_iter().map(|c| c.name).collect();
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(n, names.len());
    }

    #[test]
    fn capabilities_attach_to_documents_and_never_stand_alone() {
        for c in collections() {
            if !c.attached.is_empty() {
                assert_eq!(
                    c.profile.header_badges()[0],
                    "Document",
                    "{} attaches a capability to a non-document profile",
                    c.name
                );
            }
        }
    }
}
