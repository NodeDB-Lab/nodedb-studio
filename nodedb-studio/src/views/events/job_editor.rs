//! Job details: the schedule, the SQL, and the policy of the selected job,
//! as an editor. Edits stay in a draft until Review changes saves them.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::code_editor::CodeEditor;
use crate::components::icon::{Ico, Icon};
use crate::models::console::Protocol;
use crate::models::feeds::Tone;
use crate::models::format::date_second;
use crate::models::jobs::{
    BACKOFFS, JobField, JobForm, JobIssue, OverlapPolicy, ScheduledJob, TIMEOUTS, secs_label,
};
use crate::models::zone::Zone;
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::jobs::{Editing, JobEditor, JobsUi};
use crate::views::events::common::{FormField, StatusBadge};
use crate::views::events::job_tasks;

/// Apply `change` to the form, starting an edit of `job` first when none is
/// open.
fn edit(mut ui: Signal<JobsUi>, job: Option<&ScheduledJob>, change: impl FnOnce(&mut JobForm)) {
    let mut w = ui.write();
    if w.editor.is_none() {
        let Some(job) = job else { return };
        w.editor = Some(JobEditor {
            target: Editing::Job {
                id: job.id.clone(),
                revision: job.revision,
            },
            form: JobForm::of(job),
            tried: false,
            review: false,
            busy: false,
            error: None,
        });
    }
    if let Some(e) = w.editor.as_mut() {
        change(&mut e.form);
        e.error = None;
    }
}

fn issue_at(issues: &[JobIssue], at: JobField, show: bool) -> Option<String> {
    show.then(|| {
        issues
            .iter()
            .find(|i| i.at == at)
            .map(|i| i.message.clone())
    })
    .flatten()
}

fn cls(error: &Option<String>, base: &str) -> String {
    if error.is_some() {
        format!("{base} invalid")
    } else {
        base.to_string()
    }
}

/// A select over `values` that also offers `current` when it is not listed.
fn secs_options(values: &[u32], current: u32) -> Vec<u32> {
    let mut v = values.to_vec();
    if !v.contains(&current) {
        v.push(current);
        v.sort_unstable();
    }
    v
}

