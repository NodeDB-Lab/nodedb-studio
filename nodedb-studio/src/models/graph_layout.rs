//! Node placement for the graph canvas. Plain Rust, deterministic: the same
//! graph always gets the same positions, so a reload or a screenshot never
//! moves a node.
//!
//! `Layered` puts each node one column right of its sources and orders every
//! column to cut crossings. `Force` starts from that arrangement, then lets
//! nodes repel and edges pull until the picture balances. Both keep edges
//! reading left to right. Neither depends on the workspace's panels.

use crate::models::graph::GraphResult;

/// How the canvas arranges nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutKind {
    #[default]
    Force,
    Layered,
}

impl LayoutKind {
    pub const ALL: [LayoutKind; 2] = [LayoutKind::Force, LayoutKind::Layered];

    pub fn label(self) -> &'static str {
        match self {
            LayoutKind::Force => "Force",
            LayoutKind::Layered => "Layered",
        }
    }
}

/// A node's centre in canvas units.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub path: String,
    pub x: f32,
    pub y: f32,
}

/// Distance between columns.
const COL_W: f32 = 180.0;
/// Distance between nodes in one column.
const ROW_H: f32 = 108.0;
/// No two centres end closer than this.
pub const MIN_GAP: f32 = 96.0;

/// Edges as index pairs, without dangling references or self loops.
fn pairs(graph: &GraphResult) -> Vec<(usize, usize)> {
    let at = |p: &str| graph.nodes.iter().position(|n| n.path == p);
    graph
        .edges
        .iter()
        .filter_map(|e| Some((at(&e.from)?, at(&e.to)?)))
        .filter(|(a, b)| a != b)
        .collect()
}

/// The edges that stay once cycles are cut: a depth-first walk drops every
/// edge that returns to a node still being visited.
fn acyclic(n: usize, edges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Open,
        Done,
    }
    fn visit(
        v: usize,
        edges: &[(usize, usize)],
        mark: &mut [Mark],
        keep: &mut Vec<(usize, usize)>,
    ) {
        mark[v] = Mark::Open;
        for &(a, b) in edges.iter().filter(|(a, _)| *a == v) {
            match mark[b] {
                Mark::Open => {}
                Mark::Done => keep.push((a, b)),
                Mark::New => {
                    keep.push((a, b));
                    visit(b, edges, mark, keep);
                }
            }
        }
        mark[v] = Mark::Done;
    }
    let mut mark = vec![Mark::New; n];
    let mut keep = Vec::new();
    let has_in = |v: usize| edges.iter().any(|&(_, b)| b == v);
    for v in (0..n).filter(|v| !has_in(*v)).chain(0..n) {
        if mark[v] == Mark::New {
            visit(v, edges, &mut mark, &mut keep);
        }
    }
    keep
}

/// The column of every node: one past its deepest source.
fn columns(n: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let mut col = vec![0usize; n];
    // `edges` is acyclic, so n passes settle every column.
    for _ in 0..n {
        let mut moved = false;
        for &(a, b) in edges {
            if col[b] < col[a] + 1 {
                col[b] = col[a] + 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    col
}

/// Reorder each column by the mean slot of its neighbours in the column
/// before (then after) it. A node with no neighbour there keeps its slot.
fn order(layers: &mut [Vec<usize>], edges: &[(usize, usize)]) {
    let slot_of = |layers: &[Vec<usize>], v: usize| {
        layers
            .iter()
            .flat_map(|l| l.iter().enumerate())
            .find(|(_, x)| **x == v)
            .map(|(i, _)| i as f32)
    };
    for _ in 0..4 {
        for forward in [true, false] {
            let range: Vec<usize> = if forward {
                (1..layers.len()).collect()
            } else {
                (0..layers.len().saturating_sub(1)).rev().collect()
            };
            for l in range {
                let other = if forward { l - 1 } else { l + 1 };
                let mut keyed: Vec<(f32, usize, usize)> = layers[l]
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| {
                        let near: Vec<f32> = edges
                            .iter()
                            .filter_map(|&(a, b)| {
                                let w = if a == v {
                                    b
                                } else if b == v {
                                    a
                                } else {
                                    return None;
                                };
                                layers[other].contains(&w).then(|| slot_of(layers, w))?
                            })
                            .collect();
                        let bary = if near.is_empty() {
                            i as f32
                        } else {
                            near.iter().sum::<f32>() / near.len() as f32
                        };
                        (bary, i, v)
                    })
                    .collect();
                keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                layers[l] = keyed.into_iter().map(|(_, _, v)| v).collect();
            }
        }
    }
}

fn layered_xy(graph: &GraphResult) -> Vec<(f32, f32)> {
    let n = graph.nodes.len();
    let edges = acyclic(n, &pairs(graph));
    let col = columns(n, &edges);
    let depth = col.iter().copied().max().map_or(0, |m| m + 1);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); depth];
    for v in 0..n {
        layers[col[v]].push(v);
    }
    order(&mut layers, &edges);
    let mut xy = vec![(0.0, 0.0); n];
    for (c, layer) in layers.iter().enumerate() {
        let mid = (layer.len() as f32 - 1.0) / 2.0;
        for (i, &v) in layer.iter().enumerate() {
            xy[v] = (c as f32 * COL_W, (i as f32 - mid) * ROW_H);
        }
    }
    xy
}

