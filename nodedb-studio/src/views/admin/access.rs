//! Administration → Access: principals and roles, what the chosen one
//! holds, and the access it has in effect.
//!
//! Five panels. Principals (or Roles) and the Role grants and Membership
//! below it sit on the left. The chosen subject's details and Effective
//! access sit on the right. The selection lives in `AdminUi`, above the
//! panels, so hiding a panel through the View menu keeps it. Edits stage
//! first. Review changes reads their preview and applies them in one step.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::admin_access::{AccessDirectory, AccessSubject};
use crate::routes::tabs::AdminTab;
use crate::services::backend::Backend;
use crate::state::admin::{AccessMode, AccessUi, AdminUi, pick};
use crate::state::workspace::{Surface, Workspace};
use crate::views::admin::access_details::{PrincipalDetails, RoleDetails};
use crate::views::admin::access_list::{PrincipalsList, RolesList};
use crate::views::admin::access_panels::{EffectiveBody, GrantsBody, MembershipBody};
use crate::views::admin::access_sheets::AccessSheets;
use crate::views::events::common::NoticeLine;

const SURFACE: Surface = Surface::Admin(AdminTab::Access);
const LOWER_SIZE: PanelSize = PanelSize::height(290.0, 180.0, 520.0);
const MEMBER_SIZE: PanelSize = PanelSize::width(320.0, 240.0, 560.0);
const SIDE_SIZE: PanelSize = PanelSize::width(360.0, 320.0, 720.0);
const EFFECTIVE_SIZE: PanelSize = PanelSize::height(210.0, 140.0, 440.0);

/// What the subject panels show: the chosen principal or role.
fn subject_of(dir: &AccessDirectory, u: &AccessUi) -> Option<AccessSubject> {
    match u.mode {
        AccessMode::Principals => pick(&dir.principals, u.principal.as_deref(), |p| &p.id)
            .map(|p| AccessSubject::Principal(p.id.clone())),
        AccessMode::Roles => pick(&dir.roles, u.role.as_deref(), |r| &r.name)
            .map(|r| AccessSubject::Role(r.name.clone())),
    }
}

#[component]
pub fn AccessTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<AdminUi>().access;
    let layout = use_context::<Signal<Workspace>>();
    let epoch = use_memo(move || ui.read().epoch);

    let mut directory = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            // Principals belong to the connection: a switch re-reads them.
            let _conn = conn();
            let _epoch = epoch();
            async move { backend.access_directory().await }
        }
    });
    let dstate = resource_state(&directory);
    let dir = dstate.loaded().cloned();

    // The chosen subject while it is listed, else the first. A memo, so the
    // access read runs again only when the choice or the data changes.
    let subject = use_memo(move || {
        let read = directory.read();
        let dir = read.as_ref().and_then(|r| r.as_ref().ok())?;
        subject_of(dir, &ui.read())
    });
    let access = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch();
            let subject = subject();
            async move {
                match subject {
                    Some(s) => backend.subject_access(&s).await.map(Some),
                    None => Ok(None),
                }
            }
        }
    });
    let astate = resource_state(&access);
    let detail = astate.loaded().cloned().flatten();

    let u = ui.read().clone();
    let roles_mode = u.mode == AccessMode::Roles;
    let title = u.mode.label();
    let (lower_visible, side_visible) = {
        let l = layout.read();
        (
            l.is_visible(SURFACE, "grants") || l.is_visible(SURFACE, "membership"),
            l.is_visible(SURFACE, "details") || l.is_visible(SURFACE, "effective"),
        )
    };
    let lower_style = region_style(&layout.read(), SURFACE, "lower", LOWER_SIZE);
    let side_style = region_style(&layout.read(), SURFACE, "side", SIDE_SIZE);
    let chosen_principal = dir
        .as_ref()
        .and_then(|d| pick(&d.principals, u.principal.as_deref(), |p| &p.id).cloned());
    let chosen_role = dir
        .as_ref()
        .and_then(|d| pick(&d.roles, u.role.as_deref(), |r| &r.name).cloned());

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                div { class: "workbench col",
                    Panel {
                        surface: SURFACE,
                        id: "principals",
                        title: title.to_string(),
                        class: "fill",
                        actions: rsx! {
                            div { class: "seg", role: "tablist",
                                for m in AccessMode::ALL {
                                    button {
                                        key: "{m.label()}",
                                        role: "tab",
                                        class: if m == u.mode { "active" } else { "" },
                                        "aria-selected": "{m == u.mode}",
                                        onclick: move |_| ui.write().mode = m,
                                        "{m.label()}"
                                    }
                                }
                            }
                        },
                        NoticeLine {
                            notice: u.notice.clone(),
                            on_dismiss: move |_| ui.write().notice = None,
                        }
                        AsyncView {
                            loading: dstate.is_loading(),
                            empty: dstate.is_empty(),
                            error: dstate.error_message(),
                            retriable: dstate.is_retriable(),
                            on_retry: move |_| directory.restart(),
                            empty_message: "This cluster lists no principals.",
                        }
                        if let Some(d) = dir.clone() {
                            if roles_mode {
                                RolesList { dir: d, chosen: chosen_role.clone().map(|r| r.name) }
                            } else {
                                PrincipalsList { dir: d, chosen: chosen_principal.clone().map(|p| p.id) }
                            }
                        }
                    }
                    if lower_visible {
                        ResizeHandle { surface: SURFACE, target: "lower", size: LOWER_SIZE, side: Side::After }
                        div { class: "workbench ad-lower", style: "{lower_style}",
                            Panel {
                                surface: SURFACE,
                                id: "grants",
                                title: "Role grants",
                                class: "fill",
                                GrantsBody { access: detail.clone(), loading: astate.is_loading() }
                            }
                            ResizeHandle { surface: SURFACE, target: "membership", size: MEMBER_SIZE, side: Side::After }
                            Panel {
                                surface: SURFACE,
                                id: "membership",
                                title: if roles_mode { "Members" } else { "Membership" },
                                size: MEMBER_SIZE,
                                if let Some(d) = dir.clone() {
                                    MembershipBody { dir: d, access: detail.clone(), loading: astate.is_loading() }
                                }
                            }
                        }
                    }
                }
                if side_visible {
                    ResizeHandle { surface: SURFACE, target: "side", size: SIDE_SIZE, side: Side::After }
                    div { class: "workbench col ad-side", style: "{side_style}",
                        Panel {
                            surface: SURFACE,
                            id: "details",
                            title: if roles_mode { "Role details" } else { "Principal details" },
                            class: "fill",
                            if let Some(d) = dir.clone() {
                                if roles_mode {
                                    if let Some(r) = chosen_role.clone() {
                                        RoleDetails { dir: d, role: r, access: detail.clone() }
                                    }
                                } else if let Some(p) = chosen_principal.clone() {
                                    PrincipalDetails { dir: d, principal: p }
                                }
                            }
                        }
                        ResizeHandle { surface: SURFACE, target: "effective", size: EFFECTIVE_SIZE, side: Side::After }
                        Panel {
                            surface: SURFACE,
                            id: "effective",
                            title: "Effective access",
                            size: EFFECTIVE_SIZE,
                            EffectiveBody { access: detail.clone(), loading: astate.is_loading(), error: astate.error_message() }
                        }
                    }
                }
                if let Some(d) = dir.clone() {
                    AccessSheets { dir: d }
                }
            }
        }
    }
}
