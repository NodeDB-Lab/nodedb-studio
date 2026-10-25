//! `users` (Document), `orders` (Document, strict), `articles` (Document).

use nodedb_types::Value;

use crate::data::mock::explorer::build::{Dataset, at, dec, int, s, ts};
use crate::models::format::epoch;
use crate::models::records::Record;

const USERS: usize = 1284;

/// Id, name, email, month/day/hour/minute/second of 2026, revision.
type Author = (
    &'static str,
    &'static str,
    &'static str,
    (u32, u32, u32, u32, u32),
    u64,
);
/// Users the Explorer counts as active.
pub const ACTIVE_USERS: usize = 842;

const FIRST: [&str; 12] = [
    "Aisha", "Ben", "Chloe", "Daniel", "Elif", "Farid", "Grace", "Hugo", "Ines", "Jamal", "Keiko",
    "Lars",
];
const LAST: [&str; 11] = [
    "Abdullah", "Berg", "Costa", "Dubois", "Evans", "Fischer", "Gupta", "Hansen", "Ito", "Jones",
    "Khan",
];

fn user_status(i: usize) -> &'static str {
    // 37 is coprime with 1284, so the first 842 values of this walk are
    // 842 distinct rows: exactly `ACTIVE_USERS` rows are active.
    if (i * 37) % USERS < ACTIVE_USERS {
        "active"
    } else if i.is_multiple_of(2) {
        "pending"
    } else {
        "suspended"
    }
}

fn profile(locale: &str, zone: &str) -> Value {
    Value::Object(
        [("locale", locale), ("timezone", zone)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), s(v)))
            .collect(),
    )
}

fn user(i: usize) -> Record {
    let authored: [Author; 5] = [
        (
            "u44182",
            "Hana Idris",
            "hana@example.com",
            (6, 13, 12, 41, 2),
            18,
        ),
        (
            "u31877",
            "Liam Chen",
            "liam@example.com",
            (6, 12, 9, 17, 36),
            7,
        ),
        (
            "u90211",
            "Noah Patil",
            "noah@example.com",
            (6, 11, 18, 3, 21),
            4,
        ),
        (
            "u77654",
            "Emma Rossi",
            "emma@example.com",
            (6, 10, 16, 28, 14),
            9,
        ),
        (
            "u55319",
            "Olivia Wang",
            "olivia@example.com",
            (6, 9, 11, 5, 47),
            2,
        ),
    ];
    if let Some((id, name, email, (mo, d, h, mi, sec), rev)) = authored.get(i) {
        let updated = ts(2026, *mo, *d, *h, *mi, *sec);
        return Record::new(
            id,
            *rev,
            vec![
                ("id", s(id)),
                ("name", s(name)),
                ("email", s(email)),
                ("status", s("active")),
                ("role", s("user")),
                ("created_at", ts(2024, 11, 3, 8, 24, 17)),
                ("updated_at", updated),
                ("profile", profile("en", "Asia/Singapore")),
            ],
        );
    }
    let id = format!("u{:05}", 10_000 + (i * 7919) % 80_000);
    let first = FIRST[i % FIRST.len()];
    let last = LAST[(i / FIRST.len()) % LAST.len()];
    let updated = epoch(2026, 6, 8, 23, 0, 0) - (i as i64) * 2_731;
    Record::new(
        &id,
        1 + (i % 6) as u64,
        vec![
            ("id", s(&id)),
            ("name", s(&format!("{first} {last}"))),
            (
                "email",
                s(&format!(
                    "{}.{}{i}@example.com",
                    first.to_lowercase(),
                    last.to_lowercase()
                )),
            ),
            ("status", s(user_status(i))),
            (
                "role",
                s(if i.is_multiple_of(17) {
                    "admin"
                } else {
                    "user"
                }),
            ),
            (
                "created_at",
                at(epoch(2025, 1, 1, 0, 0, 0) + (i as i64) * 86_400 / 3),
            ),
            ("updated_at", at(updated)),
            ("profile", profile("en", "UTC")),
        ],
    )
}

pub const USERS_SET: Dataset = Dataset {
    total: USERS,
    row: user,
};

fn address(i: usize) -> Value {
    let (name, line, city, country, post) = [
        (
            "Aziz Karim",
            "12 Jalan Ampang",
            "Kuala Lumpur",
            "MY",
            "50450",
        ),
        ("Mei Tan", "8 Orchard Road", "Singapore", "SG", "238823"),
        ("Rosa Diaz", "400 Pine Street", "Seattle", "US", "98101"),
    ][i % 3];
    Value::Object(
        [
            ("name", name),
            ("line1", line),
            ("city", city),
            ("country", country),
            ("postcode", post),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), s(v)))
        .collect(),
    )
}

