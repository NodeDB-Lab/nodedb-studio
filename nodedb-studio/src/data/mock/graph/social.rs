//! `social_graph`: the graph binding on `users`. People post, posts carry
//! topics, people follow each other, and people purchase orders.

use crate::data::mock::explorer::build::{s, ts};
use crate::data::mock::graph::store::{StoredGraph, edge, edge_at, node, order, user};
use crate::models::graph::{GraphBinding, NodeKind, Relation};

pub const NAME: &str = "social_graph";

fn post(id: &str, title: &str, day: u32) -> crate::models::graph::GraphNode {
    node(
        NodeKind::Post,
        id,
        None,
        vec![
            ("title", s(title)),
            ("created_at", ts(2026, 6, day, 9, 30, 0)),
        ],
        (1, 12),
    )
}

fn topic(name: &str, inbound: u64) -> crate::models::graph::GraphNode {
    node(
        NodeKind::Topic,
        name,
        None,
        vec![("name", s(name))],
        (0, inbound),
    )
}

pub fn graph() -> StoredGraph {
    let u44182 = user("u44182", "Hana Idris", "MY", (412, 88));
    let u31877 = user("u31877", "Liam Chen", "MY", (96, 41));
    let u90211 = user("u90211", "Noah Patil", "MY", (58, 20));
    let u77654 = user("u77654", "Emma Rossi", "SG", (131, 64));
    let u55319 = user("u55319", "Olivia Wang", "MY", (27, 15));

    let p8281 = post("p8281", "Choosing a shard key", 12);
    let p8302 = post("p8302", "Reading the query plan", 11);
    let p8317 = post("p8317", "Tuning the write buffer", 10);
    let p8344 = post("p8344", "Backups you can restore", 9);
    let p8390 = post("p8390", "Why we moved to Raft", 8);
    let p8402 = post("p8402", "Lifetimes in async code", 7);

    let databases = topic("databases", 4_211);
    let rust = topic("rust", 1_874);

    let o104 = order("o104", "249.99", "shipped", (6, 13, 10, 24, 17));
    let o087 = order("o087", "89.00", "shipped", (6, 9, 16, 5, 33));
    let o063 = order("o063", "45.00", "shipped", (6, 8, 9, 17, 48));

    let edges = vec![
        edge("e_5a10c001", Relation::Posted, &u44182, &p8281),
        edge("e_5a10c002", Relation::Posted, &u44182, &p8302),
        edge("e_5a10c003", Relation::Posted, &u31877, &p8317),
        edge("e_5a10c004", Relation::Posted, &u90211, &p8344),
        edge("e_5a10c005", Relation::Posted, &u77654, &p8390),
        edge("e_5a10c006", Relation::Posted, &u55319, &p8402),
        edge("e_5b20d001", Relation::Tagged, &p8281, &databases),
        edge("e_5b20d002", Relation::Tagged, &p8302, &databases),
        edge("e_5b20d003", Relation::Tagged, &p8317, &databases),
        edge("e_5b20d004", Relation::Tagged, &p8344, &databases),
        edge("e_5b20d005", Relation::Tagged, &p8390, &databases),
        edge("e_5b20d006", Relation::Tagged, &p8402, &rust),
        edge("e_5c30e001", Relation::Follows, &u44182, &u31877),
        edge("e_5c30e002", Relation::Follows, &u31877, &u44182),
        edge("e_5c30e003", Relation::Follows, &u55319, &u44182),
        edge("e_5c30e004", Relation::Follows, &u77654, &u90211),
        edge_at(
            "e_7f3a2d59",
            Relation::Purchased,
            &u44182,
            &o104,
            (6, 10, 14, 22, 11),
        ),
        edge_at(
            "e_9b8c1a04",
            Relation::Purchased,
            &u44182,
            &o087,
            (6, 9, 16, 5, 33),
        ),
        edge_at(
            "e_1d9e4f66",
            Relation::Purchased,
            &u44182,
            &o063,
            (6, 8, 9, 17, 48),
        ),
    ];

    StoredGraph {
        binding: GraphBinding {
            name: NAME.to_string(),
            kinds: vec![
                NodeKind::User,
                NodeKind::Post,
                NodeKind::Topic,
                NodeKind::Order,
            ],
            relations: vec![
                Relation::Posted,
                Relation::Tagged,
                Relation::Follows,
                Relation::Purchased,
            ],
            nodes: 3_200_000,
            edges: 18_400_000,
        },
        nodes: vec![
            u44182, u31877, u90211, u77654, u55319, p8281, p8302, p8317, p8344, p8390, p8402,
            databases, rust, o104, o087, o063,
        ],
        edges,
    }
}
