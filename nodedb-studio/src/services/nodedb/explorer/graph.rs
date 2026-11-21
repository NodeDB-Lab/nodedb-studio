//! Graph edges, read and removed through `MATCH` and `GRAPH` statements.
//!
//! A graph is an overlay on a document collection: its nodes are that
//! collection's ids, and an edge is `(from, label, to)`. The server has no
//! statement that pages edges. `MATCH (a)-[r]->(b)` lists every edge as
//! `a`, `r = "a|label|b"` and `b`, and ignores `LIMIT`. So a read lists them
//! all and pages here. Properties come from one `GRAPH TRAVERSE ... DEPTH 1`
//! per distinct source node on the page. Node and graph names are bound as
//! parameters. The server reports no node type, so `NodeRef.kind` is empty.

use std::collections::HashMap;

use nodedb_client::{NodeDb, QueryResult};
use nodedb_types::Value;

use crate::models::graph::records::{Edge, EdgePage, EdgeQuery, NodeRef};
use crate::models::records::json::parse_object;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::explorer::records::write_error;
use crate::services::nodedb::sql::ident;

/// One listed edge, before its properties are read.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::services::nodedb) struct EdgeKey {
    /// The server's own name for the edge, `from|label|to`.
    pub id: String,
    pub from: String,
    pub label: String,
    pub to: String,
}

fn malformed(reason: String) -> StudioError {
    StudioError::MalformedRows { reason }
}

fn text(value: &Value) -> Option<&str> {
    match value {
        Value::String(s) => Some(s),
        _ => None,
    }
}

/// The edges a `MATCH (a)-[r]->(b) RETURN a, r, b` result lists. The label is
/// what `r` holds between `a|` and `|b`, so an id that contains `|` still
/// splits right.
pub(in crate::services::nodedb) fn edge_keys(
    result: &QueryResult,
) -> Result<Vec<EdgeKey>, StudioError> {
    // The columns are checked before the rows: a result without them is the
    // wrong answer even when it is empty.
    let at = |name: &str| {
        result
            .columns
            .iter()
            .position(|c| c == name)
            .ok_or_else(|| StudioError::UnexpectedColumns {
                expected: "a, r, b".to_string(),
                got: result.columns.join(", "),
            })
    };
    let (a, r, b) = (at("a")?, at("r")?, at("b")?);
    result
        .rows
        .iter()
        .map(|row| {
            let cell = |i: usize| row.get(i).and_then(text);
            let (Some(from), Some(id), Some(to)) = (cell(a), cell(r), cell(b)) else {
                return Err(malformed("an edge row is not three strings".to_string()));
            };
            let label = id
                .strip_prefix(from)
                .and_then(|s| s.strip_prefix('|'))
                .and_then(|s| s.strip_suffix(to))
                .and_then(|s| s.strip_suffix('|'))
                .ok_or_else(|| malformed(format!("edge `{id}` is not `{from}|label|{to}`")))?;
            Ok(EdgeKey {
                id: id.to_string(),
                from: from.to_string(),
                label: label.to_string(),
                to: to.to_string(),
            })
        })
        .collect()
}

/// The edges `query` keeps, in id order, and every relation the graph holds.
pub(in crate::services::nodedb) fn select_edges(
    mut all: Vec<EdgeKey>,
    query: &EdgeQuery,
) -> (Vec<EdgeKey>, Vec<String>) {
    all.sort();
    let mut relations: Vec<String> = all.iter().map(|e| e.label.clone()).collect();
    relations.sort();
    relations.dedup();
    let needle = query.text.trim().to_lowercase();
    all.retain(|e| {
        query.relation.as_ref().is_none_or(|r| *r == e.label)
            && (needle.is_empty()
                || [&e.id, &e.from, &e.to]
                    .iter()
                    .any(|t| t.to_lowercase().contains(&needle)))
    });
    (all, relations)
}

/// The `properties` of each edge a `GRAPH TRAVERSE` result crossed, by id.
pub(in crate::services::nodedb) fn traversed_properties(
    result: &QueryResult,
) -> Result<HashMap<String, Value>, StudioError> {
    let json = result
        .rows
        .first()
        .and_then(|r| r.first())
        .and_then(text)
        .ok_or_else(|| malformed("GRAPH TRAVERSE returned no result cell".to_string()))?;
    let parsed = parse_object(json, &[]).map_err(|e| malformed(e.message))?;
    let Value::Object(top) = parsed else {
        return Err(malformed(
            "GRAPH TRAVERSE result is not an object".to_string(),
        ));
    };
    let Some(Value::Array(edges)) = top.get("edges") else {
        return Err(malformed(
            "GRAPH TRAVERSE result has no `edges`".to_string(),
        ));
    };
    let mut out = HashMap::new();
    for edge in edges {
        let Value::Object(e) = edge else { continue };
        let part = |k: &str| e.get(k).and_then(text);
        if let (Some(from), Some(label), Some(to)) = (part("from"), part("label"), part("to")) {
            let props = e
                .get("properties")
                .cloned()
                .unwrap_or_else(|| Value::Object(HashMap::new()));
            out.insert(format!("{from}|{label}|{to}"), props);
        }
    }
    Ok(out)
}

