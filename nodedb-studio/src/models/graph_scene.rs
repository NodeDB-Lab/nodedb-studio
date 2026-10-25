//! The canvas's drawing, as numbers: where each node sits, what it says,
//! and the curve each edge takes. The view only turns these into SVG, so
//! the geometry is testable without a renderer.

use crate::models::graph::{GraphResult, NodeKind};
use crate::models::graph_layout::Placed;

/// Half the width of a node's shape.
pub const NODE_R: f32 = 22.0;
/// Edges stop this far short of a node's centre, past its border.
const GAP: f32 = NODE_R + 4.0;
/// Room around the drawing for labels.
const PAD_X: f32 = 24.0;
const PAD_TOP: f32 = 56.0;
const PAD_BOTTOM: f32 = 110.0;
/// Longest label line, in characters.
const LABEL_CHARS: usize = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    pub path: String,
    pub kind: NodeKind,
    pub x: f32,
    pub y: f32,
    /// The bold line under the shape.
    pub label: String,
    /// The small line under that.
    pub sub: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    pub id: String,
    pub relation: &'static str,
    /// An SVG path from the source's border to the target's border.
    pub d: String,
    pub label_x: f32,
    pub label_y: f32,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
    /// `min_x min_y width height`.
    pub view_box: (f32, f32, f32, f32),
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Rect {
    fn overlaps(self, o: Rect) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }
}

/// The box of a relation label centred on `x` with baseline `y`.
fn label_rect(x: f32, y: f32, text: &str) -> Rect {
    let half = text.chars().count() as f32 * 4.6 + 4.0;
    Rect {
        x0: x - half,
        y0: y - 12.0,
        x1: x + half,
        y1: y + 4.0,
    }
}

/// A node's shape, and the two lines of text under it.
fn node_rects(n: &SceneNode) -> [Rect; 2] {
    let shape = NODE_R + 2.0;
    let text =
        (n.label.chars().count() as f32 * 9.0).max(n.sub.chars().count() as f32 * 7.6) / 2.0 + 2.0;
    [
        Rect {
            x0: n.x - shape,
            y0: n.y - shape,
            x1: n.x + shape,
            y1: n.y + shape,
        },
        Rect {
            x0: n.x - text,
            y0: n.y + NODE_R + 6.0,
            x1: n.x + text,
            y1: n.y + NODE_R + 42.0,
        },
    ]
}

/// Where a relation label sits: on the curve near its middle, above it, else
/// below it, else further along, taking the first spot that overlaps no
/// node, node text, or earlier label.
fn place_label(
    a: (f32, f32),
    c: (f32, f32),
    b: (f32, f32),
    text: &str,
    blocked: &[Rect],
    taken: &mut Vec<Rect>,
) -> (f32, f32) {
    let at = |t: f32| {
        let u = 1.0 - t;
        (
            u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
            u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
        )
    };
    // Every spot along the curve, middle first, above the line then below
    // it. The first clear spot wins. With none clear, the one that hits the
    // fewest boxes does.
    let mut best: Option<((f32, f32), Rect, usize)> = None;
    let mut steps: Vec<f32> = (1..=17).map(|i| i as f32 * 0.05).collect();
    steps.sort_by(|x, y| (x - 0.5).abs().total_cmp(&(y - 0.5).abs()));
    for t in steps {
        let (px, py) = at(t);
        for lift in [-9.0, 17.0, -26.0, 36.0] {
            let spot = (px, py + lift);
            let rect = label_rect(spot.0, spot.1, text);
            let hits = blocked
                .iter()
                .chain(taken.iter())
                .filter(|r| r.overlaps(rect))
                .count();
            if hits == 0 {
                taken.push(rect);
                return spot;
            }
            if best.is_none_or(|(_, _, h)| hits < h) {
                best = Some((spot, rect, hits));
            }
        }
    }
    let (spot, rect, _) = best.unwrap_or((a, label_rect(a.0, a.1, text), 0));
    taken.push(rect);
    spot
}

fn clip(text: &str) -> String {
    if text.chars().count() <= LABEL_CHARS {
        text.to_string()
    } else {
        let cut: String = text.chars().take(LABEL_CHARS - 1).collect();
        format!("{cut}…")
    }
}

fn dist_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dy * dy).max(0.0001);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    let d = ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt();
    // The side of the chord the point lies on.
    let side = dx * (p.1 - a.1) - dy * (p.0 - a.0);
    (d, side)
}

