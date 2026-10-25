//! Workspace panels: a titled frame with a six-dot grip and a close control,
//! plus the divider that resizes it.
//!
//! Visibility and size live in the shared `Workspace`, keyed by page and
//! panel id. A hidden panel stays mounted and only stops displaying, so its
//! filters, selection, and scroll position survive until the View menu shows
//! it again.
//!
//! The grip marks the panel's movement handle. Docking destinations are not
//! designed yet, so the grip does not move panels: a drop target invented
//! here would fix a layout model nobody approved.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::state::workspace::{Surface, Workspace};

/// Which way a resizable panel grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// The panel has a width.
    Horizontal,
    /// The panel has a height.
    Vertical,
}

impl Axis {
    fn class(self) -> &'static str {
        match self {
            Axis::Horizontal => "horizontal",
            Axis::Vertical => "vertical",
        }
    }
}

/// A resizable panel's default size and limits, in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelSize {
    pub axis: Axis,
    pub default: f64,
    pub min: f64,
    pub max: f64,
}

impl PanelSize {
    pub const fn width(default: f64, min: f64, max: f64) -> Self {
        Self {
            axis: Axis::Horizontal,
            default,
            min,
            max,
        }
    }

    pub const fn height(default: f64, min: f64, max: f64) -> Self {
        Self {
            axis: Axis::Vertical,
            default,
            min,
            max,
        }
    }

    fn clamp(self, px: f64) -> f64 {
        px.clamp(self.min, self.max)
    }

    fn style(self, px: f64) -> String {
        match self.axis {
            Axis::Horizontal => format!("flex: 0 0 {px}px; width: {px}px;"),
            Axis::Vertical => format!("flex: 0 0 {px}px; height: {px}px;"),
        }
    }
}

/// The current size of a resizable panel.
fn current_size(ws: &Workspace, surface: Surface, id: &str, size: PanelSize) -> f64 {
    size.clamp(ws.size(surface, id).unwrap_or(size.default))
}

/// The inline style of a resizable region that is not a closable panel, such
/// as the Query editor. It sizes like a panel and takes a `ResizeHandle`.
pub fn region_style(ws: &Workspace, surface: Surface, id: &str, size: PanelSize) -> String {
    size.style(current_size(ws, surface, id, size))
}

/// A closable workspace panel. `id` must appear in `surface.panels()`.
/// Without `size` the panel fills the space its container gives it.
#[component]
pub fn Panel(
    surface: Surface,
    id: &'static str,
    title: String,
    /// Controls at the right of the header, before the close button.
    #[props(default = VNode::empty())]
    actions: Element,
    /// Inline content right after the title, e.g. a type badge.
    #[props(default = VNode::empty())]
    title_extra: Element,
    size: Option<PanelSize>,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let mut workspace = use_context::<Signal<Workspace>>();
    let ws = workspace.read();
    let visible = ws.is_visible(surface, id);
    let style = size
        .map(|s| s.style(current_size(&ws, surface, id, s)))
        .unwrap_or_default();

    rsx! {
        section {
            class: "panel {class}",
            hidden: !visible,
            style: "{style}",
            "aria-label": "{title}",
            header { class: "panel-header",
                span {
                    class: "panel-grip",
                    title: "Move panel",
                    "aria-hidden": "true",
                    Ico { icon: Icon::GripVertical, size: 16 }
                }
                h2 { class: "panel-title", "{title}" }
                {title_extra}
                div { class: "panel-actions", {actions} }
                button {
                    class: "icon-btn panel-close",
                    title: "Close panel",
                    "aria-label": "Close {title}",
                    onclick: move |_| workspace.write().set_visible(surface, id, false),
                    Ico { icon: Icon::X, size: 16 }
                }
            }
            div { class: "panel-body", {children} }
        }
    }
}

/// Which side of the handle the resized panel sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The panel is left of (or above) the handle: dragging right grows it.
    Before,
    /// The panel is right of (or below) the handle: dragging right shrinks it.
    After,
}

/// Step for one arrow-key press on a focused handle.
const KEY_STEP: f64 = 16.0;

/// The divider that resizes panel `target`. Hidden while the target is.
#[component]
pub fn ResizeHandle(
    surface: Surface,
    target: &'static str,
    size: PanelSize,
    side: Side,
) -> Element {
    let mut workspace = use_context::<Signal<Workspace>>();
    // Pointer position and panel size when the drag started.
    let mut drag = use_signal(|| None::<(f64, f64)>);
    if !workspace.read().is_visible(surface, target) {
        return rsx! {};
    }

    let along = move |p: dioxus::html::geometry::ClientPoint| match size.axis {
        Axis::Horizontal => p.x,
        Axis::Vertical => p.y,
    };
    let sign = match side {
        Side::Before => 1.0,
        Side::After => -1.0,
    };
    let mut resize_to = move |px: f64| {
        workspace.write().set_size(surface, target, size.clamp(px));
    };
    let axis = size.axis.class();
    let orientation = match size.axis {
        Axis::Horizontal => "vertical",
        Axis::Vertical => "horizontal",
    };

    rsx! {
        div {
            class: "resize-handle {axis}",
            role: "separator",
            tabindex: "0",
            title: "Drag to resize",
            "aria-orientation": orientation,
            onmousedown: move |e| {
                e.prevent_default();
                let start = current_size(&workspace.peek(), surface, target, size);
                drag.set(Some((along(e.client_coordinates()), start)));
            },
            onkeydown: move |e| {
                let now = current_size(&workspace.peek(), surface, target, size);
                let step = match (e.key(), size.axis) {
                    (Key::ArrowRight, Axis::Horizontal) | (Key::ArrowDown, Axis::Vertical) => KEY_STEP,
                    (Key::ArrowLeft, Axis::Horizontal) | (Key::ArrowUp, Axis::Vertical) => -KEY_STEP,
                    _ => return,
                };
                e.prevent_default();
                resize_to(now + sign * step);
            },
        }
        // While dragging, a full-window layer takes every pointer event, so
        // the drag survives the pointer crossing iframes, inputs, or text.
        if let Some((from, start)) = drag() {
            div {
                class: "drag-capture {axis}",
                onmousemove: move |e| resize_to(start + sign * (along(e.client_coordinates()) - from)),
                onmouseup: move |_| drag.set(None),
                onmouseleave: move |_| drag.set(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::tabs::ClusterTab;

    const S: Surface = Surface::Cluster(ClusterTab::Nodes);
    const DETAILS: PanelSize = PanelSize::width(340.0, 240.0, 560.0);

    #[test]
    fn default_size_applies_until_resized() {
        let mut ws = Workspace::default();
        assert_eq!(current_size(&ws, S, "details", DETAILS), 340.0);
        ws.set_size(S, "details", 400.0);
        assert_eq!(current_size(&ws, S, "details", DETAILS), 400.0);
    }

    /// A size stored under looser limits (an older layout file) is clamped.
    #[test]
    fn stored_size_is_clamped_to_the_limits() {
        let mut ws = Workspace::default();
        ws.set_size(S, "details", 9000.0);
        assert_eq!(current_size(&ws, S, "details", DETAILS), 560.0);
    }
}
