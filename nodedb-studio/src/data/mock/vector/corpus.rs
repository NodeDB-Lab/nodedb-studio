//! The articles behind `articles.embedding`.
//!
//! The index holds 1,108,492 vectors. The first 104 are the Explorer's
//! articles, with their titles. The rest are generated: each record's text,
//! language, and vector are pure functions of its number, so any record can
//! be read without storing a million of them.
//!
//! A vector is built from shared directions: a group, a topic, a subtopic,
//! and a language, plus noise that belongs to the record alone. Records that
//! share more of them are closer, so nearest neighbors and a projection show
//! real structure that the fixture put there, and that the code finds by
//! measuring.

use std::sync::OnceLock;

use nodedb_types::Value;

use crate::data::mock::explorer::{ARTICLE_COUNT, article_row};
use crate::models::format::epoch;

pub const COLLECTION: &str = "articles";
pub const FIELD: &str = "embedding";
pub const INDEX: &str = "idx_articles_embedding";
pub const MODEL: &str = "support-embed-v2";
pub const DIMENSIONS: usize = 768;
/// Vectors the index reports as live.
pub const POPULATION: u64 = 1_108_492;
/// Records a mock search scans. A real index answers from its graph.
pub const RESIDENT: u64 = 2_000;
pub const REVISION: u64 = 412;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Topic {
    Architecture,
    Search,
    Modeling,
    Operations,
    Events,
    Basics,
    Spatial,
}

impl Topic {
    const ALL: [Topic; 7] = [
        Topic::Architecture,
        Topic::Search,
        Topic::Modeling,
        Topic::Operations,
        Topic::Events,
        Topic::Basics,
        Topic::Spatial,
    ];

    fn index(self) -> usize {
        Topic::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// Topics that read alike share a group, so the group is the widest split.
    fn group(self) -> usize {
        match self {
            Topic::Architecture | Topic::Operations => 0,
            Topic::Search | Topic::Modeling | Topic::Basics => 1,
            Topic::Events | Topic::Spatial => 2,
        }
    }

    fn titles(self) -> &'static [&'static str] {
        match self {
            Topic::Architecture => &[
                "Distributed databases",
                "Replication and consensus",
                "Scaling your cluster",
                "Distributed database architecture",
                "Database replication explained",
                "Scaling distributed queries",
                "Database consistency models",
            ],
            Topic::Search => &[
                "Vector search basics",
                "Hybrid search with fusion",
                "Indexing strategies",
            ],
            Topic::Modeling => &[
                "Data modeling guide",
                "Schema evolution",
                "Query language overview",
            ],
            Topic::Operations => &[
                "Backup and restore",
                "Monitoring and metrics",
                "Performance tuning",
                "Security and access control",
                "Storage engines overview",
            ],
            Topic::Events => &["Change feeds explained", "Time series retention"],
            Topic::Basics => &["Getting started", "Building with NodeDB"],
            Topic::Spatial => &["Spatial queries in practice"],
        }
    }

