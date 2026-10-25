//! `commerce_graph`: customers, their orders, the products in them, and the
//! articles that mention a product.

use crate::data::mock::explorer::build::{dec, s, ts};
use crate::data::mock::graph::store::{StoredGraph, edge, edge_at, node, order, user};
use crate::models::graph::{GraphBinding, GraphNode, NodeKind, Relation};

pub const NAME: &str = "commerce_graph";

fn product(id: &str, name: &str, price: &str, inbound: u64) -> GraphNode {
    node(
        NodeKind::Product,
        id,
        Some(name),
        vec![
            ("name", s(name)),
            ("price", dec(price)),
            ("currency", s("USD")),
        ],
        (0, inbound),
    )
}

fn article(id: &str, title: &str, excerpt: &str, mentions: u64) -> GraphNode {
    node(
        NodeKind::Article,
        id,
        None,
        vec![
            ("title", s(title)),
            ("excerpt", s(excerpt)),
            ("published_at", ts(2026, 5, 28, 8, 0, 0)),
        ],
        (mentions, 0),
    )
}

pub fn graph() -> StoredGraph {
    let u44182 = user("u44182", "Hana Idris", "MY", (6, 0));
    let u31877 = user("u31877", "Liam Chen", "MY", (2, 0));
    let u90211 = user("u90211", "Noah Patil", "SG", (1, 0));
    let u77654 = user("u77654", "Emma Rossi", "SG", (1, 0));
    let u55319 = user("u55319", "Olivia Wang", "MY", (1, 0));

    // Totals above 100 are the ones the Builder's default filter keeps.
    let o104 = order("o104", "249.99", "shipped", (6, 13, 10, 24, 17));
    let o102 = order("o102", "1250.00", "paid", (6, 12, 14, 33, 21));
    let o100 = order("o100", "319.00", "shipped", (6, 10, 16, 41, 55));
    let o099 = order("o099", "180.00", "shipped", (6, 9, 11, 2, 40));
    let o103 = order("o103", "89.50", "processing", (6, 12, 18, 5, 3));
    let o101 = order("o101", "67.20", "cancelled", (6, 11, 9, 14, 9));
    let o087 = order("o087", "89.00", "shipped", (6, 9, 16, 5, 33));
    let o063 = order("o063", "45.00", "shipped", (6, 8, 9, 17, 48));
    let o041 = order("o041", "59.00", "shipped", (6, 6, 18, 27, 5));
    let o021 = order("o021", "34.50", "shipped", (6, 5, 9, 55, 41));
    let o007 = order("o007", "75.00", "shipped", (6, 2, 10, 14, 36));

    let p12 = product("p12", "Wireless Headphones", "129.00", 3);
    let p77 = product("p77", "Travel Case", "24.00", 1);
    let p31 = product("p31", "Bluetooth Speaker", "89.00", 4);

    let a10023 = article(
        "a10023",
        "A guide to wireless headphones",
        "Wireless headphones offer freedom and excellent sound quality. This guide covers key features, popular models, and how to choose the right pair for your needs...",
        1,
    );
    let a10456 = article(
        "a10456",
        "Noise-cancelling earbuds review",
        "We tested twelve pairs of noise-cancelling earbuds for comfort, battery life, and call quality, and a travel case for each...",
        1,
    );
    let a09871 = article(
        "a09871",
        "Bluetooth speaker comparison",
        "Portable Bluetooth speakers differ most in battery life, water resistance, and how they handle bass at volume...",
        0,
    );

    let edges = vec![
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
        edge_at(
            "e_6e0f1d98",
            Relation::Purchased,
            &u44182,
            &o041,
            (6, 6, 18, 27, 5),
        ),
        edge_at(
            "e_d9f4e120",
            Relation::Purchased,
            &u44182,
            &o021,
            (6, 5, 9, 55, 41),
        ),
        edge_at(
            "e_5f6e3c77",
            Relation::Purchased,
            &u44182,
            &o007,
            (6, 2, 10, 14, 36),
        ),
        edge_at(
            "e_c1a05b02",
            Relation::Purchased,
            &u31877,
            &o102,
            (6, 12, 14, 33, 21),
        ),
        edge_at(
            "e_c1a05b03",
            Relation::Purchased,
            &u90211,
            &o100,
            (6, 10, 16, 41, 55),
        ),
        edge_at(
            "e_c1a05b04",
            Relation::Purchased,
            &u77654,
            &o099,
            (6, 9, 11, 2, 40),
        ),
        edge_at(
            "e_c1a05b05",
            Relation::Purchased,
            &u31877,
            &o103,
            (6, 12, 18, 5, 3),
        ),
        edge_at(
            "e_c1a05b06",
            Relation::Purchased,
            &u55319,
            &o101,
            (6, 11, 9, 14, 9),
        ),
        edge("e_c2b06c01", Relation::Contains, &o104, &p12),
        edge("e_c2b06c02", Relation::Contains, &o104, &p77),
        edge("e_c2b06c03", Relation::Contains, &o102, &p31),
        edge("e_c2b06c04", Relation::Contains, &o100, &p31),
        edge("e_c2b06c05", Relation::Contains, &o099, &p31),
        edge("e_c3c07d01", Relation::Mentions, &a10023, &p12),
        edge("e_c3c07d02", Relation::Mentions, &a10456, &p77),
    ];

    StoredGraph {
        binding: GraphBinding {
            name: NAME.to_string(),
            kinds: vec![
                NodeKind::User,
                NodeKind::Order,
                NodeKind::Product,
                NodeKind::Article,
            ],
            relations: vec![Relation::Purchased, Relation::Contains, Relation::Mentions],
            nodes: 482_613,
            edges: 1_904_227,
        },
        nodes: vec![
            u44182, u31877, u90211, u77654, u55319, o104, o102, o100, o099, o103, o101, o087, o063,
            o041, o021, o007, p12, p77, p31, a10023, a10456, a09871,
        ],
        edges,
    }
}
