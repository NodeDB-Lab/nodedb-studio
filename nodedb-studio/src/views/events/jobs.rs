//! Scheduled jobs: named SQL that the server runs on a cron schedule.
//!
//! A schedule is not a stream: a job runs, ends, and leaves a run in its
//! history. Run now, Disable, and Cancel run are separate operations. Edits
//! stay in a draft until the review saves them, and a draft survives a switch
//! of the selected job only by an explicit Save, Discard, or Cancel.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::feeds::Tone;
use crate::models::jobs::{JobForm, ScheduledJob};
use crate::routes::tabs::EventsTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::events::EventsUi;
use crate::state::events::jobs::{Editing, JobEditor, JobsUi};
use crate::state::workspace::{Surface, Workspace};
use crate::views::events::common::{NoticeLine, StatusText};
use crate::views::events::job_editor::JobDetails;
use crate::views::events::job_review::ReviewSheet;
use crate::views::events::job_runs::{LogPanel, RunsPanel};

pub const SURFACE: Surface = Surface::Events(EventsTab::ScheduledJobs);
/// The list key of the draft row, which has no id yet.
const DRAFT_KEY: &str = "draft";
const LIST_SIZE: PanelSize = PanelSize::width(360.0, 260.0, 520.0);
const BOTTOM_SIZE: PanelSize = PanelSize::height(220.0, 160.0, 460.0);
const LOG_SIZE: PanelSize = PanelSize::width(460.0, 300.0, 720.0);

/// Jobs whose name contains `filter`, ignoring case.
pub fn matching<'a>(jobs: &'a [ScheduledJob], filter: &str) -> Vec<&'a ScheduledJob> {
    let needle = filter.trim().to_lowercase();
    jobs.iter()
        .filter(|j| needle.is_empty() || j.name.to_lowercase().contains(&needle))
        .collect()
}

/// Start editing a new job. The draft sits in the list until it is created.
pub fn new_job(ui: &mut JobsUi, source: &str, owner: &str) {
    ui.editor = Some(JobEditor {
        target: Editing::New,
        form: JobForm::blank(source, owner),
        tried: false,
        review: false,
        busy: false,
        error: None,
    });
    ui.selected = None;
    ui.switch_to = None;
}