impl NodeDbConnectionService {
    async fn list_edges(&self, graph: &str) -> Result<Vec<EdgeKey>, StudioError> {
        let client = self.client(None)?;
        let result = client
            .execute_sql(
                "MATCH (a)-[r]->(b) IN $1 RETURN a, r, b",
                &[Value::String(ident(graph, "graph")?.to_string())],
            )
            .await?;
        edge_keys(&result)
    }

    pub(in crate::services::nodedb) async fn read_edges(
        &self,
        graph: &str,
        query: &EdgeQuery,
    ) -> Result<EdgePage, StudioError> {
        let (kept, relations) = select_edges(self.list_edges(graph).await?, query);
        let total = kept.len() as u64;
        let size = query.page_size.max(1);
        let page: Vec<EdgeKey> = kept
            .into_iter()
            .skip(size.saturating_mul(query.page))
            .take(size)
            .collect();
        let client = self.client(None)?;
        let mut props: HashMap<String, Value> = HashMap::new();
        let mut sources: Vec<&String> = page.iter().map(|e| &e.from).collect();
        sources.sort();
        sources.dedup();
        for from in sources {
            let result = client
                .execute_sql(
                    "GRAPH TRAVERSE IN $1 FROM $2 DEPTH 1 DIRECTION out",
                    &[
                        Value::String(graph.to_string()),
                        Value::String(from.clone()),
                    ],
                )
                .await?;
            props.extend(traversed_properties(&result)?);
        }
        let node = |id: &str| NodeRef {
            collection: graph.to_string(),
            id: id.to_string(),
            kind: String::new(),
        };
        let edges = page
            .iter()
            .map(|e| {
                let properties = props.get(&e.id).cloned().ok_or_else(|| {
                    malformed(format!(
                        "GRAPH TRAVERSE from `{}` did not cross `{}`",
                        e.from, e.id
                    ))
                })?;
                Ok(Edge {
                    id: e.id.clone(),
                    relation: e.label.clone(),
                    from: node(&e.from),
                    to: node(&e.to),
                    properties,
                })
            })
            .collect::<Result<Vec<_>, StudioError>>()?;
        Ok(EdgePage {
            edges,
            total,
            relations,
        })
    }

