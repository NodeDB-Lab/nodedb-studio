//! One dashboard panel: a titled chart that runs its own query. A panel
//! error shows in that panel alone. The six-dot grip moves the panel (Up and
//! Down arrows while it has focus), × hides it for this person, and Remove
//! panel in its menu changes the document.

use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::clipboard::copy_text;
use crate::components::icon::{Ico, Icon};
use crate::models::timeseries::ExploreQuery;
use crate::models::ts_dashboard::{Dashboard, MAX_PANEL_HEIGHT, MIN_PANEL_HEIGHT, PanelDef};
use crate::routes::Route;
use crate::routes::tabs::TimeseriesTab;
use crate::services::backend::Backend;
use crate::state::timeseries::{DashDialog, Notice, TimeseriesUi};
use crate::state::workspace::{Surface, Workspace};
use crate::views::timeseries::chart::TsChart;
use crate::views::timeseries::dash_ops::{open_editor, panel_key, remove};
use crate::views::timeseries::explore_tasks::drill_into_explore;

pub const SURFACE: Surface = Surface::Timeseries(TimeseriesTab::Dashboards);

/// Series a dashboard chart draws: panels are compact.
const SERIES_LIMIT: usize = 3;
/// One arrow-key press on a divider.
const KEY_STEP: u32 = 16;

#[component]
pub fn DashPanel(
    doc: Dashboard,
    panel: PanelDef,
    first: bool,
    last: bool,
    refresh: u32,
    menu_open: bool,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    let mut workspace = use_context::<Signal<Workspace>>();
    let mut hidden_series = use_signal(BTreeSet::<String>::new);

    let key = panel_key(&doc, &panel.id);
    let visible = workspace.read().is_visible(SURFACE, &key);
    let query = doc.run_query(&panel);
    let read = {
        let backend = backend.clone();
        let q = query.clone();
        use_resource(use_reactive!(|q, refresh| {
            let backend = backend.clone();
            let _refresh = refresh;
            async move { backend.explore(&q).await }
        }))
    };
    let result = read.read().clone();
    let id = panel.id.clone();
    let subtitle = format!(
        "{} · {}",
        panel.query.collection,
        panel.query.measure.label()
    );
    let style = format!("height:{}px; flex:none;", panel.height);

    let move_by = {
        let id = id.clone();
        move |delta: isize| {
            let mut w = dash.write();
            if let Some(d) = w.open.as_mut() {
                d.move_panel(&id, delta);
            }
        }
    };

    rsx! {
        section {
            class: "panel ts-dash-panel",
            hidden: !visible,
            style: "{style}",
            "aria-label": "{panel.title}",
            header { class: "panel-header",
                button {
                    class: "panel-grip ts-grip",
                    title: "Move panel. Press Up or Down while this has focus.",
                    "aria-label": "Move {panel.title}. Up and Down arrows reorder.",
                    onkeydown: {
                        let mut move_by = move_by.clone();
                        move |e| match e.key() {
                            Key::ArrowUp => { e.prevent_default(); move_by(-1); }
                            Key::ArrowDown => { e.prevent_default(); move_by(1); }
                            _ => {}
                        }
                    },
                    Ico { icon: Icon::GripVertical, size: 16 }
                }
                h2 { class: "panel-title", "{panel.title}" }
                span { class: "ts-panel-sub", "{subtitle}" }
                div { class: "panel-actions",
                    button {
                        class: "link-btn",
                        title: "Edit this panel",
                        onclick: {
                            let id = id.clone();
                            move |_| open_editor(&mut dash.write(), &id)
                        },
                        Ico { icon: Icon::Pencil, size: 16 }
                        "Edit"
                    }
                    div { class: "anchor",
                        button {
                            class: "icon-btn",
                            title: "Panel actions",
                            "aria-label": "Actions for {panel.title}",
                            "aria-haspopup": "menu",
                            "aria-expanded": "{menu_open}",
                            onclick: {
                                let id = id.clone();
                                move |_| {
                                    let mut w = dash.write();
                                    w.menu = if w.menu.as_deref() == Some(id.as_str()) { None } else { Some(id.clone()) };
                                }
                            },
                            Ico { icon: Icon::Ellipsis, size: 16 }
                        }
                        if menu_open {
                            div { class: "ts-scrim", onclick: move |_| dash.write().menu = None }
                            PanelMenu { doc: doc.clone(), panel: panel.clone(), query: query.clone(), first, last }
                        }
                    }
                }
                button {
                    class: "icon-btn panel-close",
                    title: "Hide panel. The dashboard keeps it. View lists it again.",
                    "aria-label": "Hide {panel.title}",
                    onclick: move |_| workspace.write().set_visible(SURFACE, &key, false),
                    Ico { icon: Icon::X, size: 16 }
                }
            }
            div { class: "panel-body ts-panel-body",
                match result {
                    None => rsx! { div { class: "ts-state", "Loading…" } },
                    Some(Err(e)) => rsx! {
                        div { class: "callout err ts-panel-error", role: "alert",
                            Ico { icon: Icon::CircleAlert, size: 16 }
                            span { class: "notice-text", "{e}" }
                            button {
                                class: "btn sm",
                                onclick: {
                                    let id = id.clone();
                                    move |_| open_editor(&mut dash.write(), &id)
                                },
                                "Edit panel"
                            }
                            button { class: "btn sm", onclick: { let mut read = read; move |_| read.restart() }, "Retry" }
                        }
                    },
                    Some(Ok(r)) => rsx! {
                        TsChart {
                            result: r,
                            kind: panel.kind,
                            hidden: hidden_series(),
                            limit: SERIES_LIMIT,
                            caption: false,
                            on_toggle: move |k: String| {
                                let mut s = hidden_series.write();
                                if !s.remove(&k) {
                                    s.insert(k);
                                }
                            },
                        }
                    },
                }
            }
        }
    }
}