/// How far, and to which side, an edge bows from its chord. Parallel edges
/// between one pair fan out. An edge that would cross a node bows away from
/// it.
fn bow(a: (f32, f32), b: (f32, f32), others: &[(f32, f32)], parallel: usize, lane: usize) -> f32 {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let fan = if parallel > 1 {
        (lane as f32 - (parallel as f32 - 1.0) / 2.0) * 34.0
    } else {
        len * 0.08
    };
    let mut crossing = 0.0f32;
    for &o in others {
        let (d, side) = dist_to_segment(o, a, b);
        if d < NODE_R + 18.0 {
            let away = if side > 0.0 { -1.0 } else { 1.0 };
            crossing = away * (len * 0.22).min(70.0);
        }
    }
    if crossing != 0.0 { crossing } else { fan }
}

/// Build the scene of `graph` with `placed` positions, in graph order.
pub fn build(graph: &GraphResult, placed: &[Placed]) -> Scene {
    let at = |path: &str| placed.iter().find(|p| p.path == path).map(|p| (p.x, p.y));
    let nodes: Vec<SceneNode> = graph
        .nodes
        .iter()
        .filter_map(|n| {
            let (x, y) = at(&n.path)?;
            let (label, sub) = match &n.name {
                Some(name) => (clip(name), clip(n.id())),
                None => (clip(n.id()), n.kind.label().to_string()),
            };
            Some(SceneNode {
                path: n.path.clone(),
                kind: n.kind,
                x,
                y,
                label,
                sub,
            })
        })
        .collect();

    // Everything an edge label must keep clear of: node shapes and their
    // text. `taken` collects the labels placed so far.
    let blocked: Vec<Rect> = nodes.iter().flat_map(node_rects).collect();
    let mut taken: Vec<Rect> = Vec::new();
    let mut edges = Vec::new();
    for (i, e) in graph.edges.iter().enumerate() {
        let (Some(a), Some(b)) = (at(&e.from), at(&e.to)) else {
            continue;
        };
        let same = |o: &crate::models::graph::GraphEdge| {
            (o.from == e.from && o.to == e.to) || (o.from == e.to && o.to == e.from)
        };
        let parallel = graph.edges.iter().filter(|o| same(o)).count();
        let lane = graph.edges[..i].iter().filter(|o| same(o)).count();
        let others: Vec<(f32, f32)> = placed
            .iter()
            .filter(|p| p.path != e.from && p.path != e.to)
            .map(|p| (p.x, p.y))
            .collect();
        let offset = bow(a, b, &others, parallel, lane);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let (ux, uy) = (dx / len, dy / len);
        // Left normal of the chord.
        let (nx, ny) = (uy, -ux);
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let ctrl = (mid.0 + nx * offset * 2.0, mid.1 + ny * offset * 2.0);
        // Start and end on the borders, along the tangent at each end.
        let toward = |from: (f32, f32), to: (f32, f32)| {
            let (tx, ty) = (to.0 - from.0, to.1 - from.1);
            let l = (tx * tx + ty * ty).sqrt().max(1.0);
            (tx / l, ty / l)
        };
        let t0 = toward(a, ctrl);
        let t1 = toward(b, ctrl);
        let start = (a.0 + t0.0 * GAP, a.1 + t0.1 * GAP);
        let end = (b.0 + t1.0 * GAP, b.1 + t1.1 * GAP);
        let (label_x, label_y) =
            place_label(start, ctrl, end, e.relation.label(), &blocked, &mut taken);
        edges.push(SceneEdge {
            id: e.id.clone(),
            relation: e.relation.label(),
            d: format!(
                "M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1}",
                start.0, start.1, ctrl.0, ctrl.1, end.0, end.1
            ),
            label_x,
            label_y,
            from: e.from.clone(),
            to: e.to.clone(),
        });
    }

    // The drawing's bounds include each node's text, so wide labels at the
    // edge are never cut off.
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = -PAD_TOP;
    let mut max_y = 0.0f32;
    for n in &nodes {
        let [_, text] = node_rects(n);
        min_x = min_x.min(text.x0);
        max_x = max_x.max(text.x1);
        max_y = max_y.max(n.y);
    }
    for r in &taken {
        min_x = min_x.min(r.x0);
        max_x = max_x.max(r.x1);
        min_y = min_y.min(r.y0 - 8.0);
    }
    if nodes.is_empty() {
        (min_x, max_x) = (0.0, 0.0);
    }
    Scene {
        nodes,
        edges,
        view_box: (
            min_x - PAD_X,
            min_y,
            max_x - min_x + 2.0 * PAD_X,
            max_y + PAD_BOTTOM - min_y,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::graph::{Degree, GraphEdge, GraphNode, Relation};
    use crate::models::graph_layout::{LayoutKind, layout};

    fn node(path: &str, kind: NodeKind, name: Option<&str>) -> GraphNode {
        GraphNode {
            path: path.into(),
            kind,
            name: name.map(str::to_string),
            properties: Vec::new(),
            degree: Degree::default(),
        }
    }

    fn graph() -> GraphResult {
        GraphResult {
            nodes: vec![
                node("users/u1", NodeKind::User, Some("Hana Idris")),
                node("posts/p1", NodeKind::Post, None),
                node("topics/databases", NodeKind::Topic, None),
            ],
            edges: vec![
                GraphEdge {
                    id: "a".into(),
                    relation: Relation::Posted,
                    from: "users/u1".into(),
                    to: "posts/p1".into(),
                    properties: Vec::new(),
                },
                GraphEdge {
                    id: "b".into(),
                    relation: Relation::Tagged,
                    from: "posts/p1".into(),
                    to: "topics/databases".into(),
                    properties: Vec::new(),
                },
            ],
            truncated: false,
        }
    }

    fn scene() -> Scene {
        let g = graph();
        build(&g, &layout(&g, LayoutKind::Layered))
    }

    #[test]
    fn a_named_node_shows_its_name_over_its_id() {
        let s = scene();
        assert_eq!(
            (s.nodes[0].label.as_str(), s.nodes[0].sub.as_str()),
            ("Hana Idris", "u1")
        );
        assert_eq!(
            (s.nodes[1].label.as_str(), s.nodes[1].sub.as_str()),
            ("p1", "Post")
        );
    }

    #[test]
    fn every_edge_is_drawn_with_its_relation() {
        let s = scene();
        assert_eq!(s.edges.len(), 2);
        assert_eq!(s.edges[0].relation, "POSTED");
        assert_eq!(s.edges[1].relation, "TAGGED");
        assert!(
            s.edges
                .iter()
                .all(|e| e.d.starts_with('M') && e.d.contains('Q'))
        );
    }

    #[test]
    fn an_edge_ends_on_the_target_border_not_its_centre() {
        let g = graph();
        let placed = layout(&g, LayoutKind::Layered);
        let s = build(&g, &placed);
        let target = placed
            .iter()
            .find(|p| p.path == "posts/p1")
            .expect("placed");
        let end: Vec<f32> = s.edges[0]
            .d
            .rsplit(' ')
            .take(2)
            .map(|v| v.parse().expect("number"))
            .collect();
        let (ey, ex) = (end[0], end[1]);
        let d = ((ex - target.x).powi(2) + (ey - target.y).powi(2)).sqrt();
        assert!((d - GAP).abs() < 0.5, "ends {d} from the centre");
    }

    #[test]
    fn the_view_box_holds_every_node() {
        let s = scene();
        let (x, y, w, h) = s.view_box;
        for n in &s.nodes {
            assert!(n.x >= x && n.x <= x + w && n.y >= y && n.y <= y + h);
        }
    }

    /// A node sits where the edge's label would, and a second edge shares
    /// the first's midpoint. No label overlaps a node, its text, or a label.
    #[test]
    fn edge_labels_clear_nodes_and_each_other() {
        let mut g = GraphResult::default();
        for (p, k) in [
            ("a/1", NodeKind::Order),
            ("b/1", NodeKind::Product),
            ("c/1", NodeKind::Order),
        ] {
            g.nodes.push(node(p, k, None));
        }
        for (id, from, to) in [("x", "a/1", "b/1"), ("y", "a/1", "c/1")] {
            g.edges.push(GraphEdge {
                id: id.into(),
                relation: Relation::Contains,
                from: from.into(),
                to: to.into(),
                properties: Vec::new(),
            });
        }
        let placed = vec![
            Placed {
                path: "a/1".into(),
                x: 0.0,
                y: 0.0,
            },
            Placed {
                path: "b/1".into(),
                x: 220.0,
                y: 0.0,
            },
            // Under the first edge's midpoint.
            Placed {
                path: "c/1".into(),
                x: 110.0,
                y: 12.0,
            },
        ];
        let s = build(&g, &placed);
        let blocked: Vec<Rect> = s.nodes.iter().flat_map(node_rects).collect();
        let labels: Vec<Rect> = s
            .edges
            .iter()
            .map(|e| label_rect(e.label_x, e.label_y, e.relation))
            .collect();
        for (i, l) in labels.iter().enumerate() {
            assert!(
                !blocked.iter().any(|b| l.overlaps(*b)),
                "label {i} hits a node"
            );
            for o in &labels[i + 1..] {
                assert!(!l.overlaps(*o), "labels overlap");
            }
        }
    }

    #[test]
    fn long_labels_are_clipped() {
        assert_eq!(clip("short"), "short");
        assert_eq!(clip(&"x".repeat(40)).chars().count(), LABEL_CHARS);
    }

    #[test]
    fn parallel_edges_take_different_lanes() {
        let mut g = graph();
        g.edges.push(GraphEdge {
            id: "c".into(),
            relation: Relation::Posted,
            from: "users/u1".into(),
            to: "posts/p1".into(),
            properties: Vec::new(),
        });
        let s = build(&g, &layout(&g, LayoutKind::Layered));
        assert_ne!(s.edges[0].d, s.edges[2].d);
    }
}
