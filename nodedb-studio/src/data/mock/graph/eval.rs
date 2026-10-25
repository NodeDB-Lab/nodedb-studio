//! A small Cypher evaluator over a stored graph: the pattern, property
//! filters, and limit the parser accepts. It answers like a server would,
//! so the Builder's preview and the editor's text both run for real.
//!
//! The result is the subgraph the matches walk: every node and edge on a
//! matching path, not only the variables `RETURN` names. The canvas needs
//! the edges that join what it draws.

use nodedb_types::Value;

use crate::data::mock::graph::store::StoredGraph;
use crate::models::cypher::{Cmp, Condition, Hop, Literal, NodePattern, Pattern, Way};
use crate::models::graph::{GraphNode, GraphResult, NodeKind, Relation};
use crate::services::error::StudioError;

/// Rows a query returns when it names no `LIMIT`.
const DEFAULT_ROWS: usize = 1000;

fn syntax(reason: String) -> StudioError {
    StudioError::Syntax { reason }
}

fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Integer(i) => Some(i.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn number_of(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn field<'a>(node: &'a GraphNode, name: &str, id: &'a Value) -> Option<&'a Value> {
    if name == "id" || name == "_id" {
        Some(id)
    } else {
        node.property(name)
    }
}

fn compare(value: &Value, op: Cmp, lit: &Literal) -> bool {
    let order = match lit {
        Literal::Num(n) => number_of(value).and_then(|v| v.partial_cmp(n)),
        Literal::Str(t) => text_of(value).map(|v| v.as_str().cmp(t.as_str())),
        Literal::Bool(b) => match value {
            Value::Bool(v) => Some(v.cmp(b)),
            _ => None,
        },
    };
    use std::cmp::Ordering::{Equal, Greater, Less};
    match (order, op) {
        (None, Cmp::Ne) => true,
        (None, _) => false,
        (Some(o), Cmp::Eq) => o == Equal,
        (Some(o), Cmp::Ne) => o != Equal,
        (Some(o), Cmp::Gt) => o == Greater,
        (Some(o), Cmp::Ge) => o != Less,
        (Some(o), Cmp::Lt) => o == Less,
        (Some(o), Cmp::Le) => o != Greater,
    }
}

struct Resolved {
    kinds: Vec<Option<NodeKind>>,
    relations: Vec<Option<Relation>>,
}

/// Check every label and type against the binding, before any matching.
fn resolve(graph: &StoredGraph, p: &Pattern) -> Result<Resolved, StudioError> {
    let name = &graph.binding.name;
    let mut kinds = Vec::new();
    for n in &p.nodes {
        kinds.push(match &n.label {
            None => None,
            Some(l) => {
                let kind = NodeKind::from_label(l)
                    .filter(|k| graph.binding.kinds.contains(k))
                    .ok_or_else(|| {
                        let known: Vec<&str> =
                            graph.binding.kinds.iter().map(|k| k.label()).collect();
                        syntax(format!(
                            "{name} has no `{l}` nodes. It holds {}.",
                            known.join(", ")
                        ))
                    })?;
                Some(kind)
            }
        });
    }
    let mut relations = Vec::new();
    for h in &p.hops {
        relations.push(match &h.relation {
            None => None,
            Some(r) => {
                let rel = Relation::from_label(r)
                    .filter(|x| graph.binding.relations.contains(x))
                    .ok_or_else(|| {
                        let known: Vec<&str> =
                            graph.binding.relations.iter().map(|r| r.label()).collect();
                        syntax(format!(
                            "{name} has no `{r}` relationships. It holds {}.",
                            known.join(", ")
                        ))
                    })?;
                Some(rel)
            }
        });
    }
    Ok(Resolved { kinds, relations })
}

struct Walk<'a> {
    graph: &'a StoredGraph,
    p: &'a Pattern,
    r: &'a Resolved,
    wanted: usize,
    /// Matches found, as the nodes and edges (indexes) each walked.
    rows: Vec<(Vec<usize>, Vec<usize>)>,
}

impl Walk<'_> {
    fn node_ok(&self, i: usize, node: &GraphNode) -> bool {
        let pat: &NodePattern = &self.p.nodes[i];
        if self.r.kinds[i].is_some_and(|k| k != node.kind) {
            return false;
        }
        let id = Value::String(node.id().to_string());
        let props = pat
            .props
            .iter()
            .all(|(k, lit)| field(node, k, &id).is_some_and(|v| compare(v, Cmp::Eq, lit)));
        let conds = self
            .p
            .conditions
            .iter()
            .filter(|c: &&Condition| c.var == pat.var)
            .all(|c| field(node, &c.field, &id).is_some_and(|v| compare(v, c.op, &c.value)));
        props && conds
    }

    /// Edges leaving `at` along `hop`, with the node each reaches.
    fn steps(&self, hop: &Hop, rel: Option<Relation>, at: usize) -> Vec<(usize, usize)> {
        let here = &self.graph.nodes[at].path;
        self.graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| rel.is_none_or(|r| r == e.relation))
            .filter_map(|(ei, e)| {
                let (near, far) = match hop.way {
                    Way::Out => (&e.from, &e.to),
                    Way::In => (&e.to, &e.from),
                };
                (near == here).then(|| {
                    self.graph
                        .nodes
                        .iter()
                        .position(|n| &n.path == far)
                        .map(|ni| (ei, ni))
                })?
            })
            .collect()
    }

    fn done(&self) -> bool {
        self.rows.len() > self.wanted
    }

    /// Bind pattern node `i` to `at`, then follow hop `i` to the next.
    fn bind(&mut self, i: usize, at: usize, nodes: &mut Vec<usize>, edges: &mut Vec<usize>) {
        if self.done() || !self.node_ok(i, &self.graph.nodes[at]) {
            return;
        }
        nodes.push(at);
        if i + 1 == self.p.nodes.len() {
            self.rows.push((nodes.clone(), edges.clone()));
        } else {
            let hop = self.p.hops[i].clone();
            let rel = self.r.relations[i];
            self.go(&hop, rel, i, at, 0, nodes, edges);
        }
        nodes.pop();
    }

    /// Walk `hop` from `at`, `taken` edges in. Stop and bind pattern node
    /// `i + 1` wherever the walk is long enough, and keep walking while it
    /// may grow. Nodes do not repeat inside one match.
    #[allow(clippy::too_many_arguments)]
    fn go(
        &mut self,
        hop: &Hop,
        rel: Option<Relation>,
        i: usize,
        at: usize,
        taken: usize,
        nodes: &mut Vec<usize>,
        edges: &mut Vec<usize>,
    ) {
        for (ei, ni) in self.steps(hop, rel, at) {
            if self.done() {
                return;
            }
            if nodes.contains(&ni) {
                continue;
            }
            edges.push(ei);
            let walked = taken + 1;
            if walked >= hop.min {
                self.bind(i + 1, ni, nodes, edges);
            }
            if walked < hop.max {
                // An intermediate node belongs to the walk.
                nodes.push(ni);
                self.go(hop, rel, i, ni, walked, nodes, edges);
                nodes.pop();
            }
            edges.pop();
        }
    }
}