/// The panel's actions. Remove panel is here, apart from ×.
#[component]
fn PanelMenu(
    doc: Dashboard,
    panel: PanelDef,
    query: ExploreQuery,
    first: bool,
    last: bool,
) -> Element {
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    let nav = use_navigator();
    let backend = use_context::<Rc<dyn Backend>>();
    let id = panel.id.clone();

    rsx! {
        div { class: "popover align-right ts-panel-menu", role: "menu", "aria-label": "Panel actions",
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    e.stop_propagation();
                    dash.write().menu = None;
                }
            },
            button {
                class: "menu-item",
                role: "menuitem",
                onclick: {
                    let id = id.clone();
                    move |_| {
                        let mut w = dash.write();
                        w.dialog = Some(DashDialog::Inspect(id.clone()));
                        w.menu = None;
                    }
                },
                Ico { icon: Icon::Code, size: 16 }
                span { "Inspect query" }
            }
            button {
                class: "menu-item",
                role: "menuitem",
                title: "Open the panel's query in Explore, with its range and filters",
                onclick: {
                    let query = query.clone();
                    move |_| {
                        dash.write().menu = None;
                        drill_into_explore(ts.explore, &query);
                        nav.push(Route::Timeseries { tab: TimeseriesTab::Explore });
                    }
                },
                Ico { icon: Icon::ExternalLink, size: 16 }
                span { "Open in Explore" }
            }
            button {
                class: "menu-item",
                role: "menuitem",
                onclick: {
                    let id = id.clone();
                    move |_| {
                        let mut w = dash.write();
                        w.menu = None;
                        if let Some(d) = w.open.as_mut()
                            && let Some(new_id) = d.duplicate_panel(&id)
                        {
                            w.notice = Some(Notice::ok(format!("Duplicated the panel as {new_id}.")));
                        }
                    }
                },
                Ico { icon: Icon::Copy, size: 16 }
                span { "Duplicate panel" }
            }
            button {
                class: "menu-item",
                role: "menuitem",
                title: "Copy this panel's rows as CSV",
                onclick: {
                    let (backend, query) = (backend.clone(), query.clone());
                    move |_| {
                        dash.write().menu = None;
                        let (backend, query) = (backend.clone(), query.clone());
                        spawn_forever(async move {
                            let notice = match backend.explore(&query).await {
                                Ok(r) => match copy_text(r.csv(&BTreeSet::new())).await {
                                    Ok(()) => Notice::ok("Copied the panel's rows as CSV."),
                                    Err(e) => Notice::err(e.to_string()),
                                },
                                Err(e) => Notice::err(e.to_string()),
                            };
                            dash.write().notice = Some(notice);
                        });
                    }
                },
                Ico { icon: Icon::Download, size: 16 }
                span { "Copy CSV" }
            }
            div { class: "menu-sep" }
            button {
                class: "menu-item",
                role: "menuitem",
                disabled: first,
                onclick: {
                    let id = id.clone();
                    move |_| {
                        let mut w = dash.write();
                        w.menu = None;
                        if let Some(d) = w.open.as_mut() {
                            d.move_panel(&id, -1);
                        }
                    }
                },
                Ico { icon: Icon::ArrowUp, size: 16 }
                span { "Move up" }
            }
            button {
                class: "menu-item",
                role: "menuitem",
                disabled: last,
                onclick: {
                    let id = id.clone();
                    move |_| {
                        let mut w = dash.write();
                        w.menu = None;
                        if let Some(d) = w.open.as_mut() {
                            d.move_panel(&id, 1);
                        }
                    }
                },
                Ico { icon: Icon::ArrowDown, size: 16 }
                span { "Move down" }
            }
            div { class: "menu-sep" }
            button {
                class: "menu-item danger",
                role: "menuitem",
                title: "Take the panel out of the dashboard. Undo stays until you save.",
                onclick: {
                    let id = id.clone();
                    move |_| remove(&mut dash.write(), &id)
                },
                Ico { icon: Icon::Trash, size: 16 }
                span { "Remove panel" }
            }
        }
    }
}

