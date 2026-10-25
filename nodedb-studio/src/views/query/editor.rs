//! The editor frame: tabs, toolbar, parameters, and the code.
//!
//! The frame is a panel without a close button. The statement is the page's
//! subject, so no View entry hides it. It resizes against Results.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::code_editor::CodeEditor;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{PanelSize, region_style};
use crate::models::console::Protocol;
use crate::models::saved_query::QueryLibrary;
use crate::models::workbench::TxState;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::query::{QueryWorkspace, TxEpoch};
use crate::state::workspace::{Surface, Workspace};
use crate::views::query::params::ParamsRow;
use crate::views::query::tabs::TabStrip;
use crate::views::query::tasks::{self, Env};
use crate::views::query::toolbar::Toolbar;

pub const SURFACE: Surface = Surface::Query;
pub const EDITOR_SIZE: PanelSize = PanelSize::height(330.0, 180.0, 720.0);

#[component]
pub fn EditorPanel(
    databases: Vec<String>,
    tx: Option<TxState>,
    library: AsyncState<QueryLibrary>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let tx_epoch = use_context::<TxEpoch>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let layout = use_context::<Signal<Workspace>>();

    // With Results hidden the editor takes the whole column.
    let style = {
        let l = layout.read();
        if l.is_visible(SURFACE, "results") {
            region_style(&l, SURFACE, "editor", EDITOR_SIZE)
        } else {
            "flex: 1 1 0;".to_string()
        }
    };
    let (sql, notice, has_tab) = {
        let w = ws.read();
        (
            w.active().map(|t| t.sql.clone()).unwrap_or_default(),
            w.notice.clone(),
            w.active().is_some(),
        )
    };
    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let env = Env::new(ws, tx_epoch);

    rsx! {
        section { class: "panel query-editor", style: "{style}", "aria-label": "Query editor",
            header { class: "panel-header editor-header",
                span { class: "panel-grip", title: "Move panel", "aria-hidden": "true",
                    Ico { icon: Icon::GripVertical, size: 16 }
                }
                TabStrip {}
            }
            Toolbar { databases, tx, library }
            if let Some(msg) = notice {
                div { class: "callout err editor-notice", role: "alert",
                    Ico { icon: Icon::CircleAlert, size: 16 }
                    span { class: "grow", "{msg}" }
                    button {
                        class: "icon-btn sm",
                        "aria-label": "Dismiss",
                        onclick: move |_| ws.write().notice = None,
                        Ico { icon: Icon::X, size: 14 }
                    }
                }
            }
            if has_tab {
                ParamsRow {}
                CodeEditor {
                    class: "query-code",
                    value: sql,
                    protocol: Protocol::Sql,
                    label: "SQL statement",
                    placeholder: "Type a statement, then press Ctrl+Enter to run it",
                    on_input: move |text: String| {
                        if let Some(t) = ws.write().active_mut() { t.set_sql(text); }
                    },
                    on_run: move |_| tasks::start_run(env, backend.clone(), &database, 0),
                }
            } else {
                div { class: "empty-state",
                    Ico { icon: Icon::FileCode, size: 28 }
                    h3 { "No query open" }
                    p { "Open a saved query from the library, or start a new one." }
                    div { class: "actions",
                        button {
                            class: "btn primary",
                            onclick: move |_| {
                                let db = active.peek().as_ref().map(|c| c.current_database.clone()).unwrap_or_default();
                                ws.write().new_tab(&db);
                            },
                            Ico { icon: Icon::Plus, size: 16 }
                            "New query"
                        }
                    }
                }
            }
        }
    }
}
