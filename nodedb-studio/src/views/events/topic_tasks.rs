//! The topics page's writes. Each runs under `spawn_forever` and touches
//! root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::models::topics::{ResetScope, ResetTarget};
use crate::services::backend::Backend;
use crate::state::events::feeds::Notice;
use crate::state::events::topics::TopicsUi;

/// Create the topic the sheet describes and select it.
pub async fn create_topic(mut ui: Signal<TopicsUi>, backend: Rc<dyn Backend>) {
    let Some(sheet) = ui.peek().create.clone() else {
        return;
    };
    if let Some(s) = ui.write().create.as_mut() {
        s.busy = true;
        s.error = None;
    }
    let result = backend.create_topic(&sheet.form).await;
    let mut w = ui.write();
    match result {
        Ok(topic) => {
            w.create = None;
            w.select(&topic.id);
            w.notice = Some(Notice::ok(format!(
                "Created topic {} ({}), {} partitions, {} days retention.",
                topic.name, topic.id, topic.partitions, topic.retention_days
            )));
            w.epoch += 1;
        }
        Err(e) => {
            if let Some(s) = w.create.as_mut() {
                s.busy = false;
                s.error = Some(e.to_string());
            }
        }
    }
}

/// Publish the message the sheet holds, then show the partition it reached.
pub async fn publish(mut ui: Signal<TopicsUi>, backend: Rc<dyn Backend>) {
    let Some(sheet) = ui.peek().publish.clone() else {
        return;
    };
    if let Some(s) = ui.write().publish.as_mut() {
        s.busy = true;
        s.error = None;
    }
    let result = backend.publish_message(&sheet.topic, &sheet.draft).await;
    let mut w = ui.write();
    match result {
        Ok(m) => {
            w.publish = None;
            // Show the new message: the range that ends at its offset.
            let from = m.offset.saturating_sub(29);
            w.input.partition = m.partition;
            w.input.from = from.to_string();
            w.input.to = m.offset.to_string();
            w.applied = (m.partition, from, m.offset);
            w.selected_message = Some((m.partition, m.offset));
            w.notice = Some(Notice::ok(format!(
                "Published to partition {} at offset {}.",
                m.partition, m.offset
            )));
            w.epoch += 1;
        }
        Err(e) => {
            if let Some(s) = w.publish.as_mut() {
                s.busy = false;
                s.error = Some(e.to_string());
            }
        }
    }
}

/// Apply the reset the review shows.
pub async fn apply_reset(
    mut ui: Signal<TopicsUi>,
    backend: Rc<dyn Backend>,
    scope: ResetScope,
    target: ResetTarget,
) {
    let Some(form) = ui.peek().reset.clone() else {
        return;
    };
    if let Some(r) = ui.write().reset.as_mut() {
        r.busy = true;
        r.error = None;
    }
    let result = backend.apply_offset_reset(&form.group, scope, target).await;
    let mut w = ui.write();
    match result {
        Ok(group) => {
            w.reset = None;
            w.notice = Some(Notice::ok(format!(
                "Reset the committed offsets of group {} as the review showed.",
                group.name
            )));
            w.epoch += 1;
        }
        Err(e) => {
            if let Some(r) = w.reset.as_mut() {
                r.busy = false;
                r.error = Some(e.to_string());
            }
        }
    }
}