#[component]
pub fn JobsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().jobs;
    let layout = use_context::<Signal<Workspace>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let epoch = use_memo(move || ui.read().epoch);

    let mut jobs = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.scheduled_jobs().await }
        })
    };
    let catalog = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.catalog().await }
        })
    };
    let state = resource_state(&jobs);
    let list = state.loaded().cloned();
    let all: Vec<ScheduledJob> = list.as_ref().map(|l| l.jobs.clone()).unwrap_or_default();
    let now = list.as_ref().map_or(0, |l| l.observed_at);

    let selected_id = use_memo(move || {
        let (chosen, drafting) = {
            let u = ui.read();
            (
                u.selected.clone(),
                matches!(u.editor.as_ref().map(|e| &e.target), Some(Editing::New)),
            )
        };
        if drafting {
            return None;
        }
        let read = jobs.read();
        let list = read.as_ref().and_then(|r| r.as_ref().ok())?;
        chosen
            .filter(|id| list.jobs.iter().any(|j| &j.id == id))
            .or_else(|| list.jobs.first().map(|j| j.id.clone()))
    });
    let selected: Option<ScheduledJob> = selected_id
        .read()
        .as_ref()
        .and_then(|id| all.iter().find(|j| &j.id == id).cloned());

    let (filter, notice, editor, switch_to, log_open) = {
        let u = ui.read();
        (
            u.filter.clone(),
            u.notice.clone(),
            u.editor.clone(),
            u.switch_to.clone(),
            u.log_open,
        )
    };
    let dirty = ui.read().dirty(selected.as_ref());
    let drafting = matches!(editor.as_ref().map(|e| &e.target), Some(Editing::New));
    let rows = matching(&all, &filter);
    let bottom_style = region_style(&layout.read(), SURFACE, "bottom", BOTTOM_SIZE);
    let catalog_names: Vec<String> = catalog
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|c| c.iter().map(|x| x.name.clone()).collect())
        .unwrap_or_default();
    let (database, user) = active
        .read()
        .as_ref()
        .map(|c| (c.current_database.clone(), c.user.clone()))
        .unwrap_or_default();
    let sources: Vec<String> = catalog_names
        .iter()
        .map(|c| format!("{database}.{c}"))
        .collect();
    let review = editor.as_ref().filter(|e| e.review).cloned();

    rsx! {
        div { class: "page-body",
            div { class: "workbench col events-wb",
                div { class: "workbench",
                    Panel {
                        surface: SURFACE,
                        id: "jobs",
                        title: "Jobs",
                        size: LIST_SIZE,
                        div { class: "toolbar nowrap",
                            div { class: "search-field grow",
                                Ico { icon: Icon::Search, size: 16 }
                                input {
                                    class: "input",
                                    placeholder: "Filter jobs…",
                                    "aria-label": "Filter jobs",
                                    value: "{filter}",
                                    oninput: move |e| ui.write().filter = e.value(),
                                }
                            }
                            button {
                                class: "btn primary",
                                title: "Draft a new scheduled job",
                                onclick: {
                                    let (source, user) = (sources.first().cloned().unwrap_or_default(), user.clone());
                                    move |_| new_job(&mut ui.write(), &source, &user)
                                },
                                Ico { icon: Icon::Plus, size: 16 }
                                "New job"
                            }
                        }
                        NoticeLine { notice, on_dismiss: move |_| ui.write().notice = None }
                        AsyncView {
                            loading: state.is_loading(),
                            empty: state.is_empty() && !drafting,
                            error: state.error_message(),
                            retriable: state.is_retriable(),
                            on_retry: move |_| jobs.restart(),
                            empty_message: "This database has no scheduled jobs. Create one with New job.",
                        }
                        if state.loaded().is_some() || drafting {
                            div { class: "dt-wrap",
                                table { class: "dt",
                                    thead {
                                        tr {
                                            th { "Name" }
                                            th { "State" }
                                        }
                                    }
                                    tbody {
                                        if let Some(e) = editor.as_ref().filter(|e| e.target == Editing::New) {
                                            tr { key: "{DRAFT_KEY}", class: "selected", "aria-selected": "true",
                                                td { class: "key",
                                                    if e.form.name.trim().is_empty() { "New job" } else { "{e.form.name.trim()}" }
                                                }
                                                td { StatusText { tone: Tone::Warn, text: "Draft".to_string() } }
                                            }
                                        }
                                        for j in rows.iter() {
                                            JobRow {
                                                key: "{j.id}",
                                                job: (*j).clone(),
                                                selected: selected_id.read().as_deref() == Some(j.id.as_str()),
                                                dirty,
                                            }
                                        }
                                    }
                                }
                                if rows.is_empty() && !drafting {
                                    div { class: "async-empty", "No job matches the filter." }
                                }
                            }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "jobs", size: LIST_SIZE, side: Side::Before }
                    Panel {
                        surface: SURFACE,
                        id: "details",
                        title: "Job details",
                        class: "fill",
                        if selected.is_some() || drafting {
                            JobDetails {
                                job: selected.clone(),
                                editor: editor.clone(),
                                now,
                                sources: sources.clone(),
                                switch_to,
                                jobs: all.clone(),
                            }
                        } else {
                            div { class: "async-empty", "Select a job to see its schedule." }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "bottom", size: BOTTOM_SIZE, side: Side::After }
                div { class: "workbench", style: "{bottom_style}",
                    Panel {
                        surface: SURFACE,
                        id: "runs",
                        title: "Job runs",
                        class: "fill",
                        if let Some(j) = selected.clone() {
                            RunsPanel { job: j }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "log", size: LOG_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "log",
                        title: "Run log",
                        size: LOG_SIZE,
                        actions: rsx! {
                            button {
                                class: "btn sm",
                                title: "Open the run log at full size",
                                disabled: selected.is_none(),
                                onclick: move |_| ui.write().log_open = true,
                                Ico { icon: Icon::ExternalLink, size: 16 }
                                "Open"
                            }
                        },
                        if let Some(j) = selected.clone() {
                            LogPanel { job: j, full: false }
                        }
                    }
                }
                if let Some(r) = review {
                    ReviewSheet { editor: r, job: selected.clone(), jobs: all.clone(), now }
                }
                if log_open {
                    if let Some(j) = selected.clone() {
                        LogPanel { job: j, full: true }
                    }
                }
            }
        }
    }
}

#[component]
fn JobRow(job: ScheduledJob, selected: bool, dirty: bool) -> Element {
    let mut ui = use_context::<EventsUi>().jobs;
    let id = job.id.clone();
    let (tone, word) = if job.enabled {
        (Tone::Ok, "Enabled")
    } else {
        (Tone::Idle, "Disabled")
    };
    rsx! {
        tr {
            class: if selected { "clickable selected" } else { "clickable" },
            "aria-selected": "{selected}",
            tabindex: "0",
            onclick: {
                let id = id.clone();
                move |_| {
                    if selected {
                        return;
                    }
                    let mut w = ui.write();
                    if dirty {
                        // Unsaved edits stay until the person decides.
                        w.switch_to = Some(id.clone());
                    } else {
                        w.selected = Some(id.clone());
                        w.selected_run = None;
                        w.editor = None;
                    }
                }
            },
            onkeydown: move |e| {
                if e.key() == Key::Enter && !selected {
                    let mut w = ui.write();
                    if dirty {
                        w.switch_to = Some(id.clone());
                    } else {
                        w.selected = Some(id.clone());
                        w.selected_run = None;
                        w.editor = None;
                    }
                }
            },
            td { class: "key", "{job.name}" }
            td { StatusText { tone, text: word.to_string() } }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::job_data::JobData;

    #[tokio::test]
    async fn the_filter_matches_names_ignoring_case() {
        let jobs = MockConnectionService::ready()
            .scheduled_jobs()
            .await
            .expect("jobs")
            .jobs;
        assert_eq!(matching(&jobs, "").len(), 2);
        assert_eq!(matching(&jobs, "ROLLUP").len(), 1);
        assert!(matching(&jobs, "nope").is_empty());
    }

    #[test]
    fn a_new_job_starts_a_draft_and_clears_the_selection() {
        let mut ui = JobsUi {
            selected: Some("job_01".into()),
            switch_to: Some("job_02".into()),
            ..JobsUi::default()
        };
        new_job(&mut ui, "analytics.sales", "admin");
        assert_eq!(ui.selected, None);
        assert_eq!(ui.switch_to, None);
        let e = ui.editor.expect("a draft");
        assert_eq!(e.target, Editing::New);
        assert_eq!(e.form.owner, "admin");
        assert_eq!(e.form.source, "analytics.sales");
    }
}