fn order(i: usize) -> Record {
    let rows = [
        ("o104", "c203", "249.99", "shipped", (6, 13, 10, 24, 17), 18),
        ("o103", "c178", "89.50", "processing", (6, 12, 18, 5, 3), 5),
        ("o102", "c203", "1250.00", "paid", (6, 12, 14, 33, 21), 9),
        ("o101", "c155", "67.20", "cancelled", (6, 11, 9, 14, 9), 4),
        ("o100", "c178", "319.00", "shipped", (6, 10, 16, 41, 55), 6),
    ];
    let (id, customer, total, status, (mo, d, h, mi, sec), rev) = rows[i % rows.len()];
    let items = |n: i64| {
        Value::Array(
            (1..=n)
                .map(|k| {
                    Value::Object(
                        [
                            ("sku", s(&format!("sku{}{k}", 100 + i))),
                            ("qty", int(k)),
                            ("unit_price", dec("19.99")),
                        ]
                        .into_iter()
                        .map(|(key, v)| (key.to_string(), v))
                        .collect(),
                    )
                })
                .collect(),
        )
    };
    Record::new(
        id,
        rev,
        vec![
            ("order_id", s(id)),
            ("customer_id", s(customer)),
            ("total", dec(total)),
            ("currency", s("USD")),
            ("status", s(status)),
            ("created_at", ts(2026, mo, d, h, mi, sec)),
            ("items", items(if i == 0 { 3 } else { 1 })),
            ("shipping_address", address(i)),
            (
                "metadata",
                Value::Object(
                    [("channel", "web"), ("coupon", "none")]
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), s(v)))
                        .collect(),
                ),
            ),
        ],
    )
}

pub const ORDERS_SET: Dataset = Dataset {
    total: 5,
    row: order,
};

const TITLES: [&str; 10] = [
    "Distributed databases",
    "Vector search basics",
    "Building with NodeDB",
    "Getting started",
    "Data modeling guide",
    "Query language overview",
    "Indexing strategies",
    "Scaling your cluster",
    "Backup and restore",
    "Monitoring and metrics",
];
const TAIL_TITLES: [&str; 8] = [
    "Replication and consensus",
    "Hybrid search with fusion",
    "Time series retention",
    "Spatial queries in practice",
    "Change feeds explained",
    "Security and access control",
    "Schema evolution",
    "Performance tuning",
];

/// A deterministic 768-component unit embedding that starts with `head`.
fn embedding(i: usize, head: [f32; 2]) -> Value {
    let mut v: Vec<f32> = (0..768)
        .map(|k| (crate::data::mock::explorer::build::unit(i * 768 + k, 11) as f32 - 0.5) * 0.2)
        .collect();
    v[0] = head[0];
    v[1] = head[1];
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    for x in &mut v {
        *x /= norm * 7.0;
    }
    v[0] = head[0];
    v[1] = head[1];
    Value::Vector(v.into())
}

fn article(i: usize) -> Record {
    let n = 104 - i;
    let id = format!("art{n:03}");
    let heads: [[f32; 2]; 10] = [
        [0.021, -0.104],
        [0.008, 0.036],
        [-0.117, 0.049],
        [0.063, -0.021],
        [-0.028, 0.076],
        [0.014, -0.093],
        [-0.051, 0.022],
        [0.034, -0.066],
        [-0.009, 0.041],
        [0.072, -0.018],
    ];
    let title = TITLES
        .get(i)
        .copied()
        .unwrap_or_else(|| TAIL_TITLES[i % TAIL_TITLES.len()]);
    let updated = epoch(2026, 6, 12, 10, 24, 31) - (i as i64) * 61_200;
    let embedded = if i == 0 {
        epoch(2026, 6, 12, 14, 23, 11)
    } else {
        updated + 14_400
    };
    let body = if i == 0 {
        "Distributed databases enable scalable, fault-tolerant applications across multiple regions...".to_string()
    } else {
        format!("{title}: a guide to the concepts, trade-offs, and operations involved.")
    };
    Record::new(
        &id,
        if i == 0 { 4 } else { 1 + (i % 3) as u64 },
        vec![
            ("_id", s(&id)),
            ("title", s(title)),
            ("body", s(&body)),
            ("language", s("en")),
            ("updated_at", at(updated)),
            (
                "embedding",
                embedding(i, heads.get(i).copied().unwrap_or([0.01, -0.02])),
            ),
            ("model", s("support-embed-v2")),
            ("embedded_at", at(embedded)),
        ],
    )
}

pub const ARTICLES_SET: Dataset = Dataset {
    total: 104,
    row: article,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(set: Dataset) -> Vec<String> {
        (0..set.total).map(|i| (set.row)(i).id).collect()
    }

    #[test]
    fn ids_are_unique_in_every_dataset() {
        for set in [USERS_SET, ORDERS_SET, ARTICLES_SET] {
            let mut all = ids(set);
            let n = all.len();
            all.sort();
            all.dedup();
            assert_eq!(n, all.len());
        }
    }

    #[test]
    fn exactly_842_users_are_active() {
        let active = (0..USERS_SET.total)
            .filter(|i| (USERS_SET.row)(*i).get("status") == Some(&s("active")))
            .count();
        assert_eq!(active, ACTIVE_USERS);
    }

    #[test]
    fn the_first_user_is_hana_at_revision_18() {
        let r = user(0);
        assert_eq!((r.id.as_str(), r.revision), ("u44182", 18));
    }

    #[test]
    fn embeddings_have_768_components() {
        match article(0).get("embedding") {
            Some(Value::Vector(v)) => assert_eq!(v.len(), 768),
            other => panic!("not a vector: {other:?}"),
        }
    }
}