    /// Passages a generated record's body is drawn from. They read like the
    /// text of a documentation page, so a keyword search finds varied matches.
    fn passages(self) -> &'static [&'static str] {
        match self {
            Topic::Architecture => &[
                "A distributed database partitions data across multiple nodes to provide high availability, scalability, and fault tolerance in modern applications.",
                "Consensus lets the nodes of a distributed database agree on one order of writes, so a copy that falls behind can catch up safely.",
                "Scaling a cluster adds nodes and moves partitions to them, while queries keep running against the database.",
                "The architecture of a distributed database splits storage, query planning, and coordination into layers that scale on their own.",
                "Replication in a distributed database ensures data consistency and availability by maintaining copies across multiple nodes and regions.",
                "Query planning in a distributed database must account for data locality, network latency, and parallel execution across many nodes.",
                "A distributed database can implement different consistency models, including strong, eventual, and causal consistency, depending on application requirements.",
            ],
            Topic::Search => &[
                "Vector search ranks records by the distance between embeddings, and a filter narrows the candidates before the top results are cut.",
                "Hybrid search fuses a keyword ranking with a vector ranking, so a record that both rankings favor rises to the top of the results.",
                "An index trades build time and storage for faster reads, and each index kind suits a different query shape.",
            ],
            Topic::Modeling => &[
                "A data model names the collections, fields, and relations an application stores, and a schema makes those names enforceable.",
                "Schema evolution adds, renames, and retires fields without rewriting every stored record at once.",
                "The query language reads records with filters, joins, and aggregates, and returns rows in a defined order.",
            ],
            Topic::Operations => &[
                "Backups copy a consistent snapshot of the database, and a restore replays it to a chosen point in time.",
                "Monitoring tracks latency, throughput, and error rates, and an alert fires when a metric leaves its expected range.",
                "Performance tuning starts from measured query plans and changes one setting at a time.",
                "Access control grants each role only the privileges it needs, and every grant is recorded in the audit log.",
                "Storage engines write records to disk in different layouts, which changes how fast a database reads, writes, and compacts them.",
            ],
            Topic::Events => &[
                "A change feed delivers each committed write to its consumers in order, so a downstream system can follow the database.",
                "Time series retention drops raw points after a set age and keeps the rolled-up aggregates.",
            ],
            Topic::Basics => &[
                "Getting started covers installing the server, connecting a client, and creating the first collection.",
                "Building an application with the database combines documents, indexes, and queries in one process.",
            ],
            Topic::Spatial => &[
                "Spatial queries find features near a point or inside an area, using an index over their geometry.",
            ],
        }
    }

    /// The topic whose titles include `title`.
    fn of_title(title: &str) -> Topic {
        Topic::ALL
            .into_iter()
            .find(|t| t.titles().contains(&title))
            .unwrap_or(Topic::Basics)
    }
}

/// What the catalog says about one article, apart from its vector.
#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    pub n: u64,
    pub title: String,
    pub body: String,
    /// `None` for a record with no language.
    pub language: Option<&'static str>,
    pub model: &'static str,
    pub embedded_at: i64,
    topic: Topic,
    sub: usize,
}

pub fn id_of(n: u64) -> String {
    format!("art{n:03}")
}

/// The number in a record id, when the index holds that record.
pub fn n_of(id: &str) -> Option<u64> {
    let n: u64 = id.strip_prefix("art")?.parse().ok()?;
    (1..=POPULATION).contains(&n).then_some(n)
}

fn mix(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 27;
    x = x.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    x ^ (x >> 32)
}

/// A float in `[-0.5, 0.5)` from two keys.
fn unit(a: u64, b: u64) -> f32 {
    ((mix(a, b) >> 40) as f32 / (1u64 << 24) as f32) - 0.5
}

fn language_of(n: u64) -> Option<&'static str> {
    match mix(n, 7) % 100 {
        0..=2 => None,
        3..=84 => Some("en"),
        85..=94 => Some("ms"),
        _ => Some("de"),
    }
}

fn explorer_text(row: &crate::models::records::Record, field: &str) -> String {
    match row.get(field) {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn explorer_metas() -> &'static Vec<Meta> {
    static METAS: OnceLock<Vec<Meta>> = OnceLock::new();
    METAS.get_or_init(|| {
        (1..=ARTICLE_COUNT as u64)
            .map(|n| {
                let row = article_row(ARTICLE_COUNT - n as usize);
                let title = explorer_text(&row, "title");
                let topic = Topic::of_title(&title);
                Meta {
                    n,
                    body: explorer_text(&row, "body"),
                    language: Some("en"),
                    model: MODEL,
                    embedded_at: epoch(2026, 6, 12, 10, 24, 31) + n as i64 * 60,
                    sub: (n % 3) as usize,
                    topic,
                    title,
                }
            })
            .collect()
    })
}

/// The catalog entry of record `n`. `n` is in `1..=POPULATION`.
pub fn meta(n: u64) -> Meta {
    if let Some(m) = n
        .checked_sub(1)
        .and_then(|i| explorer_metas().get(usize::try_from(i).unwrap_or(usize::MAX)))
    {
        return m.clone();
    }
    let topic = Topic::ALL[(mix(n, 1) % Topic::ALL.len() as u64) as usize];
    let titles = topic.titles();
    let pick = (mix(n, 2) % titles.len() as u64) as usize;
    let title = titles[pick];
    let passages = topic.passages();
    Meta {
        n,
        title: format!("{title}, part {}", 1 + mix(n, 3) % 24),
        body: passages[pick % passages.len()].to_string(),
        language: language_of(n),
        model: MODEL,
        embedded_at: epoch(2026, 6, 12, 10, 24, 31) - (n % 40_000) as i64 * 31,
        topic,
        sub: (mix(n, 4) % 3) as usize,
    }
}

/// Shared directions, built once.
struct Directions {
    group: Vec<Vec<f32>>,
    topic: Vec<Vec<f32>>,
    sub: Vec<Vec<f32>>,
    language: Vec<Vec<f32>>,
}

fn direction(kind: u64, id: u64) -> Vec<f32> {
    (0..DIMENSIONS as u64)
        .map(|k| unit(kind * 1_000 + id, k))
        .collect()
}

fn directions() -> &'static Directions {
    static D: OnceLock<Directions> = OnceLock::new();
    D.get_or_init(|| Directions {
        group: (0..3).map(|i| direction(1, i)).collect(),
        topic: (0..7).map(|i| direction(2, i)).collect(),
        sub: (0..21).map(|i| direction(3, i)).collect(),
        language: (0..4).map(|i| direction(4, i)).collect(),
    })
}