/// The divider below a panel: drag to set the panel's height, or press Up
/// and Down while it has focus.
#[component]
pub fn PanelDivider(panel_id: String, height: u32) -> Element {
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    let mut drag = use_signal(|| None::<(f64, u32)>);
    let resize = {
        let panel_id = panel_id.clone();
        move |h: u32| {
            if let Some(d) = dash.write().open.as_mut() {
                d.resize_panel(&panel_id, h);
            }
        }
    };

    rsx! {
        div {
            class: "resize-handle vertical ts-divider",
            role: "separator",
            tabindex: "0",
            title: "Drag to resize the panel above",
            "aria-orientation": "horizontal",
            "aria-valuenow": "{height}",
            "aria-valuemin": "{MIN_PANEL_HEIGHT}",
            "aria-valuemax": "{MAX_PANEL_HEIGHT}",
            onmousedown: move |e| {
                e.prevent_default();
                drag.set(Some((e.client_coordinates().y, height)));
            },
            onkeydown: {
                let mut resize = resize.clone();
                move |e| {
                    let next = match e.key() {
                        Key::ArrowDown => height.saturating_add(KEY_STEP),
                        Key::ArrowUp => height.saturating_sub(KEY_STEP),
                        _ => return,
                    };
                    e.prevent_default();
                    resize(next);
                }
            },
        }
        if let Some((from, start)) = drag() {
            div {
                class: "drag-capture vertical",
                onmousemove: {
                    let mut resize = resize.clone();
                    move |e| {
                        let dy = e.client_coordinates().y - from;
                        resize((f64::from(start) + dy).max(0.0) as u32);
                    }
                },
                onmouseup: move |_| drag.set(None),
                onmouseleave: move |_| drag.set(None),
            }
        }
    }
}
