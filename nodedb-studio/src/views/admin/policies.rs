//! Administration → Row policies: the policies of one database, the editor
//! of the chosen one, and a read-only test of it.
//!
//! Three panels. A draft belongs to its policy, so choosing another policy
//! keeps it, and a row with unsaved edits shows a dot. Review changes reads
//! what the edit would do, and applies it only from the review sheet.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::admin_policy::{Policy, PolicyDraft, PolicySet};
use crate::routes::tabs::AdminTab;
use crate::services::backend::Backend;
use crate::state::admin::{AdminUi, NEW_POLICY, PolicyUi, pick};
use crate::state::connection::ActiveConnection;
use crate::state::workspace::Surface;
use crate::views::admin::common::{Cut, ReviewSheet};
use crate::views::admin::policy_editor::PolicyEditor;
use crate::views::admin::policy_test::PolicyTestPanel;
use crate::views::admin::tasks;
use crate::views::cluster::common::Ctl;
use crate::views::events::common::NoticeLine;

const SURFACE: Surface = Surface::Admin(AdminTab::RowPolicies);
const LIST_SIZE: PanelSize = PanelSize::width(450.0, 320.0, 640.0);
const TEST_SIZE: PanelSize = PanelSize::width(360.0, 300.0, 560.0);

/// What the editor edits: the stored policy with its draft laid over it, or
/// the policy being created.
#[derive(Debug, Clone, PartialEq)]
pub struct Editing {
    /// The stored policy. `None` while a new one is created.
    pub stored: Option<Policy>,
    pub draft: PolicyDraft,
}

impl Editing {
    pub fn dirty(&self) -> bool {
        match &self.stored {
            Some(p) => self.draft.differs_from(p),
            None => true,
        }
    }
}

/// The policy the editor shows, given the selection and the drafts.
pub fn editing(set: &PolicySet, u: &PolicyUi) -> Option<Editing> {
    if u.selected.as_deref() == Some(NEW_POLICY)
        && let Some(d) = u.creating.clone()
    {
        return Some(Editing {
            stored: None,
            draft: d,
        });
    }
    let p = pick(&set.policies, u.selected.as_deref(), |p| &p.id)?;
    let draft = u.draft_of(&p.id).cloned().unwrap_or_else(|| p.draft());
    Some(Editing {
        stored: Some(p.clone()),
        draft,
    })
}

