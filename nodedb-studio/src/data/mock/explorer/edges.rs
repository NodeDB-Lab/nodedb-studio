//! The `social_graph` edges over `users`.

use std::collections::BTreeSet;

use nodedb_types::Value;

use crate::data::mock::explorer::build::{at, ts, unit};
use crate::models::format::epoch;
use crate::models::graph_records::{Edge, EdgePage, EdgeQuery, NodeRef};

/// The graph binding of `users`.
pub const GRAPH: &str = "social_graph";

const PURCHASED: usize = 54;

/// Edge id, user, order, month/day/hour/minute/second of 2026.
type Authored = (
    &'static str,
    &'static str,
    &'static str,
    (u32, u32, u32, u32, u32),
);
const FOLLOWS: usize = 12;

fn node(collection: &str, id: &str) -> NodeRef {
    NodeRef {
        collection: collection.to_string(),
        id: id.to_string(),
        kind: match collection {
            "users" => "User",
            _ => "Order",
        }
        .to_string(),
    }
}

fn properties(created: Value) -> Value {
    Value::Object([("created_at".to_string(), created)].into_iter().collect())
}

fn edge(i: usize) -> Edge {
    let authored: [Authored; 10] = [
        ("e_7f3a2d59", "u44182", "o104", (10, 14, 22, 11, 0)),
        ("e_9b8c1a04", "u44182", "o087", (9, 16, 5, 33, 0)),
        ("e_3c6d7e21", "u32811", "o076", (9, 11, 44, 2, 0)),
        ("e_1d9e4f66", "u44182", "o063", (8, 9, 17, 48, 0)),
        ("e_4b2a90cc", "u19002", "o058", (7, 21, 3, 16, 0)),
        ("e_6e0f1d98", "u44182", "o041", (6, 18, 27, 5, 0)),
        ("e_a7c2b335", "u77510", "o037", (6, 12, 11, 9, 0)),
        ("e_d9f4e120", "u44182", "o021", (5, 9, 55, 41, 0)),
        ("e_8c1d7a09", "u66123", "o018", (4, 16, 38, 22, 0)),
        ("e_5f6e3c77", "u44182", "o007", (2, 10, 14, 36, 0)),
    ];
    if let Some((id, user, order, (d, h, mi, sec, _))) = authored.get(i) {
        return Edge {
            id: id.to_string(),
            relation: "PURCHASED".to_string(),
            from: node("users", user),
            to: node("orders", order),
            properties: properties(ts(2026, 6, *d, *h, *mi, *sec)),
        };
    }
    let created = epoch(2026, 6, 2, 10, 14, 36) - (i as i64) * 7_200;
    if i < PURCHASED {
        let user = 10_000 + (i * 7919) % 80_000;
        Edge {
            id: format!("e_{:08x}", 0x2a00_0000u64 + (i as u64) * 7919),
            relation: "PURCHASED".to_string(),
            from: node("users", &format!("u{user}")),
            to: node(
                "orders",
                &format!("o{:03}", 6 - (i as i64 - 10).rem_euclid(6)),
            ),
            properties: properties(at(created)),
        }
    } else {
        let a = 10_000 + (i * 7919) % 80_000;
        let b = 10_000 + (i * 4493 + 17) % 80_000;
        Edge {
            id: format!("e_{:08x}", 0x5c00_0000u64 + (i as u64) * 4493),
            relation: "FOLLOWS".to_string(),
            from: node("users", &format!("u{a}")),
            to: node("users", &format!("u{b}")),
            properties: properties(at(created - 40_000 + (unit(i, 41) * 1000.0) as i64)),
        }
    }
}

fn matches(e: &Edge, query: &EdgeQuery) -> bool {
    if let Some(r) = &query.relation
        && &e.relation != r
    {
        return false;
    }
    let needle = query.text.trim().to_lowercase();
    needle.is_empty()
        || e.id.to_lowercase().contains(&needle)
        || e.from.path().to_lowercase().contains(&needle)
        || e.to.path().to_lowercase().contains(&needle)
}

/// One page of the graph's edges, minus the ones deleted this session.
pub fn edge_page(query: &EdgeQuery, deleted: &BTreeSet<String>) -> EdgePage {
    let all: Vec<Edge> = (0..PURCHASED + FOLLOWS)
        .map(edge)
        .filter(|e| !deleted.contains(&e.id) && matches(e, query))
        .collect();
    let total = all.len() as u64;
    let size = query.page_size.max(1);
    let edges = all.into_iter().skip(query.page * size).take(size).collect();
    EdgePage {
        edges,
        total,
        relations: vec!["PURCHASED".to_string(), "FOLLOWS".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purchased_has_54_edges_and_ids_are_unique() {
        let q = EdgeQuery {
            relation: Some("PURCHASED".into()),
            ..EdgeQuery::new(100)
        };
        let page = edge_page(&q, &BTreeSet::new());
        assert_eq!(page.total, 54);
        let mut ids: Vec<&String> = page.edges.iter().map(|e| &e.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 54);
    }

    #[test]
    fn a_deleted_edge_leaves_the_page() {
        let first = edge(0).id;
        let page = edge_page(&EdgeQuery::new(10), &BTreeSet::from([first.clone()]));
        assert!(page.edges.iter().all(|e| e.id != first));
        assert_eq!(page.total, 65);
    }
}
