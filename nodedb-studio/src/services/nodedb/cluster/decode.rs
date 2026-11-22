//! Rows of the cluster `SHOW` statements, read by column name. The native
//! protocol sends every integer as text, so each one is parsed strictly: a
//! value that does not parse is a typed error naming the column, never 0.

use std::collections::HashMap;
use std::str::FromStr;

use crate::models::cluster::monitor::EngineBudget;
use crate::models::cluster::{Circuit, GroupRole, Link, NodeState};
use crate::services::decode::{Row, Table, decode_rows};
use crate::services::error::StudioError;

fn malformed(reason: String) -> StudioError {
    StudioError::MalformedRows { reason }
}

/// `text` as a `T`, or an error naming `column` and the text.
fn number<T: FromStr>(text: &str, column: &str) -> Result<T, StudioError> {
    text.trim()
        .parse()
        .map_err(|_| malformed(format!("{column} is not a number: `{text}`")))
}

/// The rows as a map by key. A key the server sends twice is an error
/// naming it: a map would keep one row and drop the other.
fn unique<K, V>(rows: Vec<(K, V)>, what: &str) -> Result<HashMap<K, V>, StudioError>
where
    K: std::hash::Hash + Eq + std::fmt::Display,
{
    let mut map = HashMap::with_capacity(rows.len());
    for (k, v) in rows {
        if map.contains_key(&k) {
            return Err(malformed(format!("{what} lists `{k}` twice")));
        }
        map.insert(k, v);
    }
    Ok(map)
}

fn int<T: FromStr>(row: &Row<'_>, column: &str) -> Result<T, StudioError> {
    number(row.field(column)?, column)
}

/// A NodeDB group id. `SHOW RAFT GROUPS` prints the `u64` cast to `i64`,
/// so the sequencer group `0xFFFF_FFFF_0000_0001` reads `-4294967295`.
/// `SHOW RAFT GROUP` prints the `u64` itself. Both give the same id.
pub(in crate::services::nodedb) fn group_id(text: &str) -> Result<u64, StudioError> {
    let t = text.trim();
    t.parse::<i64>()
        .map(|i| i as u64)
        .or_else(|_| t.parse::<u64>())
        .map_err(|_| malformed(format!("group_id is not a number: `{text}`")))
}

/// `1, 3, 2`: node ids in membership order. Blank is no node.
fn id_list(text: &str, column: &str) -> Result<Vec<u64>, StudioError> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| number(s, column))
        .collect()
}

/// A `property` / `value` result, by property.
pub(in crate::services::nodedb) fn properties(
    table: &Table,
) -> Result<HashMap<String, String>, StudioError> {
    let rows = decode_rows(table, &["property", "value"], |r| {
        Ok((
            r.field("property")?.to_string(),
            r.field("value")?.to_string(),
        ))
    })?;
    unique(rows, "the property list")
}

fn property<'a>(
    props: &'a HashMap<String, String>,
    name: &str,
    statement: &str,
) -> Result<&'a str, StudioError> {
    props
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| malformed(format!("{statement} has no `{name}` row")))
}

/// What `SHOW CLUSTER` says about the node that answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::services::nodedb) struct Seen {
    /// The node that answered: the observer.
    pub node_id: u64,
    /// The groups it hosts: the ones it leads plus the ones it follows.
    pub hosted: u32,
}

pub(in crate::services::nodedb) fn seen(table: &Table) -> Result<Seen, StudioError> {
    let props = properties(table)?;
    let get = |name| property(&props, name, "SHOW CLUSTER");
    let leading: u32 = number(get("groups_leading")?, "groups_leading")?;
    let following: u32 = number(get("groups_following")?, "groups_following")?;
    Ok(Seen {
        node_id: number(get("node_id")?, "node_id")?,
        hosted: leading + following,
    })
}

/// One `SHOW NODES` row. Its `raft_groups` column is always blank on
/// 0.6.2, so Studio does not read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::services::nodedb) struct NodeRow {
    pub node_id: u64,
    pub address: String,
    pub state: NodeState,
}

