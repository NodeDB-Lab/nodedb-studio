//! The projection map: the sampled points, the selection, and the state of
//! the run that made them.
//!
//! Points draw as one SVG path per color group, so thousands cost a few
//! nodes. The plot stretches to the panel, and every dot keeps a fixed
//! screen size, so stretching never makes an ellipse. A grid of cells sits
//! over the plot and takes the pointer. A cell with a few points selects the
//! nearest. A cell with more zooms in on them first, so a click never picks
//! a record from a pile it cannot tell apart.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::vector::VectorTarget;
use crate::models::vector_projection::{NO_CATEGORY, Projection, ProjectionRequest};
use crate::state::vector::{Drafts, MapTool, MapView, ProjectionUi, Selection, VectorUi, ZOOM_MAX};
use crate::views::vector::proj_tools::{GoTo, Legend, MapTools};

/// Cells over the plot, across and down.
const GX: usize = 28;
const GY: usize = 16;
/// A cell with at most this many points selects instead of zooming.
const SELECT_MAX: usize = 3;
const PICK_ZOOM: f32 = 2.5;

fn edit(mut drafts: Signal<Drafts>, key: &str, f: impl FnOnce(&mut ProjectionUi)) {
    f(drafts.write().projection_mut(key));
}

/// One path per color group: a zero-length segment per point, which a round
/// cap draws as a dot.
fn paths(p: &Projection) -> Vec<(u8, String)> {
    let mut groups: Vec<(u8, String)> = Vec::new();
    for pt in &p.points {
        let at = match groups.iter().position(|(c, _)| *c == pt.category) {
            Some(i) => i,
            None => {
                groups.push((pt.category, String::new()));
                groups.len() - 1
            }
        };
        groups[at]
            .1
            .push_str(&format!("M{:.4} {:.4}h0", pt.x, -pt.y));
    }
    groups.sort_by_key(|(c, _)| *c);
    groups
}

/// The points in each cell of the current window, as indexes into the sample.
fn bins(p: &Projection, view: MapView) -> Vec<Vec<usize>> {
    let (left, top, size) = view.window();
    let mut out = vec![Vec::new(); GX * GY];
    for (i, pt) in p.points.iter().enumerate() {
        let (fx, fy) = ((pt.x - left) / size, (-pt.y - top) / size);
        if !(0.0..1.0).contains(&fx) || !(0.0..1.0).contains(&fy) {
            continue;
        }
        out[(fy * GY as f32) as usize * GX + (fx * GX as f32) as usize].push(i);
    }
    out
}

/// The center of cell `c`, in plot units.
fn cell_center(view: MapView, c: usize) -> (f32, f32) {
    let (left, top, size) = view.window();
    (
        left + size * ((c % GX) as f32 + 0.5) / GX as f32,
        top + size * ((c / GX) as f32 + 0.5) / GY as f32,
    )
}

fn category_class(c: u8, focus: Option<u8>) -> String {
    let color = if c == NO_CATEGORY {
        "c-none".to_string()
    } else {
        format!("c{}", c % 6)
    };
    match focus {
        Some(f) if f != c => format!("pj-pts {color} dim"),
        Some(_) | None => format!("pj-pts {color}"),
    }
}