#[component]
pub fn JobDetails(
    job: Option<ScheduledJob>,
    editor: Option<JobEditor>,
    now: i64,
    sources: Vec<String>,
    switch_to: Option<String>,
    jobs: Vec<ScheduledJob>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<EventsUi>().jobs;
    let drafting = matches!(editor.as_ref().map(|e| &e.target), Some(Editing::New));
    let form = match (&editor, &job) {
        (Some(e), _) => e.form.clone(),
        (None, Some(j)) => JobForm::of(j),
        (None, None) => return rsx! {},
    };
    let dirty = ui.read().dirty(job.as_ref());
    let tried = editor.as_ref().is_some_and(|e| e.tried);
    let issues = form.issues(&jobs, job.as_ref().map(|j| j.id.as_str()));
    let name = form.name.trim().to_string();
    let next = form.next_runs(now, 3);
    let schedule_error = issue_at(&issues, JobField::Schedule, true);
    let zone_error = issue_at(&issues, JobField::Timezone, true);
    let owner_error = issue_at(&issues, JobField::Owner, tried);
    let retries_error = issue_at(&issues, JobField::Retries, true);
    let name_error = issue_at(&issues, JobField::Name, tried || !form.name.is_empty());
    let sql_error = issue_at(&issues, JobField::Sql, tried);
    let enabled = job.as_ref().is_some_and(|j| j.enabled);
    let job_for_edit = job.clone();
    let timeouts = secs_options(&TIMEOUTS, form.timeout_secs);
    let backoffs = secs_options(&BACKOFFS, form.backoff_secs);
    let switch_name = switch_to
        .as_ref()
        .and_then(|id| jobs.iter().find(|j| &j.id == id))
        .map(|j| j.name.clone());

    let review = move |_| {
        let mut w = ui.write();
        if let Some(e) = w.editor.as_mut() {
            e.tried = true;
            if issues_free(&e.form, &jobs, job.as_ref()) {
                e.review = true;
            }
        }
    };

    rsx! {
        div { class: "job-details",
            if let Some(target) = switch_name {
                div { class: "callout warn switch-bar", role: "alertdialog", "aria-label": "Unsaved changes",
                    Ico { icon: Icon::TriangleAlert, size: 16 }
                    div { class: "stack",
                        strong { "{name} has unsaved changes." }
                        span { "Switching to {target} needs you to save or discard them." }
                        div { class: "row",
                            button {
                                class: "btn sm primary",
                                onclick: move |_| {
                                    let mut w = ui.write();
                                    w.switch_to = None;
                                    if let Some(e) = w.editor.as_mut() { e.tried = true; }
                                },
                                "Keep editing to review and save"
                            }
                            button {
                                class: "btn sm danger",
                                onclick: move |_| {
                                    let mut w = ui.write();
                                    if let Some(id) = w.switch_to.take() {
                                        w.selected = Some(id);
                                        w.selected_run = None;
                                        w.editor = None;
                                    }
                                },
                                "Discard changes"
                            }
                            button { class: "btn sm", onclick: move |_| ui.write().switch_to = None, "Cancel" }
                        }
                    }
                }
            }
            div { class: "job-head",
                if drafting {
                    div { class: "field name-field",
                        label { r#for: "jd-name", "Job name" }
                        input {
                            id: "jd-name",
                            class: cls(&name_error, "input mono"),
                            value: "{form.name}",
                            placeholder: "hourly_sales_check",
                            spellcheck: "false",
                            "aria-invalid": "{name_error.is_some()}",
                            oninput: {
                                let j = job_for_edit.clone();
                                move |e| edit(ui, j.as_ref(), |f| f.name = e.value())
                            },
                        }
                        if let Some(m) = name_error.clone() {
                            div { class: "error", role: "alert", "{m}" }
                        }
                    }
                    StatusBadge { tone: Tone::Warn, text: "Draft".to_string() }
                } else {
                    h3 { class: "job-name", "{name}" }
                    StatusBadge {
                        tone: if enabled { Tone::Ok } else { Tone::Idle },
                        text: if enabled { "Enabled".to_string() } else { "Disabled".to_string() },
                    }
                }
                div { class: "toolbar-spacer" }
                button {
                    class: "btn",
                    disabled: drafting,
                    title: if drafting { "Create the job first".to_string() } else if dirty { format!("Run the saved definition of {name} now. Your unsaved edits are not used.") } else { format!("Run {name} now, outside its schedule") },
                    onclick: {
                        let (backend, id, name) = (backend.clone(), job_for_edit.as_ref().map(|j| j.id.clone()), name.clone());
                        move |_| {
                            let Some(id) = id.clone() else { return };
                            let (backend, name) = (backend.clone(), name.clone());
                            spawn_forever(async move { job_tasks::run_now(ui, backend, id, name).await });
                        }
                    },
                    Ico { icon: Icon::Play, size: 16 }
                    "Run now"
                }
                button {
                    class: "btn",
                    disabled: drafting,
                    title: if drafting { "Create the job first".to_string() } else if enabled { format!("Disable the schedule of {name}. Its history stays.") } else { format!("Enable the schedule of {name}") },
                    onclick: {
                        let (backend, id) = (backend.clone(), job_for_edit.as_ref().map(|j| j.id.clone()));
                        move |_| {
                            let Some(id) = id.clone() else { return };
                            let backend = backend.clone();
                            spawn_forever(async move { job_tasks::set_enabled(ui, backend, id, !enabled).await });
                        }
                    },
                    if enabled {
                        Ico { icon: Icon::Ban, size: 16 }
                        "Disable"
                    } else {
                        Ico { icon: Icon::Play, size: 16 }
                        "Enable"
                    }
                }
                button {
                    class: "btn primary",
                    disabled: !dirty,
                    title: if dirty { if drafting { "Review the new job before creating it" } else { "Review your changes before saving them" } } else { "No changes to review" },
                    onclick: review,
                    if drafting { "Review new job" } else { "Review changes" }
                }
            }
            div { class: "job-form",
                FormField { label: "Schedule (cron)", id: "jd-cron", error: schedule_error.clone(),
                    input {
                        id: "jd-cron",
                        class: cls(&schedule_error, "input mono"),
                        value: "{form.cron}",
                        spellcheck: "false",
                        "aria-invalid": "{schedule_error.is_some()}",
                        oninput: {
                            let j = job_for_edit.clone();
                            move |e| edit(ui, j.as_ref(), |f| f.cron = e.value())
                        },
                    }
                }
                FormField { label: "Source", id: "jd-source",
                    select {
                        id: "jd-source",
                        class: "select native",
                        onchange: {
                            let j = job_for_edit.clone();
                            move |e| edit(ui, j.as_ref(), |f| f.source = e.value())
                        },
                        for s in sources.iter() {
                            option { key: "{s}", value: "{s}", selected: *s == form.source, "{s}" }
                        }
                        if !sources.contains(&form.source) {
                            option { value: "{form.source}", selected: true, "{form.source}" }
                        }
                    }
                }
                FormField { label: "Overlap policy", id: "jd-overlap",
                    select {
                        id: "jd-overlap",
                        class: "select native",
                        title: "{form.overlap.hint()}",
                        onchange: {
                            let j = job_for_edit.clone();
                            move |e| {
                                let p = OverlapPolicy::ALL.into_iter().find(|p| p.label() == e.value()).unwrap_or(OverlapPolicy::Skip);
                                edit(ui, j.as_ref(), |f| f.overlap = p);
                            }
                        },
                        for p in OverlapPolicy::ALL {
                            option { key: "{p.label()}", value: "{p.label()}", selected: form.overlap == p, title: "{p.hint()}", "{p.label()}" }
                        }
                    }
                }
                FormField { label: "Timezone", id: "jd-zone", error: zone_error.clone(),
                    select {
                        id: "jd-zone",
                        class: "select native",
                        onchange: {
                            let j = job_for_edit.clone();
                            move |e| edit(ui, j.as_ref(), |f| f.timezone = e.value())
                        },
                        for z in Zone::ALL {
                            option { key: "{z.name()}", value: "{z.name()}", selected: z.name() == form.timezone, "{z.name()}" }
                        }
                        if form.zone().is_none() {
                            option { value: "{form.timezone}", selected: true, "{form.timezone}" }
                        }
                    }
                }
                FormField { label: "Owner", id: "jd-owner", error: owner_error.clone(),
                    input {
                        id: "jd-owner",
                        class: cls(&owner_error, "input"),
                        value: "{form.owner}",
                        spellcheck: "false",
                        title: "The role the SQL runs as",
                        oninput: {
                            let j = job_for_edit.clone();
                            move |e| edit(ui, j.as_ref(), |f| f.owner = e.value())
                        },
                    }
                }
                FormField { label: "Retry attempts", id: "jd-retries", error: retries_error.clone(),
                    input {
                        id: "jd-retries",
                        class: cls(&retries_error, "input"),
                        inputmode: "numeric",
                        value: "{form.retries}",
                        oninput: {
                            let j = job_for_edit.clone();
                            move |e| edit(ui, j.as_ref(), |f| f.retries = e.value())
                        },
                    }
                }
                div { class: "field next-runs",
                    span { class: "field-label", "Next executions (UTC)" }
                    match next.clone() {
                        Ok(runs) if runs.is_empty() => rsx! { span { class: "muted", "This schedule never runs." } },
                        Ok(runs) => rsx! {
                            ul { class: "next-list",
                                for t in runs {
                                    li { key: "{t}", class: "mono", "{date_second(t, 0)}" }
                                }
                            }
                        },
                        Err(_) => rsx! { span { class: "muted", "Fix the schedule to see its next runs." } },
                    }
                }
                FormField { label: "Timeout", id: "jd-timeout",
                    select {
                        id: "jd-timeout",
                        class: "select native",
                        onchange: {
                            let j = job_for_edit.clone();
                            move |e| {
                                if let Ok(n) = e.value().parse::<u32>() {
                                    edit(ui, j.as_ref(), |f| f.timeout_secs = n);
                                }
                            }
                        },
                        for t in timeouts.iter().copied() {
                            option { key: "{t}", value: "{t}", selected: t == form.timeout_secs, "{secs_label(t)}" }
                        }
                    }
                }
                FormField { label: "Backoff interval", id: "jd-backoff",
                    select {
                        id: "jd-backoff",
                        class: "select native",
                        onchange: {
                            let j = job_for_edit.clone();
                            move |e| {
                                if let Ok(n) = e.value().parse::<u32>() {
                                    edit(ui, j.as_ref(), |f| f.backoff_secs = n);
                                }
                            }
                        },
                        for b in backoffs.iter().copied() {
                            option { key: "{b}", value: "{b}", selected: b == form.backoff_secs, "{secs_label(b)}" }
                        }
                    }
                }
            }
            div { class: "field sql-field",
                span { class: "field-label", id: "jd-sql-label", "SQL" }
                div { class: if sql_error.is_some() { "sql-box invalid" } else { "sql-box" },
                    CodeEditor {
                        value: form.sql.clone(),
                        protocol: Protocol::Sql,
                        label: "SQL the job runs",
                        placeholder: "INSERT INTO sales_daily SELECT …",
                        on_input: {
                            let j = job_for_edit.clone();
                            move |t: String| edit(ui, j.as_ref(), |f| f.sql = t)
                        },
                    }
                }
                if let Some(m) = sql_error.clone() {
                    div { class: "error", role: "alert", "{m}" }
                }
            }
        }
    }
}

/// Whether the form has no issue, so the review can open.
fn issues_free(form: &JobForm, jobs: &[ScheduledJob], job: Option<&ScheduledJob>) -> bool {
    form.issues(jobs, job.map(|j| j.id.as_str())).is_empty()
}
