//! Canvas geometry: card sizes, where each card sits, and where a reference
//! connector runs. Plain numbers in canvas pixels, so the CSS row heights
//! and these constants stay one definition.

use crate::models::designer::{DesignCollection, DesignSchema, LogicalReference};
use crate::state::workspace::Workspace;

pub const CARD_W: f64 = 372.0;
pub const HEADER_H: f64 = 52.0;
pub const SECTION_H: f64 = 30.0;
pub const FIELD_H: f64 = 32.0;
pub const INDEX_H: f64 = 40.0;
pub const SETTINGS_H: f64 = 40.0;
pub const CARD_PAD: f64 = 10.0;

const COLUMNS: [f64; 2] = [40.0, 436.0];
const TOP: f64 = 28.0;
const ROW_GAP: f64 = 40.0;
/// The channel left of a card that a connector runs in.
const MARGIN_LANE: f64 = 28.0;
/// The smallest x a card can be dragged to, so its lane stays on the canvas.
pub const MIN_X: f64 = MARGIN_LANE + 12.0;
pub const MIN_Y: f64 = 12.0;

/// The height of a card for `c`.
pub fn card_height(c: &DesignCollection) -> f64 {
    let mut h = HEADER_H + FIELD_H * c.fields.len() as f64 + CARD_PAD;
    if c.heading.is_some() {
        h += SECTION_H;
    }
    if !c.indexes.is_empty() {
        h += SECTION_H + INDEX_H * c.indexes.len() as f64;
    }
    if c.settings.is_some() {
        h += SETTINGS_H;
    }
    h
}

/// Distance from a card's top to the vertical center of field `i`.
pub fn field_center(c: &DesignCollection, i: usize) -> f64 {
    let heading = if c.heading.is_some() { SECTION_H } else { 0.0 };
    HEADER_H + heading + FIELD_H * i as f64 + FIELD_H / 2.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
}

/// Two columns, filled row by row, each row as tall as its tallest card.
pub fn auto_positions(schema: &DesignSchema) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(schema.collections.len());
    let mut y = TOP;
    for row in schema.collections.chunks(COLUMNS.len()) {
        let tallest = row.iter().map(card_height).fold(0.0, f64::max);
        out.extend(COLUMNS.iter().take(row.len()).map(|x| (*x, y)));
        y += tallest + ROW_GAP;
    }
    out
}

/// Each card's rectangle: where the user dragged it, else its automatic
/// place.
pub fn rects(schema: &DesignSchema, layout: &Workspace) -> Vec<(String, Rect)> {
    let auto = auto_positions(schema);
    schema
        .collections
        .iter()
        .zip(auto)
        .map(|(c, (ax, ay))| {
            let (x, y) = layout.card_position(c.name()).unwrap_or((ax, ay));
            (
                c.name().to_string(),
                Rect {
                    x,
                    y,
                    w: CARD_W,
                    h: card_height(c),
                },
            )
        })
        .collect()
}

/// The canvas size that holds every card with room around it.
pub fn world(rects: &[(String, Rect)]) -> (f64, f64) {
    let w = rects.iter().map(|(_, r)| r.right()).fold(0.0, f64::max);
    let h = rects.iter().map(|(_, r)| r.bottom()).fold(0.0, f64::max);
    (w + 16.0, h + 16.0)
}

/// A connector between two field rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub points: Vec<(f64, f64)>,
    /// Top-left of the label, in the clear space beside the line.
    pub label: (f64, f64),
}

impl Route {
    /// The SVG path of the polyline.
    pub fn path(&self) -> String {
        self.points
            .iter()
            .enumerate()
            .map(|(i, (x, y))| format!("{}{x:.1},{y:.1}", if i == 0 { "M" } else { " L" }))
            .collect()
    }
}

/// Route from the row at `from_y` of card `from` to the row at `to_y` of
/// card `to`. A connector never crosses a card: cards side by side meet in
/// the gap between them, and stacked cards meet in the lane left of both.
pub fn route(from: Rect, from_y: f64, to: Rect, to_y: f64) -> Route {
    if from.right() <= to.x {
        let mid = (from.right() + to.x) / 2.0;
        return Route {
            points: vec![
                (from.right(), from_y),
                (mid, from_y),
                (mid, to_y),
                (to.x, to_y),
            ],
            label: (mid + 8.0, (from_y + to_y) / 2.0 - 16.0),
        };
    }
    if to.right() <= from.x {
        let mid = (to.right() + from.x) / 2.0;
        return Route {
            points: vec![
                (from.x, from_y),
                (mid, from_y),
                (mid, to_y),
                (to.right(), to_y),
            ],
            label: (mid + 8.0, (from_y + to_y) / 2.0 - 16.0),
        };
    }
    let lane = from.x.min(to.x) - MARGIN_LANE;
    let (upper, lower) = if from.y <= to.y {
        (from, to)
    } else {
        (to, from)
    };
    // The label sits in the clear band between the two cards when there is
    // one, else beside the lane.
    let gap = lower.y - upper.bottom();
    let label_y = if gap >= 40.0 {
        upper.bottom() + (gap - 32.0) / 2.0
    } else {
        (from_y + to_y) / 2.0 - 16.0
    };
    Route {
        points: vec![(from.x, from_y), (lane, from_y), (lane, to_y), (to.x, to_y)],
        label: (from.x.min(to.x) + 40.0, label_y),
    }
}

