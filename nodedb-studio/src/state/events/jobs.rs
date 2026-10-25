//! Scheduled jobs page state: the selection, the edit buffer, and the
//! review step.

use crate::models::jobs::JobForm;

/// What the editor holds: edits to one job, or a new job.
#[derive(Debug, Clone, PartialEq)]
pub enum Editing {
    /// Job `id`, read at `revision`.
    Job {
        id: String,
        revision: u64,
    },
    New,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobEditor {
    pub target: Editing,
    pub form: JobForm,
    /// Review changes was pressed, so the form shows every issue.
    pub tried: bool,
    /// The review step is open.
    pub review: bool,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct JobsUi {
    pub selected: Option<String>,
    pub filter: String,
    /// Edits in progress. `None` shows the stored job.
    pub editor: Option<JobEditor>,
    pub selected_run: Option<String>,
    /// The run log is open at full size.
    pub log_open: bool,
    /// A job the person asked to select while edits are unsaved. The editor
    /// asks to review, discard, or cancel first.
    pub switch_to: Option<String>,
    pub notice: Option<crate::state::events::feeds::Notice>,
    /// Bumps after a write, so reads run again.
    pub epoch: u32,
}

impl JobsUi {
    /// Whether the editor holds edits `stored` does not: a new job, or a form
    /// that differs from the job it edits.
    pub fn dirty(&self, stored: Option<&crate::models::jobs::ScheduledJob>) -> bool {
        match (&self.editor, stored) {
            (Some(e), _) if e.target == Editing::New => true,
            (Some(e), Some(job)) => e.form != JobForm::of(job),
            (Some(_), None) | (None, _) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> crate::models::jobs::ScheduledJob {
        crate::data::mock::seeded_jobs().jobs.remove(0)
    }

    fn editing(form: JobForm) -> JobsUi {
        let j = job();
        JobsUi {
            editor: Some(JobEditor {
                target: Editing::Job {
                    id: j.id,
                    revision: j.revision,
                },
                form,
                tried: false,
                review: false,
                busy: false,
                error: None,
            }),
            ..JobsUi::default()
        }
    }

    #[test]
    fn an_untouched_editor_is_not_dirty() {
        let j = job();
        assert!(!editing(JobForm::of(&j)).dirty(Some(&j)));
        assert!(!JobsUi::default().dirty(Some(&j)));
    }

    #[test]
    fn an_edit_makes_the_editor_dirty_and_undoing_it_does_not() {
        let j = job();
        let mut f = JobForm::of(&j);
        f.cron = "30 2 * * *".into();
        assert!(editing(f.clone()).dirty(Some(&j)));
        f.cron = j.cron.clone();
        assert!(!editing(f).dirty(Some(&j)));
    }

    #[test]
    fn a_new_job_is_always_dirty() {
        let mut ui = editing(JobForm::blank("analytics.sales", "rollup_service"));
        if let Some(e) = ui.editor.as_mut() {
            e.target = Editing::New;
        }
        assert!(ui.dirty(None));
    }
}
