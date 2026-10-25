//! Console fixtures: command history and the replies each protocol gives.

use crate::data::mock::workbench::results::result_set;
use crate::models::console::{ConsoleOutput, HistoryEntry, Protocol};
use crate::models::format::epoch;
use crate::models::workbench::QueryRequest;
use crate::services::error::StudioError;

fn entry(n: u32, at: i64, protocol: Protocol, text: &str) -> HistoryEntry {
    HistoryEntry {
        id: format!("h-{n:03}"),
        at,
        protocol,
        text: text.to_string(),
    }
}

/// Seeded history, newest first.
pub fn history() -> Vec<HistoryEntry> {
    use Protocol::{Admin, Cypher, Sql};
    vec![
        entry(
            14,
            epoch(2026, 6, 13, 13, 21, 0),
            Sql,
            "SELECT current_database();",
        ),
        entry(
            13,
            epoch(2026, 6, 13, 13, 18, 42),
            Sql,
            "SELECT count(*) FROM users;",
        ),
        entry(
            12,
            epoch(2026, 6, 13, 13, 12, 17),
            Sql,
            "SELECT id, name FROM users ORDER BY created_at DESC LIMIT 10;",
        ),
        entry(
            11,
            epoch(2026, 6, 13, 13, 4, 51),
            Cypher,
            "MATCH (u:User)-[:PURCHASED]->(p:Product) RETURN u.name, p.name LIMIT 10;",
        ),
        entry(
            10,
            epoch(2026, 6, 13, 12, 55, 3),
            Sql,
            "SELECT * FROM products WHERE status = 'active';",
        ),
        entry(9, epoch(2026, 6, 13, 12, 31, 9), Admin, "SHOW DATABASES;"),
        entry(
            8,
            epoch(2026, 6, 13, 11, 44, 27),
            Sql,
            "SELECT count(*) FROM orders WHERE created_at >= '2026-06-01';",
        ),
        entry(
            7,
            epoch(2026, 6, 13, 10, 2, 33),
            Cypher,
            "MATCH (u:User) RETURN count(u);",
        ),
        entry(
            6,
            epoch(2026, 6, 12, 16, 36, 11),
            Sql,
            "SELECT * FROM schema_version;",
        ),
        entry(
            5,
            epoch(2026, 6, 12, 15, 20, 9),
            Sql,
            "SELECT current_user;",
        ),
        entry(
            4,
            epoch(2026, 6, 12, 14, 10, 33),
            Sql,
            "SELECT name FROM databases;",
        ),
        entry(
            3,
            epoch(2026, 6, 10, 9, 41, 18),
            Admin,
            "SHOW CLUSTER STATUS;",
        ),
        entry(
            2,
            epoch(2026, 6, 9, 17, 3, 55),
            Cypher,
            "MATCH (u:User)-[:FOLLOWS]->(v:User) RETURN u.name, v.name LIMIT 5;",
        ),
        entry(1, epoch(2026, 6, 8, 8, 12, 0), Admin, "SHOW SESSIONS;"),
    ]
}

fn table(columns: &[&str], rows: &[&[&str]]) -> ConsoleOutput {
    ConsoleOutput::Table {
        columns: columns.iter().map(|c| c.to_string()).collect(),
        rows: rows
            .iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect(),
    }
}

/// The statement without its closing semicolon and surrounding space.
pub fn bare(text: &str) -> &str {
    text.trim().trim_end_matches(';').trim()
}

/// What `protocol` answers for `text`, and how long it took in ms.
/// Transaction control is the store's job and never reaches this.
pub fn execute(
    protocol: Protocol,
    database: &str,
    text: &str,
) -> Result<(ConsoleOutput, u32), StudioError> {
    let t = bare(text);
    if t.is_empty() {
        return Err(StudioError::EmptyStatement);
    }
    let lower = t.to_lowercase();
    match protocol {
        Protocol::Sql => Ok(match lower.as_str() {
            "select current_database()" => (table(&["current_database"], &[&[database]]), 12),
            "select count(*) from users" => (table(&["count"], &[&["1284"]]), 18),
            "select current_user" => (table(&["current_user"], &[&["admin"]]), 4),
            _ => {
                let first = lower.split_whitespace().next().unwrap_or_default();
                if matches!(first, "insert" | "update" | "delete") {
                    let tag = if first == "insert" {
                        "INSERT 0 1"
                    } else {
                        "UPDATE 1"
                    };
                    return Ok((ConsoleOutput::Text(vec![tag.to_string()]), 9));
                }
                let rs = result_set(&QueryRequest {
                    sql: t.to_string(),
                    params: Vec::new(),
                    database: database.to_string(),
                    page: 0,
                    page_size: 10,
                });
                (
                    ConsoleOutput::Table {
                        columns: rs.columns().to_vec(),
                        rows: rs.rows().iter().take(10).map(|r| r.cells.clone()).collect(),
                    },
                    rs.elapsed_ms,
                )
            }
        }),
        Protocol::Cypher => {
            if lower.starts_with("match") || lower.starts_with("optional match") {
                Ok((
                    table(
                        &["u.name", "p.name"],
                        &[
                            &["Hana", "Standing desk"],
                            &["Amir", "Monitor arm"],
                            &["Lena", "Desk lamp"],
                        ],
                    ),
                    21,
                ))
            } else {
                Err(StudioError::Syntax {
                    reason: "a Cypher query starts with MATCH or OPTIONAL MATCH".to_string(),
                })
            }
        }
        Protocol::Admin => match lower.as_str() {
            "show databases" => Ok((
                table(
                    &["name", "tables"],
                    &[&["analytics", "6"], &["staging", "11"], &["sandbox", "2"]],
                ),
                6,
            )),
            "show cluster status" => Ok((
                ConsoleOutput::Text(vec![
                    "cluster: staging-cluster \u{b7} nodes: 7 \u{b7} healthy: 6/7".to_string(),
                    "node-5 lagging \u{b7} 3 groups, each with its own leader".to_string(),
                ]),
                14,
            )),
            _ => Err(StudioError::Syntax {
                reason: format!(
                    "unknown admin command `{t}`: try SHOW DATABASES or SHOW CLUSTER STATUS"
                ),
            }),
        },
    }
}