/// A stable offset in -1..1 from a node's path, to break exact ties.
fn jitter(path: &str, salt: u32) -> f32 {
    let mut h: u32 = 2_166_136_261 ^ salt;
    for b in path.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(16_777_619);
    }
    (h % 2000) as f32 / 1000.0 - 1.0
}

fn relax(graph: &GraphResult, xy: &mut [(f32, f32)]) {
    const ITERATIONS: usize = 160;
    const IDEAL: f32 = 105.0;
    let n = xy.len();
    let edges = pairs(graph);
    let anchor: Vec<f32> = xy.iter().map(|p| p.0).collect();
    for (i, p) in xy.iter_mut().enumerate() {
        p.1 += jitter(&graph.nodes[i].path, 7) * 6.0;
    }
    for it in 0..ITERATIONS {
        let heat = 30.0 * (1.0 - it as f32 / ITERATIONS as f32) + 1.0;
        let mut force = vec![(0.0f32, 0.0f32); n];
        for i in 0..n {
            for j in (i + 1)..n {
                let (dx, dy) = (xy[i].0 - xy[j].0, xy[i].1 - xy[j].1);
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let push = IDEAL * IDEAL / dist;
                let (fx, fy) = (dx / dist * push, dy / dist * push);
                force[i].0 += fx;
                force[i].1 += fy;
                force[j].0 -= fx;
                force[j].1 -= fy;
            }
        }
        for &(a, b) in &edges {
            let (dx, dy) = (xy[a].0 - xy[b].0, xy[a].1 - xy[b].1);
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let pull = dist * dist / IDEAL * 1.2;
            let (fx, fy) = (dx / dist * pull, dy / dist * pull);
            force[a].0 -= fx;
            force[a].1 -= fy;
            force[b].0 += fx;
            force[b].1 += fy;
        }
        let mid_y = xy.iter().map(|p| p.1).sum::<f32>() / n as f32;
        for i in 0..n {
            // Gravity keeps unconnected parts from drifting apart.
            force[i].1 += (mid_y - xy[i].1) * 1.1;
            // The column spring keeps edges reading left to right.
            force[i].0 += (anchor[i] - xy[i].0) * 6.0;
            let len = (force[i].0 * force[i].0 + force[i].1 * force[i].1)
                .sqrt()
                .max(0.001);
            let step = len.min(heat);
            xy[i].0 += force[i].0 / len * step;
            xy[i].1 += force[i].1 / len * step;
        }
    }
}