    pub(in crate::services::nodedb) async fn remove_edge(
        &self,
        graph: &str,
        id: &str,
    ) -> Result<(), StudioError> {
        self.outside_tx()?;
        let missing = || StudioError::RecordMissing {
            collection: graph.to_string(),
            id: id.to_string(),
        };
        let edge = self
            .list_edges(graph)
            .await?
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(missing)?;
        let result = self
            .client(None)?
            .execute_sql(
                "GRAPH DELETE EDGE IN $1 FROM $2 TO $3 TYPE $4",
                &[
                    Value::String(graph.to_string()),
                    Value::String(edge.from),
                    Value::String(edge.to),
                    Value::String(edge.label),
                ],
            )
            .await
            .map_err(|e| write_error(e, "DELETE", graph))?;
        if result.rows_affected == 0 {
            return Err(missing());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(rows: &[[&str; 3]]) -> QueryResult {
        QueryResult {
            columns: vec!["a".into(), "r".into(), "b".into()],
            rows: rows
                .iter()
                .map(|r| r.iter().map(|c| Value::String((*c).into())).collect())
                .collect(),
            rows_affected: 0,
            command: None,
        }
    }

    #[test]
    fn a_listing_splits_each_edge_into_from_label_and_to() {
        let keys = edge_keys(&listing(&[
            ["a", "a|KNOWS|b", "b"],
            ["x|y", "x|y|L|z", "z"],
        ]))
        .expect("splits");
        assert_eq!(
            (keys[0].from.as_str(), keys[0].label.as_str()),
            ("a", "KNOWS")
        );
        assert_eq!(
            keys[1].label, "L",
            "a pipe inside a node id still splits right"
        );
        assert!(edge_keys(&listing(&[["a", "a|KNOWS|c", "b"]])).is_err());
        assert!(edge_keys(&listing(&[])).expect("no edges").is_empty());
        assert!(
            matches!(
                edge_keys(&QueryResult::empty()),
                Err(StudioError::UnexpectedColumns { .. })
            ),
            "an empty result without the columns is the wrong answer"
        );
    }

    #[test]
    fn a_query_filters_by_relation_and_text_then_the_page_counts_what_is_left() {
        let all = edge_keys(&listing(&[
            ["a", "a|KNOWS|b", "b"],
            ["a", "a|LIKES|c", "c"],
            ["b", "b|KNOWS|c", "c"],
        ]))
        .expect("splits");
        let by_relation = EdgeQuery {
            relation: Some("KNOWS".into()),
            ..EdgeQuery::new(10)
        };
        let (kept, relations) = select_edges(all.clone(), &by_relation);
        assert_eq!(kept.len(), 2);
        assert_eq!(relations, ["KNOWS", "LIKES"]);
        let by_text = EdgeQuery {
            text: "C".into(),
            ..EdgeQuery::new(10)
        };
        assert_eq!(select_edges(all, &by_text).0.len(), 2);
    }

    #[test]
    fn traverse_properties_key_by_edge_id() {
        let json = r#"{"nodes":[],"edges":[{"from":"a","to":"b","label":"KNOWS","properties":{"since":2020}},{"from":"a","to":"c","label":"LIKES","properties":{}}]}"#;
        let result = QueryResult {
            columns: vec!["result".into()],
            rows: vec![vec![Value::String(json.into())]],
            rows_affected: 0,
            command: None,
        };
        let props = traversed_properties(&result).expect("parses");
        assert_eq!(props.len(), 2);
        assert!(matches!(&props["a|KNOWS|b"], Value::Object(m) if m.contains_key("since")));
        assert!(traversed_properties(&QueryResult::empty()).is_err());
    }

    use crate::services::explorer_data::ExplorerData;
    use crate::services::nodedb::service::live;

    async fn seed(svc: &NodeDbConnectionService, name: &str) {
        let c = svc.client(None).expect("a client");
        c.query(&format!("DROP COLLECTION IF EXISTS {name}"))
            .await
            .expect("drops");
        c.query(&format!("CREATE COLLECTION {name}"))
            .await
            .expect("creates");
        for sql in [
            format!("INSERT INTO {name} (id, name) VALUES ('a', 'A')"),
            format!("INSERT INTO {name} (id, name) VALUES ('b', 'B')"),
            format!("INSERT INTO {name} (id, name) VALUES ('c', 'C')"),
            format!(
                "GRAPH INSERT EDGE IN '{name}' FROM 'a' TO 'b' TYPE 'KNOWS' PROPERTIES '{{\"since\": 2020}}'"
            ),
            format!("GRAPH INSERT EDGE IN '{name}' FROM 'a' TO 'c' TYPE 'LIKES'"),
            format!("GRAPH INSERT EDGE IN '{name}' FROM 'b' TO 'c' TYPE 'KNOWS'"),
        ] {
            c.query(&sql).await.expect("seeds");
        }
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_edges_page_filter_and_delete() {
        let svc = live().await;
        let name = "studio_probe_x_graph";
        seed(&svc, name).await;
        let all = svc.edges(name, &EdgeQuery::new(2)).await.expect("page 0");
        assert_eq!((all.total, all.edges.len()), (3, 2));
        assert_eq!(all.relations, ["KNOWS", "LIKES"]);
        let first = &all.edges[0];
        assert_eq!(first.id, "a|KNOWS|b");
        assert_eq!((first.from.id.as_str(), first.to.id.as_str()), ("a", "b"));
        assert_eq!(first.from.collection, name);
        assert!(
            matches!(&first.properties, Value::Object(m) if m.get("since") == Some(&Value::Integer(2020)))
        );
        let second = svc
            .edges(
                name,
                &EdgeQuery {
                    page: 1,
                    ..EdgeQuery::new(2)
                },
            )
            .await
            .expect("page 1");
        assert_eq!(second.edges.len(), 1);

        let knows = svc
            .edges(
                name,
                &EdgeQuery {
                    relation: Some("KNOWS".into()),
                    ..EdgeQuery::new(10)
                },
            )
            .await
            .expect("filtered");
        assert_eq!(knows.total, 2);
        let text = svc
            .edges(
                name,
                &EdgeQuery {
                    text: "LIKES".into(),
                    ..EdgeQuery::new(10)
                },
            )
            .await
            .expect("text");
        assert_eq!(text.total, 1);

        svc.delete_edge(name, "a|LIKES|c").await.expect("delete");
        assert_eq!(
            svc.edges(name, &EdgeQuery::new(10))
                .await
                .expect("after")
                .total,
            2
        );
        assert!(matches!(
            svc.delete_edge(name, "a|LIKES|c").await,
            Err(StudioError::RecordMissing { .. })
        ));
        // A graph with no edges still answers with its columns.
        for id in ["a|KNOWS|b", "b|KNOWS|c"] {
            svc.delete_edge(name, id).await.expect("delete");
        }
        let empty = svc
            .edges(name, &EdgeQuery::new(10))
            .await
            .expect("no edges");
        assert_eq!((empty.total, empty.edges.len()), (0, 0));
        assert!(matches!(
            svc.edges("studio_probe_x_no_such_graph", &EdgeQuery::new(10))
                .await,
            Err(StudioError::NotFound(_))
        ));
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_arrays_have_no_source() {
        use crate::models::array_data::SliceRequest;
        let svc = live().await;
        let err = svc
            .array_schema("studio_probe_arr")
            .await
            .expect_err("no schema");
        assert!(err.to_string().contains("studio_probe_arr"), "{err}");
        let request = SliceRequest {
            time_index: 0,
            lat: (0.0, 1.0),
            lon: (0.0, 1.0),
            attribute: "a".into(),
        };
        assert!(svc.array_slice("studio_probe_arr", &request).await.is_err());
    }
}