/// Run `p` over `graph`, returning at most `cap` matches.
pub fn run(graph: &StoredGraph, p: &Pattern, cap: usize) -> Result<GraphResult, StudioError> {
    let resolved = resolve(graph, p)?;
    let wanted = p.limit.unwrap_or(DEFAULT_ROWS).min(cap).max(1);
    let mut walk = Walk {
        graph,
        p,
        r: &resolved,
        wanted,
        rows: Vec::new(),
    };
    for start in 0..graph.nodes.len() {
        walk.bind(0, start, &mut Vec::new(), &mut Vec::new());
    }
    let truncated = walk.rows.len() > wanted;
    let mut out = GraphResult {
        truncated,
        ..GraphResult::default()
    };
    for (nodes, edges) in walk.rows.iter().take(wanted) {
        for &n in nodes {
            let node = &graph.nodes[n];
            if out.node(&node.path).is_none() {
                out.nodes.push(node.clone());
            }
        }
        for &e in edges {
            let edge = &graph.edges[e];
            if out.edge(&edge.id).is_none() {
                out.edges.push(edge.clone());
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::graph::{commerce, social};
    use crate::models::cypher::parse;

    const DESIGN: &str = "MATCH (u:User)-[:POSTED]->(p:Post)-[:TAGGED]->(t:Topic {name: \"databases\"})\nWHERE u.country = \"MY\"\nRETURN u, p, t\nLIMIT 50";

    fn query(g: &StoredGraph, text: &str, cap: usize) -> Result<GraphResult, StudioError> {
        run(g, &parse(text).expect("parses"), cap)
    }

    #[test]
    fn the_design_query_returns_eight_nodes_and_eight_edges() {
        let r = query(&social::graph(), DESIGN, 50).expect("runs");
        assert_eq!((r.nodes.len(), r.edges.len()), (8, 8));
        let kinds = |k: NodeKind| r.nodes.iter().filter(|n| n.kind == k).count();
        assert_eq!(
            (
                kinds(NodeKind::User),
                kinds(NodeKind::Post),
                kinds(NodeKind::Topic)
            ),
            (3, 4, 1)
        );
        // The filter drops the one user outside MY.
        assert!(r.node("users/u77654").is_none());
        assert_eq!(r.nodes[0].path, "users/u44182");
    }

    #[test]
    fn the_builder_default_returns_four_users_and_four_orders() {
        let text = "MATCH (u:Users)-[:PURCHASED]->(o:Orders)\nWHERE u.status = 'active' AND o.total > 100\nRETURN u, o\nLIMIT 100";
        let r = query(&commerce::graph(), text, 100).expect("runs");
        assert_eq!((r.nodes.len(), r.edges.len()), (8, 4));
        assert!(r.edges.iter().all(|e| e.relation == Relation::Purchased));
        assert!(r.node("orders/o103").is_none(), "89.50 is under the filter");
    }

    #[test]
    fn limit_and_cap_cut_the_matches_and_say_so() {
        let g = social::graph();
        let r = query(
            &g,
            "MATCH (p:Post)-[:TAGGED]->(t:Topic) RETURN p, t LIMIT 2",
            50,
        )
        .expect("runs");
        assert_eq!(r.edges.len(), 2);
        assert!(r.truncated);
        let capped = query(&g, "MATCH (p:Post)-[:TAGGED]->(t:Topic) RETURN p, t", 3).expect("runs");
        assert_eq!(capped.edges.len(), 3);
        assert!(capped.truncated);
        let all = query(&g, "MATCH (p:Post)-[:TAGGED]->(t:Topic) RETURN p, t", 50).expect("runs");
        assert!(!all.truncated);
    }

    #[test]
    fn inbound_and_long_hops_walk_the_same_edges() {
        let g = commerce::graph();
        let back = query(
            &g,
            "MATCH (o:Order)<-[:PURCHASED]-(u:User {name: 'Hana Idris'}) RETURN o, u",
            50,
        )
        .expect("runs");
        assert_eq!(back.edges.len(), 6);
        let two = query(
            &g,
            "MATCH (u:User {name: 'Hana Idris'})-[*1..2]->(p:Product) RETURN u, p",
            50,
        )
        .expect("runs");
        // u44182 -> o104 -> p12 and -> p77, with the order on both walks.
        assert_eq!(two.edges.len(), 3);
        assert!(two.node("orders/o104").is_some());
        assert!(two.node("products/p12").is_some());
    }

    #[test]
    fn an_unknown_label_or_type_names_what_the_graph_holds() {
        let g = social::graph();
        let err = query(&g, "MATCH (p:Product) RETURN p", 50).expect_err("unknown label");
        assert!(
            err.to_string()
                .contains("social_graph has no `Product` nodes"),
            "{err}"
        );
        let err = query(&g, "MATCH (a)-[:CONTAINS]->(b) RETURN a", 50).expect_err("unknown type");
        assert!(
            err.to_string().contains("no `CONTAINS` relationships"),
            "{err}"
        );
    }

    #[test]
    fn comparisons_read_numbers_text_and_ids() {
        let g = commerce::graph();
        let big = query(&g, "MATCH (o:Order) WHERE o.total >= 1250 RETURN o", 50).expect("runs");
        assert_eq!(big.nodes.len(), 1);
        let by_id = query(&g, "MATCH (o:Order) WHERE o.id = 'o104' RETURN o", 50).expect("runs");
        assert_eq!(by_id.nodes[0].path, "orders/o104");
        let not = query(
            &g,
            "MATCH (o:Order) WHERE o.status <> 'shipped' RETURN o",
            50,
        )
        .expect("runs");
        assert_eq!(not.nodes.len(), 3);
    }
}
