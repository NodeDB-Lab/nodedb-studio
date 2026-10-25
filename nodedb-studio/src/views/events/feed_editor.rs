//! The Create change feed drawer and its Edit twin: source, events to
//! capture, delivery, policy, and a preview of the events the feed will
//! send. Review feed opens the review. Nothing reaches the server until the
//! review's Create button.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::feed_draft::{DraftField, FeedDraft, Issue};
use crate::models::feeds::{
    Backoff, ChangeFeed, ChangeOp, DeliveryKind, RetentionUnit, StartPosition,
};
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::feeds::{EditorMode, EditorStep, FeedEditor, FeedsUi};
use crate::views::events::common::{FormField, InfoTip};
use crate::views::events::feed_preview::{PreviewColumn, ReviewStep};
use crate::views::events::feed_tasks;

/// Change the draft under edit.
fn edit(mut ui: Signal<FeedsUi>, change: impl FnOnce(&mut FeedDraft)) {
    if let Some(e) = ui.write().editor.as_mut() {
        change(&mut e.draft);
        e.error = None;
    }
}

/// The first issue at `at`, once the form has been tried or the field has text.
fn issue_at(issues: &[Issue], at: DraftField, show: bool) -> Option<String> {
    show.then(|| {
        issues
            .iter()
            .find(|i| i.at == at)
            .map(|i| i.message.clone())
    })
    .flatten()
}

fn invalid(error: &Option<String>) -> &'static str {
    if error.is_some() {
        "input invalid"
    } else {
        "input"
    }
}