#[component]
pub fn ProjMap(
    target: VectorTarget,
    /// Parameters changed since the shown projection ran.
    stale: bool,
    running: bool,
    on_generate: EventHandler<ProjectionRequest>,
) -> Element {
    let ui = use_context::<VectorUi>();
    let drafts = ui.drafts;
    let mut selected = ui.selected;
    let key = target.label();
    let s = drafts.read().projection(&key);
    let sel_id = ui.selected_id(&key);
    let mut plot_px = use_signal(|| 700.0_f32);

    // The cell handler reads the current state when it runs, so it always
    // acts on what is on screen.
    let pick = {
        let key = key.clone();
        use_callback(move |c: usize| {
            let st = drafts.peek().projection(&key);
            let Some(p) = st.shown.clone() else { return };
            let (cx, cy) = cell_center(st.view, c);
            match st.tool {
                MapTool::Pan => edit(drafts, &key, |s| s.view = s.view.centered(cx, cy)),
                MapTool::Select => {
                    let cell = bins(&p, st.view).swap_remove(c);
                    if cell.len() > SELECT_MAX && st.view.zoom < ZOOM_MAX {
                        edit(drafts, &key, |s| {
                            s.view = s.view.zoomed_about(PICK_ZOOM, cx, cy);
                        });
                        return;
                    }
                    let nearest = cell.into_iter().min_by(|a, b| {
                        let d = |i: usize| {
                            let pt = &p.points[i];
                            (pt.x - cx).powi(2) + (-pt.y - cy).powi(2)
                        };
                        d(*a).total_cmp(&d(*b))
                    });
                    if let Some(i) = nearest {
                        selected.set(Some(Selection {
                            target: key.clone(),
                            id: p.points[i].id.clone(),
                        }));
                        edit(drafts, &key, |s| s.goto_note = None);
                    }
                }
            }
        })
    };
    let wheel = {
        let key = key.clone();
        use_callback(move |(c, up): (usize, bool)| {
            let view = drafts.peek().projection(&key).view;
            let (cx, cy) = cell_center(view, c);
            let factor = if up { 1.25 } else { 0.8 };
            edit(drafts, &key, |s| {
                s.view = s.view.zoomed_about(factor, cx, cy)
            });
        })
    };

    let Some(shown) = s.shown.clone() else {
        return rsx! {
            div { class: "vec-map empty",
                if running {
                    div { class: "vec-state", role: "status", "Sampling the index and projecting the sample…" }
                } else {
                    div { class: "vec-state",
                        "No projection yet. Choose a sample, then press Generate. The map shows only points Studio has computed."
                    }
                }
                if let Some(e) = s.error.clone() {
                    div { class: "callout err vec-error", role: "alert",
                        Ico { icon: Icon::CircleAlert, size: 16 }
                        span { "{e}" }
                    }
                }
            }
        };
    };

    let groups = use_memo(use_reactive!(|shown| paths(&shown)));
    let (left, top, size) = s.view.window();
    let cells = bins(&shown, s.view);
    // A dot keeps its screen size: its width in plot units is that size over
    // the screen size of one plot unit.
    let dot = (4.5 + s.view.zoom.log2() * 1.4).min(11.0);
    let dot_units = dot * size / plot_px().max(1.0);
    let marker = sel_id.as_deref().and_then(|id| shown.point(id)).map(|p| {
        (
            p.id.clone(),
            s.view.percent(p.x, -p.y),
            s.view.contains(p.x, -p.y),
        )
    });
    let missing = sel_id
        .as_deref()
        .filter(|id| shown.point(id).is_none())
        .map(str::to_string);
    let dense = s.view.zoom < 4.0 && shown.points.len() > 1_000;
    let field = shown.request.color_by.clone();
    let include_request = |id: &str| {
        let mut form_include = s.include.clone();
        if !form_include.iter().any(|i| i == id) {
            form_include.push(id.to_string());
        }
        s.form.check(&target, &form_include)
    };
    let regenerate = missing.as_deref().map(include_request);
    let var = format!(
        "Axes keep {:.0}% and {:.0}% of the sample's variance",
        shown.explained[0] * 100.0,
        shown.explained[1] * 100.0
    );

    rsx! {
        div { class: "vec-meta",
            span { class: "vec-meta-main", "{shown.summary()}" }
            if !shown.request.include.is_empty() {
                span { " · includes {shown.request.include.join(\", \")}" }
            }
            span { class: "vec-meta-var", title: "PCA keeps the two directions of greatest variance. Distances on screen are not distances between the vectors.", "{var}" }
            if running {
                span { class: "badge blue", "Running" }
            } else if stale {
                span { class: "badge amber", title: "The controls changed. This map shows the last run.", "Stale" }
            }
        }
        if let Some(e) = s.error.clone() {
            div { class: "callout err vec-error", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                span { "{e} The last projection stays on the map." }
            }
        }
        div {
            class: if running || stale { "vec-map stale" } else { "vec-map" },
            tabindex: "0",
            "aria-label": "Projection map. Arrow keys pan. Plus and minus zoom.",
            onkeydown: {
                let key = key.clone();
                move |e| {
                    let step = |dx: f32, dy: f32| edit(drafts, &key, |s| {
                        let h = s.view.half() * 0.25;
                        s.view = s.view.centered(s.view.cx + dx * h, s.view.cy + dy * h);
                    });
                    match e.key() {
                        Key::ArrowLeft => step(-1.0, 0.0),
                        Key::ArrowRight => step(1.0, 0.0),
                        Key::ArrowUp => step(0.0, -1.0),
                        Key::ArrowDown => step(0.0, 1.0),
                        Key::Character(c) if c == "+" || c == "=" => {
                            edit(drafts, &key, |s| s.view = s.view.zoomed(1.5));
                        }
                        Key::Character(c) if c == "-" => {
                            edit(drafts, &key, |s| s.view = s.view.zoomed(1.0 / 1.5));
                        }
                        Key::Character(c) if c == "0" => {
                            edit(drafts, &key, |s| s.view = MapView::default());
                        }
                        _ => return,
                    }
                    e.prevent_default();
                }
            },
            div {
                class: "pj-plot",
                onresize: move |e| {
                    if let Ok(sz) = e.get_border_box_size() {
                        let px = sz.width as f32;
                        if (px - *plot_px.peek()).abs() > 1.0 { plot_px.set(px); }
                    }
                },
                svg {
                    class: "pj-svg",
                    role: "img",
                    "aria-label": "Two-dimensional projection of {shown.points.len()} sampled records",
                    view_box: "{left} {top} {size} {size}",
                    for (c , d) in groups.read().iter() {
                        path {
                            key: "{c}",
                            class: category_class(*c, s.focus),
                            d: "{d}",
                            fill: "none",
                            stroke_linecap: "round",
                            style: "stroke-width:{dot_units}px",
                        }
                    }
                }
                div { class: if s.tool == MapTool::Pan { "pj-grid pan" } else { "pj-grid" },
                    for (c , ids) in cells.iter().enumerate() {
                        {
                            let title = if !ids.is_empty() && ids.len() <= SELECT_MAX {
                                ids.iter().map(|i| shown.points[*i].id.as_str()).collect::<Vec<_>>().join(", ")
                            } else if ids.len() > SELECT_MAX {
                                format!("{} records here. Click to zoom in.", ids.len())
                            } else {
                                String::new()
                            };
                            rsx! {
                                div {
                                    key: "{c}",
                                    class: if ids.is_empty() { "pj-cell empty" } else { "pj-cell" },
                                    title: "{title}",
                                    onclick: move |_| pick.call(c),
                                    onwheel: move |e| {
                                        e.prevent_default();
                                        wheel.call((c, e.delta().strip_units().y < 0.0));
                                    },
                                }
                            }
                        }
                    }
                }
                if let Some((id, (px, py), inside)) = marker {
                    if inside {
                        div { class: "pj-marker", style: "left:{px}%; top:{py}%;",
                            span { class: "pj-ring" }
                            span { class: "pj-label mono", "{id}" }
                        }
                    }
                }
            }
            if let Some(f) = field.clone() {
                Legend { tkey: key.clone(), field: f, categories: shown.categories.clone(), focus: s.focus }
            }
            GoTo { tkey: key.clone(), projection: shown.clone(), note: s.goto_note.clone() }
            MapTools { tkey: key.clone(), view: s.view, tool: s.tool }
            if running {
                div { class: "pj-busy", role: "status", "Projecting a new sample. The map shows the last run." }
            }
        }
        div { class: "pj-foot",
            span { class: "pj-caption",
                "Projection is approximate. Use Find similar for ranked distances."
            }
            if let Some(id) = missing {
                span { class: "pj-missing", role: "status",
                    "{id} is not in this sample. "
                    button {
                        class: "link-btn",
                        disabled: !matches!(regenerate, Some(Ok(_))) || running,
                        title: "Draw a new sample that includes {id}. It can move every point.",
                        onclick: move |_| {
                            if let Some(Ok(req)) = regenerate.clone() { on_generate.call(req); }
                        },
                        "Include and regenerate"
                    }
                }
            }
            if dense {
                span { class: "pj-dense", "Dense areas overlap at this zoom. Click one to zoom in." }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::vector_projection::{Method, ProjectionPoint};

    fn projection(points: Vec<(f32, f32, u8)>) -> Projection {
        Projection {
            request: ProjectionRequest {
                collection: "articles".into(),
                field: "embedding".into(),
                method: Method::Pca,
                sample: points.len() as u32,
                seed: 1,
                color_by: None,
                include: Vec::new(),
            },
            population: 100,
            revision: 1,
            algorithm: "pca".into(),
            generated_at: 0,
            explained: [0.5, 0.2],
            categories: Vec::new(),
            points: points
                .into_iter()
                .enumerate()
                .map(|(i, (x, y, c))| ProjectionPoint {
                    id: format!("art{i}"),
                    x,
                    y,
                    category: c,
                })
                .collect(),
        }
    }

    #[test]
    fn every_point_lands_in_one_cell_of_the_full_view() {
        let p = projection(vec![
            (-1.0, 1.0, NO_CATEGORY),
            (0.0, 0.0, NO_CATEGORY),
            (1.0, -1.0, NO_CATEGORY),
        ]);
        let b = bins(&p, MapView::default());
        assert_eq!(b.iter().map(Vec::len).sum::<usize>(), 3);
    }

    #[test]
    fn a_zoomed_view_drops_points_outside_it() {
        let p = projection(vec![(0.0, 0.0, NO_CATEGORY), (0.95, 0.95, NO_CATEGORY)]);
        let b = bins(&p, MapView::default().zoomed(8.0));
        assert_eq!(b.iter().map(Vec::len).sum::<usize>(), 1);
    }

    #[test]
    fn paths_group_points_by_category() {
        let p = projection(vec![(0.0, 0.0, 0), (0.1, 0.1, 1), (0.2, 0.2, 0)]);
        let g = paths(&p);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].1.matches('M').count(), 2);
    }

    #[test]
    fn cell_centers_sit_inside_the_window() {
        let v = MapView::default();
        let (x, y) = cell_center(v, 0);
        let (left, top, size) = v.window();
        assert!(x > left && x < left + size && y > top && y < top + size);
    }
}