fn language_slot(language: Option<&str>) -> usize {
    match language {
        Some("en") => 0,
        Some("ms") => 1,
        Some("de") => 2,
        _ => 3,
    }
}

/// The embedding of record `n`, scaled to unit length.
pub fn vector_of(n: u64) -> Vec<f32> {
    let m = meta(n);
    let d = directions();
    let (g, t) = (m.topic.group(), m.topic.index());
    let s = t * 3 + m.sub;
    let l = language_slot(m.language);
    // Most records lean toward a second topic, some a lot, so the sample is a
    // continuum with dense cores instead of separate balls.
    let t2 = (mix(n, 9) % 7) as usize;
    let lean = ((mix(n, 10) >> 40) as f32 / (1u64 << 24) as f32).powi(2) * 1.4;
    let mut v: Vec<f32> = (0..DIMENSIONS)
        .map(|k| {
            lean * d.topic[t2][k]
                + 0.8 * d.group[g][k]
                + 0.8 * d.topic[t][k]
                + 0.6 * d.sub[s][k]
                + 0.15 * d.language[l][k]
                + 0.7 * unit(n.wrapping_add(0x5EED), k as u64)
        })
        .collect();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

/// The records a mock search scans, as one flat block of unit vectors.
pub fn resident() -> &'static [f32] {
    static BLOCK: OnceLock<Vec<f32>> = OnceLock::new();
    BLOCK.get_or_init(|| (1..=RESIDENT).flat_map(vector_of).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    #[test]
    fn ids_round_trip_and_stay_in_range() {
        assert_eq!(id_of(104), "art104");
        assert_eq!(n_of("art104"), Some(104));
        assert_eq!(n_of("art0"), None);
        assert_eq!(n_of("art1108493"), None);
        assert_eq!(n_of("user44"), None);
    }

    #[test]
    fn the_first_records_are_the_explorers_articles() {
        let m = meta(104);
        assert_eq!(m.title, "Distributed databases");
        assert!(m.body.starts_with("Distributed databases enable"));
        assert_eq!(meta(1).title, meta(1).title);
    }

    #[test]
    fn a_vector_has_768_values_and_unit_length() {
        let v = vector_of(104);
        assert_eq!(v.len(), DIMENSIONS);
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn records_of_one_topic_are_closer_than_records_of_another() {
        let a = 104;
        let same = (1..=RESIDENT)
            .filter(|n| *n != a && meta(*n).topic == meta(a).topic)
            .take(30);
        let other = (1..=RESIDENT)
            .filter(|n| meta(*n).topic.group() != meta(a).topic.group())
            .take(30);
        let va = vector_of(a);
        let near: f32 = same.map(|n| cosine(&va, &vector_of(n))).sum::<f32>() / 30.0;
        let far: f32 = other.map(|n| cosine(&va, &vector_of(n))).sum::<f32>() / 30.0;
        assert!(near > far + 0.3, "near {near}, far {far}");
    }

    #[test]
    fn the_resident_block_holds_every_resident_vector() {
        assert_eq!(resident().len(), RESIDENT as usize * DIMENSIONS);
    }

    #[test]
    fn languages_include_one_missing_value() {
        assert!((1..=RESIDENT).any(|n| meta(n).language.is_none()));
        assert!((1..=RESIDENT).any(|n| meta(n).language == Some("ms")));
    }
}