#[component]
pub fn FeedSheet(
    editor: FeedEditor,
    feeds: Vec<ChangeFeed>,
    catalog: Vec<String>,
    database: String,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().feeds;
    let draft = editor.draft.clone();
    let issues = draft.issues(&feeds, editor.editing());
    let tried = editor.tried;
    let original: Option<ChangeFeed> = editor
        .editing()
        .and_then(|id| feeds.iter().find(|f| f.id == id).cloned());

    let preview = {
        let backend = backend.clone();
        let draft = draft.clone();
        use_resource(use_reactive!(|draft| {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.preview_feed(&draft).await }
        }))
    };
    let preview_state = resource_state(&preview);

    let title = match &editor.mode {
        EditorMode::Create => "Create change feed".to_string(),
        EditorMode::Edit { .. } => format!("Edit change feed {}", draft.name),
    };
    let save_label = match &editor.mode {
        EditorMode::Create => format!("Create change feed {}", draft.name.trim()),
        EditorMode::Edit { .. } => format!("Save changes to {}", draft.name.trim()),
    };
    let review_label = match &editor.mode {
        EditorMode::Create => "Review feed",
        EditorMode::Edit { .. } => "Review changes",
    };

    let on_review = {
        let issues = issues.clone();
        move |_| {
            if issues.is_empty() {
                if let Some(e) = ui.write().editor.as_mut() {
                    e.step = EditorStep::Review;
                    e.tried = true;
                }
            } else if let Some(e) = ui.write().editor.as_mut() {
                e.tried = true;
            }
        }
    };
    let can_save =
        preview_state.loaded().is_some_and(|p| p.can_create()) && issues.is_empty() && !editor.busy;
    let blocked = tried && !issues.is_empty();
    let sources: Vec<String> = {
        let mut s: Vec<String> = catalog.iter().map(|c| format!("{database}.{c}")).collect();
        if !s.contains(&draft.source) {
            s.push(draft.source.clone());
        }
        s
    };
    let name_error = issue_at(&issues, DraftField::Name, tried || !draft.name.is_empty());
    let filter_error = issue_at(&issues, DraftField::Filter, true);
    let projection_error = issue_at(&issues, DraftField::Projection, true);
    let endpoint_error = issue_at(
        &issues,
        DraftField::Endpoint,
        tried || !draft.endpoint.is_empty(),
    );
    let alias_error = issue_at(&issues, DraftField::Alias, tried || !draft.alias.is_empty());
    let retries_error = issue_at(&issues, DraftField::MaxRetries, true);
    let retention_error = issue_at(&issues, DraftField::Retention, true);
    let ops_error = issue_at(&issues, DraftField::Operations, true);
    let checkpoint_error = issue_at(&issues, DraftField::Checkpoint, tried);
    let checkpoints = preview_state
        .loaded()
        .map(|p| p.checkpoints.clone())
        .unwrap_or_default();

    let footer = match editor.step {
        EditorStep::Form => rsx! {
            SheetFooter {
                error: editor.error.clone().or_else(|| blocked.then(|| format!("Fix {} issue(s) above to review the feed.", issues.len()))),
                hint: Some("Creating a change feed starts server delivery. Pause display in Studio does not.".to_string()),
                button { class: "btn", onclick: move |_| ui.write().editor = None, "Cancel" }
                button { class: "btn primary", onclick: on_review, "{review_label}" }
            }
        },
        EditorStep::Review => rsx! {
            SheetFooter {
                error: editor.error.clone(),
                hint: Some("The feed is created only when you press the button on the right.".to_string()),
                button {
                    class: "btn",
                    disabled: editor.busy,
                    onclick: move |_| {
                        if let Some(e) = ui.write().editor.as_mut() {
                            e.step = EditorStep::Form;
                        }
                    },
                    "Back to edit"
                }
                button {
                    class: "btn primary",
                    disabled: !can_save,
                    title: if can_save { "" } else { "Resolve the failed checks before saving" },
                    onclick: {
                        let backend = backend.clone();
                        move |_| {
                            let backend = backend.clone();
                            spawn_forever(async move { feed_tasks::save_editor(ui, backend).await });
                        }
                    },
                    "{save_label}"
                }
            }
        },
    };

    rsx! {
        Sheet {
            title,
            width: SheetWidth::Wide,
            class: "feed-sheet",
            on_close: move |_| ui.write().editor = None,
            footer,
            match editor.step {
                EditorStep::Review => rsx! {
                    ReviewStep { editor: editor.clone(), original, preview: preview_state.clone() }
                },
                EditorStep::Form => rsx! {
                    div { class: "sheet-cols feed-cols",
                        div { class: "sheet-col",
                            h3 { class: "section-title", "Source" }
                            FormField { label: "Feed name", id: "fe-name", error: name_error.clone(),
                                input {
                                    id: "fe-name",
                                    class: invalid(&name_error),
                                    value: "{draft.name}",
                                    placeholder: "export_sales",
                                    spellcheck: "false",
                                    "aria-invalid": "{name_error.is_some()}",
                                    oninput: move |e| edit(ui, |d| d.name = e.value()),
                                }
                            }
                            FormField { label: "Database and collection", id: "fe-source",
                                select {
                                    id: "fe-source",
                                    class: "select native",
                                    disabled: editor.editing().is_some(),
                                    title: if editor.editing().is_some() { "A feed's source cannot change. Create a new feed instead." } else { "" },
                                    onchange: move |e| edit(ui, |d| d.source = e.value()),
                                    for s in sources.iter() {
                                        option { key: "{s}", value: "{s}", selected: *s == draft.source, "{s}" }
                                    }
                                }
                            }
                            h3 { class: "section-title", "Events to capture" }
                            div { class: "field",
                                span { class: "field-label", id: "fe-ops", "Operations" }
                                div { class: "check-list", role: "group", "aria-labelledby": "fe-ops",
                                    for op in ChangeOp::ALL {
                                        label { key: "{op.label()}", class: "check-row",
                                            input {
                                                r#type: "checkbox",
                                                class: "checkbox",
                                                checked: draft.operations.contains(&op),
                                                onchange: move |_| edit(ui, |d| d.toggle(op)),
                                            }
                                            "{op.title()}"
                                        }
                                    }
                                }
                                if let Some(m) = ops_error {
                                    div { class: "error", role: "alert", "{m}" }
                                }
                            }
                            div { class: "field",
                                label { r#for: "fe-filter",
                                    "Filter (optional) "
                                    InfoTip { text: "Capture only rows that match. Use field = 'value', joined by AND. Operators: = != < <= > >=.".to_string() }
                                }
                                input {
                                    id: "fe-filter",
                                    class: invalid(&filter_error),
                                    value: "{draft.filter}",
                                    placeholder: "store_id = 'nyc'",
                                    spellcheck: "false",
                                    oninput: move |e| edit(ui, |d| d.filter = e.value()),
                                }
                                if let Some(m) = filter_error.clone() {
                                    div { class: "error", role: "alert", "{m}" }
                                }
                            }
                            div { class: "field",
                                label { r#for: "fe-projection",
                                    "Projection fields (optional) "
                                    InfoTip { text: "Deliver only these fields of the document. Leave empty to deliver the whole document.".to_string() }
                                }
                                input {
                                    id: "fe-projection",
                                    class: invalid(&projection_error),
                                    value: "{draft.projection}",
                                    placeholder: "_id, order_id, store_id, amount",
                                    spellcheck: "false",
                                    oninput: move |e| edit(ui, |d| d.projection = e.value()),
                                }
                                if let Some(m) = projection_error.clone() {
                                    div { class: "error", role: "alert", "{m}" }
                                }
                            }
                            label { class: "check-row",
                                input {
                                    r#type: "checkbox",
                                    class: "checkbox",
                                    checked: draft.before_image,
                                    onchange: move |e| edit(ui, |d| d.before_image = e.checked()),
                                }
                                "Include before image "
                                InfoTip { text: "Update and delete events carry the previous document. The preview checks whether this source can supply it.".to_string() }
                            }
                            h3 { class: "section-title", "Start position" }
                            div { class: "start-list", role: "radiogroup", "aria-label": "Start position",
                                for p in StartPosition::ALL {
                                    label { key: "{p.label()}", class: "start-option",
                                        input {
                                            r#type: "radio",
                                            class: "radio",
                                            name: "fe-start",
                                            checked: draft.start == p,
                                            onchange: move |_| edit(ui, |d| d.start = p),
                                        }
                                        span { class: "start-text",
                                            span { class: "start-label", "{p.label()}" }
                                            span { class: "hint", "{p.hint()}" }
                                        }
                                        if p == StartPosition::SavedCheckpoint {
                                            select {
                                                class: "select native compact",
                                                "aria-label": "Saved checkpoint",
                                                disabled: draft.start != p,
                                                onchange: move |e| edit(ui, |d| d.checkpoint = e.value()),
                                                option { value: "", selected: draft.checkpoint.is_empty(), "Select checkpoint" }
                                                for c in checkpoints.iter() {
                                                    option { key: "{c}", value: "{c}", selected: *c == draft.checkpoint, "{c}" }
                                                }
                                            }
                                        }
                                    }
                                }
                                if let Some(m) = checkpoint_error {
                                    div { class: "error", role: "alert", "{m}" }
                                }
                            }
                        }
                        div { class: "sheet-col",
                            h3 { class: "section-title", "Delivery" }
                            FormField { label: "Consumer type", id: "fe-kind",
                                select {
                                    id: "fe-kind",
                                    class: "select native",
                                    onchange: move |e| {
                                        let kind = DeliveryKind::ALL
                                            .into_iter()
                                            .find(|k| k.consumer_label() == e.value())
                                            .unwrap_or(DeliveryKind::Webhook);
                                        edit(ui, |d| d.delivery = kind);
                                    },
                                    for k in DeliveryKind::ALL {
                                        option {
                                            key: "{k.label()}",
                                            value: "{k.consumer_label()}",
                                            selected: draft.delivery == k,
                                            "{k.consumer_label()}"
                                        }
                                    }
                                }
                            }
                            if draft.delivery == DeliveryKind::Webhook {
                                div { class: "field",
                                    label { r#for: "fe-endpoint",
                                        "Endpoint URL "
                                        InfoTip { text: "An https:// address. Put tokens in a secret reference. A token in the URL is refused because the URL is shown and logged.".to_string() }
                                    }
                                    input {
                                        id: "fe-endpoint",
                                        class: invalid(&endpoint_error),
                                        value: "{draft.endpoint}",
                                        placeholder: "https://warehouse.example.com/webhooks/sales",
                                        spellcheck: "false",
                                        "aria-invalid": "{endpoint_error.is_some()}",
                                        oninput: move |e| edit(ui, |d| d.endpoint = e.value()),
                                    }
                                    if let Some(m) = endpoint_error.clone() {
                                        div { class: "error", role: "alert", "{m}" }
                                    }
                                }
                            }
                            FormField {
                                label: "Alias",
                                id: "fe-alias",
                                error: alias_error.clone(),
                                hint: "The consumer's name on this feed. Its checkpoint is scoped to the feed.".to_string(),
                                input {
                                    id: "fe-alias",
                                    class: invalid(&alias_error),
                                    value: "{draft.alias}",
                                    placeholder: "warehouse_sink",
                                    spellcheck: "false",
                                    "aria-invalid": "{alias_error.is_some()}",
                                    oninput: move |e| edit(ui, |d| d.alias = e.value()),
                                }
                            }
                            div { class: "field-row two",
                                FormField { label: "Retry backoff", id: "fe-backoff",
                                    select {
                                        id: "fe-backoff",
                                        class: "select native",
                                        onchange: move |e| {
                                            let b = Backoff::ALL
                                                .into_iter()
                                                .find(|b| b.label() == e.value())
                                                .unwrap_or(Backoff::Exponential);
                                            edit(ui, |d| d.backoff = b);
                                        },
                                        for b in Backoff::ALL {
                                            option { key: "{b.label()}", value: "{b.label()}", selected: draft.backoff == b, "{b.label()}" }
                                        }
                                    }
                                }
                                FormField { label: "Max retries", id: "fe-retries", error: retries_error.clone(),
                                    input {
                                        id: "fe-retries",
                                        class: invalid(&retries_error),
                                        inputmode: "numeric",
                                        value: "{draft.max_retries}",
                                        oninput: move |e| edit(ui, |d| d.max_retries = e.value()),
                                    }
                                }
                            }
                            div { class: "field-row two",
                                FormField { label: "Retention window", id: "fe-retention", error: retention_error.clone(),
                                    input {
                                        id: "fe-retention",
                                        class: invalid(&retention_error),
                                        inputmode: "numeric",
                                        value: "{draft.retention}",
                                        oninput: move |e| edit(ui, |d| d.retention = e.value()),
                                    }
                                }
                                FormField { label: "Unit", id: "fe-unit",
                                    select {
                                        id: "fe-unit",
                                        class: "select native",
                                        onchange: move |e| {
                                            let u = RetentionUnit::ALL
                                                .into_iter()
                                                .find(|u| u.label() == e.value())
                                                .unwrap_or(RetentionUnit::Days);
                                            edit(ui, |d| d.retention_unit = u);
                                        },
                                        for u in RetentionUnit::ALL {
                                            option { key: "{u.label()}", value: "{u.label()}", selected: draft.retention_unit == u, "{u.label()}" }
                                        }
                                    }
                                }
                            }
                            div { class: "callout info",
                                Ico { icon: Icon::Info, size: 16 }
                                span { "Delivery is at least once. A consumer can receive an event more than once, so make it duplicate-aware." }
                            }
                        }
                        PreviewColumn { state: preview_state.clone(), open: editor.open_nodes.clone() }
                    }
                },
            }
        }
    }
}