pub(in crate::services::nodedb) fn nodes(table: &Table) -> Result<Vec<NodeRow>, StudioError> {
    decode_rows(table, &["node_id", "address", "state"], |r| {
        Ok(NodeRow {
            node_id: int(&r, "node_id")?,
            address: r.field("address")?.to_string(),
            state: NodeState::parse(r.field("state")?),
        })
    })
}

/// `SHOW PEER HEALTH`: the observer's circuit to each peer. The observer
/// itself is never listed.
pub(in crate::services::nodedb) fn links(table: &Table) -> Result<HashMap<u64, Link>, StudioError> {
    let rows = decode_rows(table, &["node_id", "circuit", "failures"], |r| {
        Ok((
            int::<u64>(&r, "node_id")?,
            Link::Circuit {
                state: Circuit::parse(r.field("circuit")?),
                failures: int(&r, "failures")?,
            },
        ))
    })?;
    unique(rows, "SHOW PEER HEALTH")
}

/// One `SHOW RAFT GROUPS` row: a group the observer hosts, as it holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::services::nodedb) struct GroupRow {
    pub group_id: u64,
    pub role: GroupRole,
    /// `0` when the observer knows no leader, as during an election.
    pub leader_id: u64,
    pub term: u64,
    pub commit_index: u64,
    pub last_applied: u64,
    /// Voters, as the observer's Raft counts them.
    pub members: u32,
}

pub(in crate::services::nodedb) fn groups(table: &Table) -> Result<Vec<GroupRow>, StudioError> {
    let cols = [
        "group_id",
        "role",
        "leader_id",
        "term",
        "commit_index",
        "last_applied",
        "members",
    ];
    decode_rows(table, &cols, |r| {
        Ok(GroupRow {
            group_id: group_id(r.field("group_id")?)?,
            role: GroupRole::parse(r.field("role")?),
            leader_id: int(&r, "leader_id")?,
            term: int(&r, "term")?,
            commit_index: int(&r, "commit_index")?,
            last_applied: int(&r, "last_applied")?,
            members: int(&r, "members")?,
        })
    })
}

/// The voters `SHOW RAFT GROUP <id>` lists from the observer's routing
/// table. `None`: the table has no entry for the group, as for the
/// sequencer.
pub(in crate::services::nodedb) fn group_members(
    table: &Table,
) -> Result<Option<Vec<u64>>, StudioError> {
    properties(table)?
        .get("members")
        .map(|m| id_list(m, "members"))
        .transpose()
}

/// One `SHOW RANGES` row. Its `qps`, `p99_latency_ms` and `requests_total`
/// columns are not read: see `placement` in `base.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::services::nodedb) struct RangeRow {
    pub vshard: u32,
    pub group_id: u64,
    /// The group's leader in the observer's routing table, `0` for none.
    pub leaseholder: u64,
    pub replicas: Vec<u64>,
}

pub(in crate::services::nodedb) fn ranges(table: &Table) -> Result<Vec<RangeRow>, StudioError> {
    let cols = ["vshard_id", "group_id", "leaseholder", "replicas"];
    decode_rows(table, &cols, |r| {
        Ok(RangeRow {
            vshard: int(&r, "vshard_id")?,
            group_id: group_id(r.field("group_id")?)?,
            leaseholder: int(&r, "leaseholder")?,
            replicas: id_list(r.field("replicas")?, "replicas")?,
        })
    })
}

/// The vShards `SHOW MIGRATIONS` lists as moving, with their phase.
pub(in crate::services::nodedb) fn migrations(
    table: &Table,
) -> Result<HashMap<u32, String>, StudioError> {
    let rows = decode_rows(table, &["vshard_id", "phase", "active"], |r| {
        let active = match r.field("active")? {
            "yes" => true,
            "no" => false,
            other => return Err(malformed(format!("active is not yes or no: `{other}`"))),
        };
        Ok((int(&r, "vshard_id")?, r.field("phase")?.to_string(), active))
    })?;
    let active = rows
        .into_iter()
        .filter(|(_, _, active)| *active)
        .map(|(v, phase, _)| (v, phase))
        .collect::<Vec<(u32, String)>>();
    unique(active, "SHOW MIGRATIONS")
}