#[component]
pub fn PoliciesTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut ui = use_context::<AdminUi>().policies;
    let epoch = use_memo(move || ui.read().epoch);
    let database = use_memo(move || {
        ui.read()
            .database
            .clone()
            .or_else(|| active.read().as_ref().map(|c| c.current_database.clone()))
    });

    let mut policies = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch();
            let db = database();
            async move {
                match db {
                    Some(db) => backend.policy_set(&db).await,
                    None => Err(crate::services::error::StudioError::NotConnected),
                }
            }
        }
    });
    let state = resource_state(&policies);
    let set = state.loaded().cloned();
    let u = ui.read().clone();
    let needle = u.filter.trim().to_lowercase();
    let current = set.as_ref().and_then(|s| editing(s, &u));
    let selected_id = current.as_ref().map(|e| match &e.stored {
        Some(p) => p.id.clone(),
        None => NEW_POLICY.to_string(),
    });

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                Panel {
                    surface: SURFACE,
                    id: "policies",
                    title: "Row policies",
                    size: LIST_SIZE,
                    NoticeLine {
                        notice: u.notice.clone(),
                        on_dismiss: move |_| ui.write().notice = None,
                    }
                    div { class: "toolbar nowrap cl-filters",
                        Ctl { label: "Database", id: "pol-db",
                            select {
                                id: "pol-db",
                                class: "select native",
                                onchange: move |e| ui.write().database = Some(e.value()),
                                for d in set.as_ref().map(|s| s.databases.clone()).unwrap_or_default() {
                                    option {
                                        key: "{d}",
                                        value: "{d}",
                                        selected: Some(d.as_str()) == database().as_deref(),
                                        "{d}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "toolbar nowrap cl-filters",
                        div { class: "search-field grow",
                            Ico { icon: Icon::Search, size: 16 }
                            input {
                                class: "input",
                                placeholder: "Filter policies…",
                                "aria-label": "Filter policies",
                                value: "{u.filter}",
                                oninput: move |e| ui.write().filter = e.value(),
                            }
                        }
                        button {
                            class: "btn primary",
                            disabled: set.is_none(),
                            title: "Create a row policy on a collection",
                            onclick: {
                                let set = set.clone();
                                move |_| {
                                    let Some(s) = set.as_ref() else { return };
                                    let blank = PolicyDraft::blank(
                                        &s.database,
                                        s.collections.first().map(String::as_str).unwrap_or(""),
                                        s.roles.first().map(String::as_str).unwrap_or(""),
                                    );
                                    let mut w = ui.write();
                                    w.creating = Some(blank);
                                    w.selected = Some(NEW_POLICY.to_string());
                                }
                            },
                            Ico { icon: Icon::Plus, size: 16 }
                            "Policy"
                        }
                    }
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty() && u.creating.is_none(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| policies.restart(),
                        empty_message: "This database has no row policy. Every role sees the rows its privileges allow.",
                    }
                    if set.is_some() || u.creating.is_some() {
                        div { class: "dt-wrap",
                            table { class: "dt cl-table ad-table ad-policies",
                                thead {
                                    tr {
                                        th { "Policy name" }
                                        th { "Collection" }
                                        th { "Operation" }
                                    }
                                }
                                tbody {
                                    if let Some(d) = u.creating.as_ref() {
                                        tr {
                                            key: "{NEW_POLICY}",
                                            class: if selected_id.as_deref() == Some(NEW_POLICY) { "clickable selected" } else { "clickable" },
                                            "aria-selected": "{selected_id.as_deref() == Some(NEW_POLICY)}",
                                            tabindex: "0",
                                            onclick: move |_| ui.write().selected = Some(NEW_POLICY.to_string()),
                                            td { class: "key",
                                                span { class: "ad-name",
                                                    span { class: "unsaved-dot", title: "Not created yet" }
                                                    Cut { text: if d.name.trim().is_empty() { "New policy".to_string() } else { d.name.trim().to_string() } }
                                                }
                                            }
                                            td { Cut { text: d.collection.clone() } }
                                            td { "{d.operation.label()}" }
                                        }
                                    }
                                    for p in set.iter().flat_map(|s| s.policies.iter()).filter(|p| p.matches(&needle)) {
                                        {
                                            let id = p.id.clone();
                                            let is_selected = selected_id.as_deref() == Some(p.id.as_str());
                                            let dirty = u.draft_of(&p.id).is_some_and(|d| d.differs_from(p));
                                            rsx! {
                                                tr {
                                                    key: "{p.id}",
                                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                                    "aria-selected": "{is_selected}",
                                                    tabindex: "0",
                                                    onclick: move |_| ui.write().selected = Some(id.clone()),
                                                    td { class: "key",
                                                        span { class: "ad-name",
                                                            if dirty {
                                                                span { class: "unsaved-dot", title: "Unsaved edits" }
                                                            }
                                                            Cut { text: p.name.clone() }
                                                            if !p.enabled {
                                                                span { class: "badge gray", "Disabled" }
                                                            }
                                                        }
                                                    }
                                                    td { Cut { text: p.collection.clone() } }
                                                    td { Cut { text: p.operation.label().to_string() } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "dt-footer",
                            "{set.as_ref().map(|s| s.policies.len()).unwrap_or(0)} policies"
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "policies", size: LIST_SIZE, side: Side::Before }
                Panel {
                    surface: SURFACE,
                    id: "editor",
                    title: "Policy editor",
                    class: "fill",
                    if let (Some(e), Some(s)) = (current.clone(), set.clone()) {
                        PolicyEditor { editing: e, set: s }
                    } else if !state.is_loading() {
                        div { class: "async-empty", "Choose a policy to edit it." }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "test", size: TEST_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "test",
                    title: "Test policy",
                    size: TEST_SIZE,
                    if let (Some(e), Some(s)) = (current.clone(), set.clone()) {
                        PolicyTestPanel { editing: e, set: s }
                    }
                }
                if let Some(state) = u.review.clone() {
                    ReviewSheet {
                        state,
                        back_label: "Back to editor",
                        on_back: move |_| ui.write().review = None,
                        on_apply: {
                            let backend = backend.clone();
                            move |_| {
                                let backend = backend.clone();
                                spawn_forever(async move { tasks::apply_review(ui, backend).await });
                            }
                        },
                    }
                }
            }
        }
    }
}
