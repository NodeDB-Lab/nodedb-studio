//! The page's writes. Each runs under `spawn_forever` and touches root state
//! only, so it finishes when the user leaves the page mid-run.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::clipboard::copy_text;
use crate::models::designer_change::{ChangeKind, PendingChange};
use crate::models::designer_form::IndexForm;
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::designer::{ApplyResult, DesignerState, Inspector, ReviewState, Target};

/// Stage `kinds` in order. When one is refused, the ones this call staged
/// are discarded again, so a form stages whole or not at all.
async fn stage_whole(
    backend: &Rc<dyn Backend>,
    kinds: Vec<ChangeKind>,
) -> Result<Vec<PendingChange>, StudioError> {
    let mut staged: Vec<PendingChange> = Vec::new();
    for kind in kinds {
        match backend.stage_change(None, kind).await {
            Ok(change) => staged.push(change),
            Err(e) => {
                for change in &staged {
                    // The discard can only fail if the change is gone already.
                    let _ = backend.discard_change(&change.id).await;
                }
                return Err(e);
            }
        }
    }
    Ok(staged)
}

/// Stage the Create collection form, then open the review. A refusal stays
/// in the drawer, under the form.
pub async fn create_collection(
    mut state: Signal<DesignerState>,
    backend: Rc<dyn Backend>,
    kinds: Vec<ChangeKind>,
    name: String,
) {
    if let Some(c) = state.write().create.as_mut() {
        c.busy = true;
        c.error = None;
    }
    match stage_whole(&backend, kinds).await {
        Ok(_) => {
            let mut s = state.write();
            s.create = None;
            s.select(Target::Collection(name));
            s.review = Some(ReviewState::default());
            s.bump();
        }
        Err(e) => {
            if let Some(c) = state.write().create.as_mut() {
                c.busy = false;
                c.error = Some(e.to_string());
            }
        }
    }
}

/// Stage a new index from the Properties form.
pub async fn stage_new_index(
    mut state: Signal<DesignerState>,
    backend: Rc<dyn Backend>,
    index: crate::models::designer::DesignIndex,
) {
    let name = index.name.clone();
    match backend
        .stage_change(None, ChangeKind::CreateIndex(index))
        .await
    {
        Ok(_) => {
            let mut s = state.write();
            s.select(Target::Index(name));
            s.bump();
        }
        Err(e) => state.write().form_error = Some(e.to_string()),
    }
}

/// Replace a staged index with the edited form.
pub async fn restage_index(
    mut state: Signal<DesignerState>,
    backend: Rc<dyn Backend>,
    form: IndexForm,
    index: crate::models::designer::DesignIndex,
) {
    let name = index.name.clone();
    match backend
        .stage_change(form.replace.as_deref(), ChangeKind::CreateIndex(index))
        .await
    {
        Ok(_) => {
            let mut s = state.write();
            s.select(Target::Index(name));
            s.bump();
        }
        Err(e) => state.write().form_error = Some(e.to_string()),
    }
}

/// Stage a logical reference from the Properties form.
pub async fn stage_reference(
    mut state: Signal<DesignerState>,
    backend: Rc<dyn Backend>,
    reference: crate::models::designer::LogicalReference,
) {
    match backend
        .stage_change(None, ChangeKind::AddReference(reference))
        .await
    {
        Ok(change) => {
            let mut s = state.write();
            s.select(Target::Reference(change.id));
            s.bump();
        }
        Err(e) => state.write().form_error = Some(e.to_string()),
    }
}

/// Remove a staged change.
pub async fn discard(mut state: Signal<DesignerState>, backend: Rc<dyn Backend>, id: String) {
    let out = backend.discard_change(&id).await;
    let mut s = state.write();
    match out {
        Ok(()) => {
            s.selected = None;
            s.inspector = Inspector::Selection;
            s.notice = None;
        }
        Err(e) => s.notice = Some(e.to_string()),
    }
    s.bump();
}

/// Remove every staged change and close the review.
pub async fn discard_all(mut state: Signal<DesignerState>, backend: Rc<dyn Backend>) {
    let out = backend.discard_all_changes().await;
    let mut s = state.write();
    match out {
        Ok(()) => {
            s.review = None;
            s.selected = None;
            s.notice = None;
        }
        Err(e) => s.notice = Some(e.to_string()),
    }
    s.bump();
}

/// Apply every staged change and report how it ended.
/// Apply the staged changes the review of `reviewed` read.
pub async fn apply(mut state: Signal<DesignerState>, backend: Rc<dyn Backend>, reviewed: u64) {
    if let Some(r) = state.write().review.as_mut() {
        r.busy = true;
        r.result = None;
    }
    let result = match backend.apply_changes(reviewed).await {
        Ok(report) => match report.failed {
            None => ApplyResult::Applied(report.applied.len()),
            Some((failed, reason)) => ApplyResult::Partial {
                applied: report.applied.len(),
                failed,
                reason,
            },
        },
        Err(e @ StudioError::StaleRevision { .. }) => ApplyResult::Stale(e.to_string()),
        Err(e) => ApplyResult::Failed(e.to_string()),
    };
    let mut s = state.write();
    // A stale review stays on screen with its error until "Review again".
    if !matches!(result, ApplyResult::Stale(_)) {
        s.bump();
    }
    match result {
        ApplyResult::Applied(n) => {
            s.review = None;
            s.selected = None;
            s.notice = Some(format!(
                "Applied {n} change{}.",
                if n == 1 { "" } else { "s" }
            ));
        }
        other @ (ApplyResult::Partial { .. } | ApplyResult::Failed(_) | ApplyResult::Stale(_)) => {
            if let Some(r) = s.review.as_mut() {
                r.busy = false;
                r.result = Some(other);
            }
        }
    }
}

/// Copy the staged changes' DDL to the clipboard.
pub async fn export_ddl(mut state: Signal<DesignerState>, backend: Rc<dyn Backend>) {
    let notice = match backend.review_changes().await {
        Ok(review) => {
            let script = review.script();
            let lines = script.lines().count();
            match copy_text(script).await {
                Ok(()) => format!("Copied {lines} lines of DDL."),
                Err(e) => e.to_string(),
            }
        }
        Err(e) => e.to_string(),
    };
    state.write().notice = Some(notice);
}
