//! Import and export jobs: the Background jobs panel and the History tab.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::{date_minute, offset_label};
use crate::models::transfer::{JobKind, JobState, TransferJob};

/// Jobs that run, or started within a day of the newest job.
pub fn recent(jobs: &[TransferJob]) -> Vec<&TransferJob> {
    let newest = jobs.iter().map(|j| j.started_at).max().unwrap_or(0);
    jobs.iter()
        .filter(|j| j.running() || newest - j.started_at < 86_400)
        .collect()
}

fn status(state: &JobState) -> Element {
    match state {
        JobState::Running { percent } => rsx! {
            span { class: "job-status",
                span { class: "status", span { class: "status-dot info" } "Running" }
                span { class: "progress", role: "progressbar", "aria-valuenow": "{percent}", "aria-valuemin": "0", "aria-valuemax": "100",
                    span { class: "progress-track",
                        span { class: "progress-bar", style: "width: {percent}%;" }
                    }
                    span { class: "progress-text", "{percent}%" }
                }
            }
        },
        JobState::Completed => {
            rsx! { span { class: "status", span { class: "status-dot ok" } "Completed" } }
        }
        JobState::Cancelled => {
            rsx! { span { class: "status", span { class: "status-dot idle" } "Cancelled" } }
        }
        JobState::Failed { .. } => {
            rsx! { span { class: "status", span { class: "status-dot err" } "Failed" } }
        }
    }
}

/// The action of one job row.
#[component]
fn JobAction(job: TransferJob, on_cancel: EventHandler<String>) -> Element {
    match (&job.state, job.kind) {
        (JobState::Running { .. }, _) => {
            let id = job.id.clone();
            rsx! { button { class: "btn sm", onclick: move |_| on_cancel.call(id.clone()), "Cancel" } }
        }
        (JobState::Completed, JobKind::Export) => rsx! {
            button {
                class: "link-btn",
                disabled: true,
                title: "Saving a file needs the native file dialog, which Studio does not ship yet",
                "Download"
            }
        },
        (JobState::Failed { reason }, _) => {
            rsx! { span { class: "job-reason", title: "{reason}", "{reason}" } }
        }
        (JobState::Completed, JobKind::Import) | (JobState::Cancelled, _) => rsx! {},
    }
}

#[component]
pub fn JobsTable(jobs: Vec<TransferJob>, offset: i16, on_cancel: EventHandler<String>) -> Element {
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt jobs-table",
                thead {
                    tr {
                        th { "Name" }
                        th { "Type" }
                        th { "Status" }
                        th { "Started ({offset_label(offset)})" }
                        th { "Action" }
                    }
                }
                tbody {
                    for job in jobs {
                        tr { key: "{job.id}",
                            td { class: "key", "{job.name}" }
                            td { "{job.kind.label()}" }
                            td { {status(&job.state)} }
                            td { "{date_minute(job.started_at, offset)}" }
                            td { JobAction { job: job.clone(), on_cancel } }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn HistoryTable(jobs: Vec<TransferJob>, offset: i16) -> Element {
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt jobs-table",
                thead {
                    tr {
                        th { "Name" }
                        th { "Type" }
                        th { "Outcome" }
                        th { "Started ({offset_label(offset)})" }
                    }
                }
                tbody {
                    for job in jobs.iter() {
                        tr { key: "{job.id}",
                            td { class: "key", "{job.name}" }
                            td { "{job.kind.label()}" }
                            td {
                                {status(&job.state)}
                                if let JobState::Failed { reason } = &job.state {
                                    div { class: "job-reason", "{reason}" }
                                }
                            }
                            td { "{date_minute(job.started_at, offset)}" }
                        }
                    }
                }
            }
            if jobs.is_empty() {
                div { class: "empty-state",
                    Ico { icon: Icon::History, size: 32 }
                    h3 { "No transfers yet" }
                    p { "Imports and exports of this database appear here." }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::transfer_data::TransferData;

    #[tokio::test]
    async fn background_jobs_are_the_running_and_the_last_day() {
        let jobs = MockConnectionService::ready()
            .transfer_jobs()
            .await
            .expect("jobs");
        let names: Vec<&str> = recent(&jobs).iter().map(|j| j.name.as_str()).collect();
        assert_eq!(names, ["users.csv", "orders.parquet"]);
    }

    #[tokio::test]
    async fn a_running_job_stays_listed_however_old() {
        let mut jobs = MockConnectionService::ready()
            .transfer_jobs()
            .await
            .expect("jobs");
        jobs[1].started_at -= 10 * 86_400;
        assert!(recent(&jobs).iter().any(|j| j.name == "orders.parquet"));
    }
}
