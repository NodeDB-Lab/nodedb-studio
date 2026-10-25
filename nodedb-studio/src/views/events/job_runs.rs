//! A job's runs and the log of the selected run.

use std::rc::Rc;
use std::time::Duration;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::format::duration;
use crate::models::jobs::{JobRun, LogLine, RunState, ScheduledJob};
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::views::events::common::StatusText;
use crate::views::events::job_tasks;

/// How often a running run is read again.
const POLL: Duration = Duration::from_millis(2000);

/// The run on screen: the chosen one while it is listed, else the newest.
pub fn effective_run<'a>(runs: &'a [JobRun], chosen: Option<&str>) -> Option<&'a JobRun> {
    chosen
        .and_then(|id| runs.iter().find(|r| r.id == id))
        .or_else(|| runs.first())
}

/// `1m 22s`, or an en dash while the run has no duration yet.
pub fn duration_label(secs: Option<u64>) -> String {
    secs.map_or_else(|| "–".to_string(), duration)
}

#[component]
pub fn RunsPanel(job: ScheduledJob) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().jobs;
    let epoch = use_memo(move || ui.read().epoch);
    let id = job.id.clone();
    let mut runs = {
        let backend = backend.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.job_runs(&id).await }
        }))
    };
    let state = resource_state(&runs);
    let list: Vec<JobRun> = state.loaded().cloned().unwrap_or_default();
    let running = use_memo(move || {
        runs.read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .is_some_and(|l| l.iter().any(|r| r.state == RunState::Running))
    });
    // A run in progress is read again until it ends.
    use_resource(move || async move {
        if !running() {
            return;
        }
        loop {
            tokio::time::sleep(POLL).await;
            ui.write().epoch += 1;
        }
    });
    let chosen = ui.read().selected_run.clone();
    let current = effective_run(&list, chosen.as_deref()).cloned();
    let cancellable = current.clone().filter(|r| r.state == RunState::Running);

    rsx! {
        div { class: "toolbar nowrap runs-bar",
            span { class: "muted", "Runs of {job.name}, newest first" }
            div { class: "toolbar-spacer" }
            button {
                class: "btn sm danger",
                disabled: cancellable.is_none(),
                title: match cancellable.as_ref() {
                    Some(r) => format!("Cancel the run of {} started {}. The schedule is unchanged.", job.name, r.started),
                    None => "Select a running run to cancel it".to_string(),
                },
                onclick: {
                    let backend = backend.clone();
                    move |_| {
                        let Some(r) = cancellable.clone() else { return };
                        let backend = backend.clone();
                        spawn_forever(async move { job_tasks::cancel_run(ui, backend, r.id).await });
                    }
                },
                Ico { icon: Icon::Ban, size: 16 }
                "Cancel run"
            }
        }
        AsyncView {
            loading: state.is_loading(),
            empty: state.is_empty(),
            error: state.error_message(),
            retriable: state.is_retriable(),
            on_retry: move |_| runs.restart(),
            empty_message: "This job has not run yet.",
        }
        if !list.is_empty() {
            div { class: "dt-wrap",
                table { class: "dt",
                    thead {
                        tr {
                            th { "Started (UTC)" }
                            th { "State" }
                            th { "Duration" }
                            th { "Message" }
                        }
                    }
                    tbody {
                        for r in list.iter() {
                            {
                                let rid = r.id.clone();
                                let is_selected = current.as_ref().is_some_and(|c| c.id == r.id);
                                rsx! {
                                    tr {
                                        key: "{r.id}",
                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                        "aria-selected": "{is_selected}",
                                        tabindex: "0",
                                        onclick: {
                                            let rid = rid.clone();
                                            move |_| ui.write().selected_run = Some(rid.clone())
                                        },
                                        onkeydown: move |e| {
                                            if e.key() == Key::Enter {
                                                ui.write().selected_run = Some(rid.clone());
                                            }
                                        },
                                        td { class: "mono", "{r.started}" }
                                        td { StatusText { tone: r.state.tone(), text: r.state.label().to_string() } }
                                        td { "{duration_label(r.duration_secs)}" }
                                        td { "{r.message}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The lines of a log, numbered, with their timestamps.
#[component]
fn LogLines(lines: Vec<LogLine>) -> Element {
    rsx! {
        div { class: "run-log", role: "log", "aria-label": "Run log",
            for (i, l) in lines.iter().enumerate() {
                div { key: "{i}", class: "log-line",
                    span { class: "log-no", "{i + 1}" }
                    span { class: "log-at", "{l.at}" }
                    span { class: if l.text.starts_with("ERROR") { "log-text err" } else { "log-text" }, "{l.text}" }
                }
            }
        }
    }
}

/// The log of the selected run, in the panel or, with `full`, in a sheet.
#[component]
pub fn LogPanel(job: ScheduledJob, full: bool) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().jobs;
    let copier = use_copier();
    let epoch = use_memo(move || ui.read().epoch);
    let id = job.id.clone();
    let runs = {
        let backend = backend.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.job_runs(&id).await }
        }))
    };
    let list: Vec<JobRun> = runs
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let chosen = ui.read().selected_run.clone();
    let current = effective_run(&list, chosen.as_deref()).cloned();
    let run_id = current.as_ref().map(|r| r.id.clone());
    let log = {
        let backend = backend.clone();
        use_resource(use_reactive!(|run_id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move {
                match run_id {
                    Some(id) => backend.job_log(&id).await,
                    None => Ok(Vec::new()),
                }
            }
        }))
    };
    let state = resource_state(&log);
    let lines: Vec<LogLine> = state.loaded().cloned().unwrap_or_default();
    let text = lines
        .iter()
        .map(|l| format!("{} {}", l.at, l.text))
        .collect::<Vec<_>>()
        .join("\n");
    let body = rsx! {
        AsyncView {
            loading: state.is_loading(),
            empty: current.is_some() && state.is_empty(),
            error: state.error_message(),
            retriable: false,
            empty_message: "This run wrote no log lines.",
        }
        if current.is_none() {
            div { class: "async-empty", "Select a run to read its log." }
        }
        if !lines.is_empty() {
            LogLines { lines: lines.clone() }
        }
    };
    if !full {
        return body;
    }
    let title = match current.as_ref() {
        Some(r) => format!("Run log — {} · {}", job.name, r.started),
        None => format!("Run log — {}", job.name),
    };
    rsx! {
        Sheet {
            title,
            width: SheetWidth::Medium,
            class: "log-sheet",
            on_close: move |_| ui.write().log_open = false,
            footer: rsx! {
                SheetFooter { error: None, hint: Some(format!("{} lines", lines.len())),
                    button {
                        class: "btn",
                        disabled: lines.is_empty(),
                        onclick: move |_| copier.copy(text.clone()),
                        Ico { icon: Icon::Copy, size: 16 }
                        "Copy log"
                    }
                    button { class: "btn", onclick: move |_| ui.write().log_open = false, "Close" }
                }
            },
            {body}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::job_data::JobData;

    #[tokio::test]
    async fn the_newest_run_shows_until_one_is_chosen() {
        let runs = MockConnectionService::ready()
            .job_runs("job_01")
            .await
            .expect("runs");
        assert_eq!(
            effective_run(&runs, None).map(|r| r.id.as_str()),
            Some("job_01_run5")
        );
        assert_eq!(
            effective_run(&runs, Some("job_01_run4")).map(|r| r.id.as_str()),
            Some("job_01_run4")
        );
        // A chosen run that is gone falls back, never shows stale.
        assert_eq!(
            effective_run(&runs, Some("job_99_run1")).map(|r| r.id.as_str()),
            Some("job_01_run5")
        );
        assert_eq!(effective_run(&[], None), None);
    }

    #[test]
    fn a_run_without_a_duration_shows_a_dash() {
        assert_eq!(duration_label(Some(82)), "1m 22s");
        assert_eq!(duration_label(None), "–");
    }
}