/// Push apart any two nodes closer than `MIN_GAP`.
fn separate(xy: &mut [(f32, f32)], paths: &[&str]) {
    for pass in 0..40 {
        let mut moved = false;
        for i in 0..xy.len() {
            for j in (i + 1)..xy.len() {
                let (mut dx, mut dy) = (xy[i].0 - xy[j].0, xy[i].1 - xy[j].1);
                let mut dist = (dx * dx + dy * dy).sqrt();
                if dist >= MIN_GAP {
                    continue;
                }
                if dist < 0.01 {
                    dx = jitter(paths[i], pass) + 0.01;
                    dy = jitter(paths[j], pass) + 0.01;
                    dist = (dx * dx + dy * dy).sqrt();
                }
                let shift = (MIN_GAP - dist) / 2.0 + 0.5;
                let (ux, uy) = (dx / dist * shift, dy / dist * shift);
                xy[i].0 += ux;
                xy[i].1 += uy;
                xy[j].0 -= ux;
                xy[j].1 -= uy;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

/// Place every node of `graph`. The result lists one `Placed` per node, in
/// node order, with the top-left node at the origin.
pub fn layout(graph: &GraphResult, kind: LayoutKind) -> Vec<Placed> {
    let mut xy = layered_xy(graph);
    if kind == LayoutKind::Force && xy.len() > 1 {
        relax(graph, &mut xy);
    }
    let paths: Vec<&str> = graph.nodes.iter().map(|n| n.path.as_str()).collect();
    separate(&mut xy, &paths);
    let min_x = xy.iter().map(|p| p.0).fold(f32::MAX, f32::min);
    let min_y = xy.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    xy.iter()
        .zip(&graph.nodes)
        .map(|(p, n)| Placed {
            path: n.path.clone(),
            x: p.0 - min_x,
            y: p.1 - min_y,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::graph::{Degree, GraphEdge, GraphNode, NodeKind, Relation};

    fn node(path: &str) -> GraphNode {
        GraphNode {
            path: path.into(),
            kind: NodeKind::User,
            name: None,
            properties: Vec::new(),
            degree: Degree::default(),
        }
    }

    fn edge(id: &str, from: &str, to: &str) -> GraphEdge {
        GraphEdge {
            id: id.into(),
            relation: Relation::Posted,
            from: from.into(),
            to: to.into(),
            properties: Vec::new(),
        }
    }

    /// Three sources, four middles, one sink: the Cypher design's shape.
    fn fan() -> GraphResult {
        let mut g = GraphResult::default();
        for p in ["u/1", "u/2", "u/3", "p/1", "p/2", "p/3", "p/4", "t/1"] {
            g.nodes.push(node(p));
        }
        for (id, a, b) in [
            ("e1", "u/1", "p/1"),
            ("e2", "u/1", "p/2"),
            ("e3", "u/2", "p/3"),
            ("e4", "u/3", "p/4"),
            ("e5", "p/1", "t/1"),
            ("e6", "p/2", "t/1"),
            ("e7", "p/3", "t/1"),
            ("e8", "p/4", "t/1"),
        ] {
            g.edges.push(edge(id, a, b));
        }
        g
    }

    fn get<'a>(placed: &'a [Placed], path: &str) -> &'a Placed {
        placed.iter().find(|p| p.path == path).expect("placed")
    }

    #[test]
    fn layout_is_deterministic() {
        for kind in LayoutKind::ALL {
            assert_eq!(layout(&fan(), kind), layout(&fan(), kind));
        }
    }

    #[test]
    fn no_two_nodes_overlap() {
        for kind in LayoutKind::ALL {
            let p = layout(&fan(), kind);
            for (i, a) in p.iter().enumerate() {
                for b in &p[i + 1..] {
                    let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                    assert!(
                        d >= MIN_GAP - 1.0,
                        "{kind:?}: {} and {} are {d} apart",
                        a.path,
                        b.path
                    );
                }
            }
        }
    }

    #[test]
    fn edges_read_left_to_right() {
        let g = fan();
        for kind in LayoutKind::ALL {
            let p = layout(&g, kind);
            for e in &g.edges {
                assert!(
                    get(&p, &e.from).x < get(&p, &e.to).x,
                    "{kind:?}: {} -> {} runs backwards",
                    e.from,
                    e.to
                );
            }
        }
    }

    #[test]
    fn the_sink_sits_in_the_last_column() {
        let p = layout(&fan(), LayoutKind::Layered);
        let sink = get(&p, "t/1");
        assert!(p.iter().all(|n| n.x <= sink.x));
    }

    #[test]
    fn positions_are_finite_and_start_at_the_origin() {
        for kind in LayoutKind::ALL {
            let p = layout(&fan(), kind);
            assert!(p.iter().all(|n| n.x.is_finite() && n.y.is_finite()));
            assert!(p.iter().all(|n| n.x >= 0.0 && n.y >= 0.0));
            assert!(p.iter().any(|n| n.x == 0.0));
            assert!(p.iter().any(|n| n.y == 0.0));
        }
    }

    #[test]
    fn a_cycle_still_gets_a_layout() {
        let mut g = GraphResult::default();
        for p in ["a/1", "a/2", "a/3"] {
            g.nodes.push(node(p));
        }
        g.edges.push(edge("x", "a/1", "a/2"));
        g.edges.push(edge("y", "a/2", "a/3"));
        g.edges.push(edge("z", "a/3", "a/1"));
        let p = layout(&g, LayoutKind::Force);
        assert_eq!(p.len(), 3);
        assert!(p.iter().all(|n| n.x.is_finite()));
    }

    #[test]
    fn empty_and_single_node_graphs_place_cleanly() {
        assert!(layout(&GraphResult::default(), LayoutKind::Force).is_empty());
        let mut one = GraphResult::default();
        one.nodes.push(node("a/1"));
        let p = layout(&one, LayoutKind::Force);
        assert_eq!((p[0].x, p[0].y), (0.0, 0.0));
    }

    #[test]
    fn a_dangling_edge_is_ignored() {
        let mut g = fan();
        g.edges.push(edge("ghost", "u/1", "missing/9"));
        assert_eq!(layout(&g, LayoutKind::Layered).len(), 8);
    }
}