/// The route of a reference, when both its cards are drawn.
pub fn route_of(
    r: &LogicalReference,
    schema: &DesignSchema,
    rects: &[(String, Rect)],
) -> Option<Route> {
    // The card's rectangle and the canvas y of the row's center.
    let end = |c: &str, f: &str| {
        let rect = rects.iter().find(|(n, _)| n == c).map(|(_, r)| *r)?;
        let design = schema.collection(c)?;
        let i = design.fields.iter().position(|x| x.name() == f)?;
        Some((rect, rect.y + field_center(design, i)))
    };
    let (from, from_y) = end(&r.from_collection, &r.from_field)?;
    let (to, to_y) = end(&r.to_collection, &r.to_field)?;
    Some(route(from, from_y, to, to_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::designer_data::DesignerData;

    async fn design_schema() -> DesignSchema {
        MockConnectionService::ready()
            .design_schema()
            .await
            .expect("the mock schema reads")
    }

    fn rect(x: f64, y: f64, h: f64) -> Rect {
        Rect { x, y, w: CARD_W, h }
    }

    #[tokio::test]
    async fn the_four_cards_fill_two_rows_of_two() {
        let s = design_schema().await;
        let p = auto_positions(&s);
        assert_eq!(p.len(), 4);
        assert_eq!((p[0].0, p[1].0, p[2].0, p[3].0), (40.0, 436.0, 40.0, 436.0));
        assert_eq!(p[0].1, p[1].1);
        assert_eq!(p[2].1, p[3].1);
        assert!(p[2].1 > p[0].1);
    }

    #[tokio::test]
    async fn the_four_cards_fit_one_screen_at_actual_size() {
        let s = design_schema().await;
        let (w, h) = world(&rects(&s, &Workspace::default()));
        assert!(w <= 830.0, "width {w}");
        assert!(h <= 650.0, "height {h}");
    }

    #[tokio::test]
    async fn a_dragged_card_keeps_its_place() {
        let s = design_schema().await;
        let mut ws = Workspace::default();
        ws.set_card_position("orders", 100.0, 500.0);
        let r = rects(&s, &ws);
        let orders = r.iter().find(|(n, _)| n == "orders").map(|(_, r)| *r);
        assert_eq!(orders.map(|r| (r.x, r.y)), Some((100.0, 500.0)));
    }

    #[test]
    fn stacked_cards_connect_in_the_left_lane() {
        let a = rect(56.0, 36.0, 260.0);
        let b = rect(56.0, 400.0, 190.0);
        let r = route(a, 100.0, b, 480.0);
        assert_eq!(r.points[0], (56.0, 100.0), "starts on the left border");
        assert_eq!(r.points[3], (56.0, 480.0), "ends on the left border");
        assert!(
            r.points[1].0 < 56.0 && r.points[2].0 < 56.0,
            "lane is left of both"
        );
        // The label sits in the band between the cards.
        assert!(r.label.1 > a.bottom() && r.label.1 < b.y);
    }

    #[test]
    fn side_by_side_cards_connect_through_the_gap() {
        let a = rect(56.0, 36.0, 260.0);
        let b = rect(444.0, 36.0, 260.0);
        let r = route(a, 100.0, b, 150.0);
        assert_eq!(r.points[0].0, a.right());
        assert_eq!(r.points[3].0, b.x);
        assert!(r.points[1].0 > a.right() && r.points[1].0 < b.x);
    }

    #[tokio::test]
    async fn the_reference_runs_between_the_named_rows() {
        let s = design_schema().await;
        let r = rects(&s, &Workspace::default());
        let route = route_of(&s.references[0], &s, &r).expect("both cards drawn");
        let at = |n: &str| {
            r.iter()
                .find(|(c, _)| c == n)
                .map(|(_, r)| *r)
                .expect("card")
        };
        // orders.customer_id is the second field of orders, users.id the
        // first of users.
        let customer = at("orders").y + field_center(&s.collections[2], 1);
        let users_id = at("users").y + field_center(&s.collections[0], 0);
        assert_eq!(route.points[0], (at("orders").x, customer));
        assert_eq!(route.points[3], (at("users").x, users_id));
    }

    #[tokio::test]
    async fn a_card_is_as_tall_as_its_rows() {
        let s = design_schema().await;
        let users = &s.collections[0];
        assert_eq!(
            card_height(users),
            HEADER_H + SECTION_H + 3.0 * FIELD_H + SECTION_H + INDEX_H + CARD_PAD
        );
    }
}
