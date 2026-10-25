//! The scheduled jobs page's writes. Each runs under `spawn_forever` and
//! touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::events::feeds::Notice;
use crate::state::events::jobs::{Editing, JobsUi};

/// Create the job or save the edit the editor holds.
pub async fn save(mut ui: Signal<JobsUi>, backend: Rc<dyn Backend>) {
    let Some(editor) = ui.peek().editor.clone() else {
        return;
    };
    if let Some(e) = ui.write().editor.as_mut() {
        e.busy = true;
        e.error = None;
    }
    let result = match &editor.target {
        Editing::New => backend.create_job(&editor.form).await,
        Editing::Job { id, revision } => backend.save_job(id, *revision, &editor.form).await,
    };
    let mut w = ui.write();
    match result {
        Ok(job) => {
            w.notice = Some(Notice::ok(match editor.target {
                Editing::New => format!("Created job {} ({}).", job.name, job.id),
                Editing::Job { .. } => {
                    format!("Saved {} at revision {}.", job.name, job.revision)
                }
            }));
            w.selected = Some(job.id);
            w.editor = None;
            w.selected_run = None;
            w.epoch += 1;
        }
        Err(e) => {
            if let Some(ed) = w.editor.as_mut() {
                ed.busy = false;
                ed.error = Some(e.to_string());
            }
        }
    }
}

/// Enable or disable one job's schedule.
pub async fn set_enabled(
    mut ui: Signal<JobsUi>,
    backend: Rc<dyn Backend>,
    id: String,
    enabled: bool,
) {
    let result = backend.set_job_enabled(&id, enabled).await;
    let mut w = ui.write();
    match result {
        Ok(job) => {
            w.notice = Some(Notice::ok(if enabled {
                format!("Enabled {}. It runs on its schedule again.", job.name)
            } else {
                format!("Disabled {}. Its history stays.", job.name)
            }));
            w.epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}

/// Start one run of a job now, outside its schedule.
pub async fn run_now(mut ui: Signal<JobsUi>, backend: Rc<dyn Backend>, id: String, name: String) {
    let result = backend.run_job_now(&id).await;
    let mut w = ui.write();
    match result {
        Ok(run) => {
            w.notice = Some(Notice::ok(format!("Started a run of {name} now.")));
            w.selected_run = Some(run.id);
            w.epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}

/// Cancel one running run. The schedule is unchanged.
pub async fn cancel_run(mut ui: Signal<JobsUi>, backend: Rc<dyn Backend>, run: String) {
    let result = backend.cancel_job_run(&run).await;
    let mut w = ui.write();
    match result {
        Ok(_) => {
            w.notice = Some(Notice::ok("Cancelled the run. The schedule is unchanged."));
            w.epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}