/// `SHOW MEMORY`: the observer's engine budgets. `utilization_percent` is
/// the reserved share of the limit, so Studio does not read it.
pub(in crate::services::nodedb) fn budgets(
    table: &Table,
) -> Result<Vec<EngineBudget>, StudioError> {
    let cols = [
        "engine",
        "allocated_bytes",
        "limit_bytes",
        "peak_bytes",
        "rejections",
    ];
    decode_rows(table, &cols, |r| {
        Ok(EngineBudget {
            engine: r.field("engine")?.to_string(),
            limit_bytes: int(&r, "limit_bytes")?,
            reserved_bytes: int(&r, "allocated_bytes")?,
            peak_bytes: int(&r, "peak_bytes")?,
            rejections: int(&r, "rejections")?,
        })
    })
}

#[cfg(test)]
pub(in crate::services::nodedb) fn table(cols: &[&str], rows: &[&[&str]]) -> Table {
    Table {
        columns: cols.iter().map(|s| s.to_string()).collect(),
        rows: rows
            .iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cluster::{GroupKind, SEQUENCER_GROUP_ID};

    /// Live `SHOW CLUSTER` on node 1 of the 3-node cluster.
    fn cluster_props() -> Table {
        table(
            &["property", "value"],
            &[
                &["node_id", "1"],
                &["nodes_total", "3"],
                &["nodes_active", "3"],
                &["topology_version", "3"],
                &["raft_groups", "5"],
                &["vshards", "1024"],
                &["groups_leading", "4"],
                &["groups_following", "2"],
            ],
        )
    }

    #[test]
    fn show_cluster_names_the_observer_and_its_groups() {
        assert_eq!(
            seen(&cluster_props()).expect("reads"),
            Seen {
                node_id: 1,
                hosted: 6
            }
        );
    }

    #[test]
    fn a_missing_or_bad_property_is_an_error_not_a_zero() {
        let t = table(&["property", "value"], &[&["node_id", "1"]]);
        assert!(matches!(seen(&t), Err(StudioError::MalformedRows { .. })));
        let t = table(
            &["property", "value"],
            &[
                &["node_id", "one"],
                &["groups_leading", "4"],
                &["groups_following", "2"],
            ],
        );
        let err = seen(&t).expect_err("not a number");
        assert!(err.to_string().contains("node_id"), "{err}");
        // The session-variable fallback some SHOW statements answer with.
        let fallback = table(&["setting"], &[&[""]]);
        assert!(matches!(
            seen(&fallback),
            Err(StudioError::UnexpectedColumns { .. })
        ));
    }

    #[test]
    fn the_sequencer_id_reads_the_same_both_ways() {
        assert_eq!(group_id("-4294967295").expect("i64"), SEQUENCER_GROUP_ID);
        assert_eq!(
            group_id("18446744069414584321").expect("u64"),
            SEQUENCER_GROUP_ID
        );
        assert_eq!(
            GroupKind::of_id(group_id("0").expect("id")),
            GroupKind::Metadata
        );
        assert_eq!(group_id("2").expect("id"), 2);
        assert!(group_id("two").is_err());
    }

    #[test]
    fn raft_rows_parse_every_integer_strictly() {
        let cols = [
            "group_id",
            "role",
            "leader_id",
            "term",
            "commit_index",
            "last_applied",
            "members",
            "vshards",
        ];
        let t = table(
            &cols,
            &[
                &["1", "Follower", "3", "2", "12", "12", "3", "256"],
                &["-4294967295", "Leader", "1", "1", "8", "8", "3", "0"],
            ],
        );
        let rows = groups(&t).expect("reads");
        assert_eq!(rows[0].role, GroupRole::Follower);
        assert_eq!(rows[0].leader_id, 3);
        assert_eq!(rows[1].group_id, SEQUENCER_GROUP_ID);
        let bad = table(&cols, &[&["1", "Leader", "1", "2", "", "12", "3", "256"]]);
        let err = groups(&bad).expect_err("blank index");
        assert!(err.to_string().contains("commit_index"), "{err}");
    }

    #[test]
    fn nodes_and_links_keep_the_servers_words() {
        let n = table(
            &["node_id", "address", "state", "raft_groups"],
            &[
                &["1", "127.0.0.1:26400", "active", ""],
                &["3", "127.0.0.1:28400", "draining", ""],
            ],
        );
        let rows = nodes(&n).expect("reads");
        assert_eq!(rows[1].state, NodeState::Draining);
        let p = table(
            &["node_id", "address", "node_state", "circuit", "failures"],
            &[&["3", "127.0.0.1:28400", "draining", "OPEN", "5"]],
        );
        assert_eq!(
            links(&p).expect("reads").get(&3),
            Some(&Link::Circuit {
                state: Circuit::Open,
                failures: 5
            })
        );
    }

    #[test]
    fn group_members_are_absent_for_the_sequencer() {
        let data = table(
            &["property", "value"],
            &[&["group_id", "2"], &["members", "1, 3, 2"]],
        );
        assert_eq!(group_members(&data).expect("reads"), Some(vec![1, 3, 2]));
        let seq = table(
            &["property", "value"],
            &[&["group_id", "18446744069414584321"], &["role", "Leader"]],
        );
        assert_eq!(group_members(&seq).expect("reads"), None);
        let bad = table(&["property", "value"], &[&["members", "1, x"]]);
        assert!(group_members(&bad).is_err());
    }

    #[test]
    fn ranges_read_the_routing_and_migrations_keep_the_active_ones() {
        let r = table(
            &[
                "vshard_id",
                "group_id",
                "leaseholder",
                "replicas",
                "qps",
                "p99_latency_ms",
                "requests_total",
            ],
            &[&["0", "1", "3", "1, 3, 2", "0", "0", "0"]],
        );
        let rows = ranges(&r).expect("reads");
        assert_eq!(rows[0].replicas, vec![1, 3, 2]);
        assert_eq!(rows[0].leaseholder, 3);
        let m = table(
            &["vshard_id", "phase", "elapsed_ms", "active"],
            &[&["7", "copy", "120", "yes"], &["8", "done", "900", "no"]],
        );
        let moving = migrations(&m).expect("reads");
        assert_eq!(moving.len(), 1);
        assert_eq!(moving.get(&7).map(String::as_str), Some("copy"));
        let bad = table(
            &["vshard_id", "phase", "elapsed_ms", "active"],
            &[&["7", "copy", "120", "maybe"]],
        );
        assert!(migrations(&bad).is_err());
    }

    /// A key the server sends twice is a malformed result, never a row
    /// dropped without a word.
    #[test]
    fn a_duplicate_key_row_is_malformed() {
        let props = table(
            &["property", "value"],
            &[&["node_id", "1"], &["node_id", "2"]],
        );
        assert!(matches!(
            properties(&props),
            Err(StudioError::MalformedRows { .. })
        ));
        let peers = table(
            &["node_id", "address", "node_state", "circuit", "failures"],
            &[
                &["2", "a", "active", "closed", "0"],
                &["2", "a", "active", "OPEN", "5"],
            ],
        );
        assert!(matches!(
            links(&peers),
            Err(StudioError::MalformedRows { .. })
        ));
        let moving = table(
            &["vshard_id", "phase", "elapsed_ms", "active"],
            &[&["7", "copy", "1", "yes"], &["7", "cutover", "2", "yes"]],
        );
        assert!(matches!(
            migrations(&moving),
            Err(StudioError::MalformedRows { .. })
        ));
    }

    #[test]
    fn memory_budgets_read_by_name() {
        let t = table(
            &[
                "engine",
                "allocated_bytes",
                "limit_bytes",
                "peak_bytes",
                "rejections",
                "utilization_percent",
            ],
            &[&["Vector", "0", "134217728", "0", "0", "0"]],
        );
        let b = budgets(&t).expect("reads");
        assert_eq!(b[0].engine, "Vector");
        assert_eq!(b[0].limit_bytes, 134_217_728);
        assert_eq!(b[0].reserved_bytes, 0);
    }
}
