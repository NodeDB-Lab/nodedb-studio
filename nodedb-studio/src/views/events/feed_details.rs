//! Feed details: the definition of the selected feed, its consumers, its
//! delivery activity, and the selected event.
//!
//! Feed and consumer changes are explicit actions that name their feed.
//! Nothing here is Studio's tail: pausing delivery stops the server's
//! delivery, and the tail controls never do.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::json_view::JsonView;
use crate::components::tabs::TabBar;
use crate::models::feed_draft::FeedDraft;
use crate::models::feeds::{ChangeFeed, FeedEvent, FeedState};
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::feeds::{FeedEditor, FeedTab};
use crate::state::explorer::{RecordView, Selected, select_collection};
use crate::views::events::common::{StatusBadge, StatusText};
use crate::views::events::feed_events::displayed;
use crate::views::events::feed_tasks;

#[component]
pub fn FeedDetails(feed: ChangeFeed, catalog: Vec<String>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let nav = use_navigator();
    let mut ui = use_context::<EventsUi>().feeds;
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let epoch = use_memo(move || ui.read().epoch);
    let id = feed.id.clone();

    let consumers = {
        let backend = backend.clone();
        let id = id.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.feed_consumers(&id).await }
        }))
    };
    let attempts = {
        let backend = backend.clone();
        let id = id.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.delivery_attempts(&id).await }
        }))
    };
    let retained = {
        let backend = backend.clone();
        let id = id.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.recent_events(&id).await }
        }))
    };

    let (tab, chosen_event, attempts_for_event, confirm_delete, tail) = {
        let u = ui.read();
        (
            u.tab,
            u.selected_event.clone(),
            u.attempts_for_event,
            u.confirm_delete,
            u.tail.clone(),
        )
    };
    let retained_events: Vec<FeedEvent> = retained
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let own: &[FeedEvent] = if tail.is_for(&feed.id) {
        &tail.shown
    } else {
        &[]
    };
    let events = displayed(own, &retained_events);
    let event: Option<FeedEvent> = chosen_event
        .and_then(|c| events.iter().find(|e| e.id == c).cloned())
        .or_else(|| events.first().cloned());

    let consumer_state = resource_state(&consumers);
    let consumer_list = consumer_state.loaded().cloned().unwrap_or_default();
    let attempt_state = resource_state(&attempts);
    let in_catalog = catalog.contains(&feed.collection);
    let paused = feed.state == FeedState::Paused;
    let name = feed.name.clone();

    let edit = {
        let feed = feed.clone();
        move |_| {
            ui.write().editor = Some(FeedEditor::edit(
                &feed.id,
                feed.revision,
                FeedDraft::of(&feed),
            ));
        }
    };
    let open_collection = {
        let collection = feed.collection.clone();
        move |_| {
            select_collection(&mut selected, &mut view, &collection);
            nav.push(Route::Explorer {});
        }
    };
    let toggle_delivery = {
        let (backend, id) = (backend.clone(), id.clone());
        move |_| {
            let (backend, id) = (backend.clone(), id.clone());
            spawn_forever(async move { feed_tasks::set_paused(ui, backend, id, !paused).await });
        }
    };

    rsx! {
        div { class: "inspector feed-details",
            div { class: "insp-head",
                div { class: "insp-title",
                    h3 { "{feed.name}" }
                    StatusBadge { tone: feed.state.tone(), text: feed.state.label().to_string() }
                }
            }
            TabBar {
                items: FeedTab::ALL.iter().map(|t| t.label()).collect::<Vec<_>>(),
                active: FeedTab::ALL.iter().position(|t| *t == tab).unwrap_or(0),
                class: "sub insp-tabs",
                on_select: move |i: usize| ui.write().tab = FeedTab::ALL[i],
            }
            match tab {
                FeedTab::Definition => rsx! {
                    if let Some(why) = feed.error.clone() {
                        div { class: "callout err feed-note", role: "alert",
                            Ico { icon: Icon::CircleAlert, size: 16 }
                            span { "Delivery stopped. {why}" }
                        }
                    }
                    if paused {
                        div { class: "callout warn feed-note",
                            Ico { icon: Icon::Pause, size: 16 }
                            span { "Delivery is paused. Events accumulate on the server until you resume." }
                        }
                    }
                    div { class: "kv feed-kv",
                        div { class: "kv-row", span { "Collection" } span { "{feed.collection}" } }
                        div { class: "kv-row", span { "Operations" } span { "{feed.operations_label()}" } }
                        if !feed.filter.is_empty() {
                            div { class: "kv-row", span { "Filter" } span { class: "mono", "{feed.filter}" } }
                        }
                        if !feed.projection.is_empty() {
                            div { class: "kv-row", span { "Projection" } span { class: "mono", "{feed.projection}" } }
                        }
                        div { class: "kv-row", span { "Destination" } span { "{feed.delivery.label()}" } }
                        if let Some(endpoint) = feed.endpoint.clone() {
                            div { class: "kv-row", span { "Endpoint" } span { class: "mono", "{endpoint}" } }
                        }
                        div { class: "kv-row",
                            span { "Delivery" }
                            span { title: "A consumer can receive an event more than once, so it must tolerate duplicates. Failed deliveries retry {feed.backoff.label().to_lowercase()}, up to {feed.max_retries} times.",
                                "At least once"
                            }
                        }
                        div { class: "kv-row",
                            span { "Checkpoint" }
                            span { class: "mono", title: "Scoped to {feed.name}", "{feed.checkpoint}" }
                        }
                    }
                    div { class: "inspector-actions feed-actions",
                        button { class: "btn", title: "Edit {name}", onclick: edit,
                            Ico { icon: Icon::Pencil, size: 16 }
                            "Edit feed"
                        }
                        button {
                            class: "btn",
                            disabled: !in_catalog,
                            title: if in_catalog { "Open {feed.collection} in the Explorer" } else { "{feed.collection} is not in this database's catalog" },
                            onclick: open_collection,
                            Ico { icon: Icon::ExternalLink, size: 16 }
                            "Open collection"
                        }
                    }
                    if confirm_delete {
                        div { class: "callout warn feed-note feed-delete", role: "alertdialog", "aria-label": "Delete {name}",
                            Ico { icon: Icon::TriangleAlert, size: 16 }
                            div { class: "stack",
                                strong { "Delete change feed {name}?" }
                                span {
                                    "This removes {consumer_list.len()} consumer"
                                    if consumer_list.len() != 1 { "s" }
                                    " and their checkpoints"
                                    if !consumer_list.is_empty() {
                                        " ("
                                        {consumer_list.iter().map(|c| format!("{} {}", c.name, c.checkpoint)).collect::<Vec<_>>().join(", ")}
                                        ")"
                                    }
                                    ". Events not yet delivered are lost. This cannot be undone."
                                }
                                div { class: "row",
                                    button {
                                        class: "btn danger sm",
                                        onclick: {
                                            let (backend, id, name) = (backend.clone(), id.clone(), name.clone());
                                            move |_| {
                                                let (backend, id, name) = (backend.clone(), id.clone(), name.clone());
                                                spawn_forever(async move { feed_tasks::delete(ui, backend, id, name).await });
                                            }
                                        },
                                        "Delete feed {name}"
                                    }
                                    button { class: "btn sm", onclick: move |_| ui.write().confirm_delete = false, "Cancel" }
                                }
                            }
                        }
                    } else {
                        div { class: "inspector-actions feed-actions",
                            button {
                                class: "btn",
                                title: if paused { "Resume server delivery of {name}" } else { "Pause server delivery of {name}. Studio's tail is separate." },
                                onclick: toggle_delivery,
                                if paused {
                                    Ico { icon: Icon::Play, size: 16 }
                                    "Resume delivery"
                                } else {
                                    Ico { icon: Icon::Pause, size: 16 }
                                    "Pause delivery"
                                }
                            }
                            button {
                                class: "btn danger",
                                title: "Review what deleting {name} removes",
                                onclick: move |_| ui.write().confirm_delete = true,
                                Ico { icon: Icon::Trash, size: 16 }
                                "Delete feed"
                            }
                        }
                    }
                    h4 { class: "inspector-section sub", "Selected event" }
                    if let Some(e) = event.clone() {
                        JsonView { text: e.payload.clone() }
                        div { class: "event-meta",
                            "Delivery to {feed.consumer}: "
                            StatusText { tone: e.delivery.tone(), text: e.delivery.label().to_string() }
                        }
                        div { class: "inspector-actions feed-actions",
                            button {
                                class: "btn",
                                onclick: move |_| {
                                    let mut w = ui.write();
                                    w.tab = FeedTab::Activity;
                                    w.attempts_for_event = true;
                                },
                                Ico { icon: Icon::ExternalLink, size: 16 }
                                "View delivery attempts"
                            }
                        }
                    } else {
                        div { class: "insp-note", "This feed has no event to show." }
                    }
                },
                FeedTab::Consumers => rsx! {
                    div { class: "insp-sub consumers-lede",
                        "Each consumer has its own checkpoint, scoped to {feed.name}. Consumers acknowledge the same event at different times."
                    }
                    AsyncView {
                        loading: consumer_state.is_loading(),
                        empty: consumer_state.is_empty(),
                        error: consumer_state.error_message(),
                        retriable: consumer_state.is_retriable(),
                        empty_message: "This feed has no consumers.",
                    }
                    div { class: "list-cards",
                        for c in consumer_list.iter() {
                            div { key: "{c.id}", class: "card-line",
                                div { class: "card-line-head",
                                    strong { "{c.name}" }
                                    StatusText { tone: c.state.tone(), text: c.state.label().to_string() }
                                }
                                div { class: "kv",
                                    div { class: "kv-row", span { "Type" } span { "{c.kind.consumer_label()}" } }
                                    div { class: "kv-row", span { "Destination" } span { class: "mono", "{c.destination}" } }
                                    div { class: "kv-row", span { "Checkpoint" } span { class: "mono", "{c.checkpoint}" } }
                                    div { class: "kv-row", span { "Last acknowledged" } span { class: "mono", "{c.last_ack}" } }
                                    div { class: "kv-row",
                                        span { "Waiting" }
                                        span {
                                            if let Some(n) = c.pending { "{n} events" } else { "Not reported" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                FeedTab::Activity => {
                    let all = attempt_state.loaded().cloned().unwrap_or_default();
                    let focus = event.as_ref().filter(|_| attempts_for_event).map(|e| e.id.clone());
                    let shown: Vec<_> = all
                        .iter()
                        .filter(|a| focus.as_deref().is_none_or(|f| a.event_id == f))
                        .cloned()
                        .collect();
                    rsx! {
                        div { class: "insp-sub consumers-lede",
                            "A delivery acknowledgment confirms a consumer received an event. It is not a Lite sync acknowledgment."
                        }
                        if let Some(e) = event.as_ref().filter(|_| attempts_for_event) {
                            div { class: "row attempt-filter",
                                span { class: "chip",
                                    "Event {e.record_id}"
                                    button {
                                        class: "chip-remove",
                                        title: "Show attempts for every event",
                                        "aria-label": "Show attempts for every event",
                                        onclick: move |_| ui.write().attempts_for_event = false,
                                        Ico { icon: Icon::X, size: 16 }
                                    }
                                }
                            }
                        }
                        AsyncView {
                            loading: attempt_state.is_loading(),
                            empty: false,
                            error: attempt_state.error_message(),
                            retriable: attempt_state.is_retriable(),
                        }
                        if attempt_state.loaded().is_some() && shown.is_empty() {
                            div { class: "async-empty", "No delivery attempts for this selection." }
                        }
                        div { class: "list-cards",
                            for a in shown.iter() {
                                div { key: "{a.id}", class: "card-line",
                                    div { class: "card-line-head",
                                        strong { class: "mono", "{a.record_id}" }
                                        StatusText { tone: a.outcome.tone(), text: a.outcome.label().to_string() }
                                    }
                                    div { class: "meta-line",
                                        span { class: "mono", "{a.at}" }
                                        span { "Attempt {a.number}" }
                                        span { "{a.consumer}" }
                                    }
                                    div { class: "insp-sub", "{a.detail}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
