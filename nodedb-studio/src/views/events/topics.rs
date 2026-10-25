//! Topics: durable messages, partitions, and consumer groups.
//!
//! A topic keeps its messages for the retention window and replays them. A
//! notification does not. Offsets belong to a partition of one topic, and a
//! reset opens a review before it moves any of them.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::topics::{ConsumerGroup, NewTopic, Topic};
use crate::routes::tabs::EventsTab;
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::topics::TopicSheet;
use crate::state::workspace::{Surface, Workspace};
use crate::views::events::common::NoticeLine;
use crate::views::events::topic_groups::{GroupPanel, TopicDetails};
use crate::views::events::topic_messages::MessagesPanel;
use crate::views::events::topic_sheets::{CreateTopicSheet, PublishSheetView, ResetSheet};

pub const SURFACE: Surface = Surface::Events(EventsTab::Topics);
const LIST_SIZE: PanelSize = PanelSize::width(250.0, 220.0, 420.0);
const SIDE_SIZE: PanelSize = PanelSize::width(350.0, 320.0, 560.0);
const DETAILS_SIZE: PanelSize = PanelSize::height(300.0, 220.0, 480.0);

/// Topics whose name contains `filter`, ignoring case.
pub fn matching<'a>(topics: &'a [Topic], filter: &str) -> Vec<&'a Topic> {
    let needle = filter.trim().to_lowercase();
    topics
        .iter()
        .filter(|t| needle.is_empty() || t.name.to_lowercase().contains(&needle))
        .collect()
}

/// `3 partitions · 7 days`.
pub fn topic_meta(t: &Topic) -> String {
    format!(
        "{} partition{} · {} day{}",
        t.partitions,
        if t.partitions == 1 { "" } else { "s" },
        t.retention_days,
        if t.retention_days == 1 { "" } else { "s" }
    )
}

#[component]
pub fn TopicsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().topics;
    let layout = use_context::<Signal<Workspace>>();
    let epoch = use_memo(move || ui.read().epoch);

    let mut topics = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.topics().await }
        })
    };
    let state = resource_state(&topics);
    let list: Vec<Topic> = state.loaded().cloned().unwrap_or_default();

    let selected_id = use_memo(move || {
        let chosen = ui.read().selected.clone();
        let read = topics.read();
        let list = read.as_ref().and_then(|r| r.as_ref().ok())?;
        chosen
            .filter(|id| list.iter().any(|t| &t.id == id))
            .or_else(|| list.first().map(|t| t.id.clone()))
    });
    let selected: Option<Topic> = selected_id
        .read()
        .as_ref()
        .and_then(|id| list.iter().find(|t| &t.id == id).cloned());

    let mut groups = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let (_conn, _epoch, id) = (conn(), epoch(), selected_id());
            async move {
                match id {
                    Some(id) => backend.consumer_groups(&id).await,
                    None => Ok(Vec::new()),
                }
            }
        })
    };
    let group_state = resource_state(&groups);
    let group_list: Vec<ConsumerGroup> = group_state.loaded().cloned().unwrap_or_default();

    let (filter, notice, reset, create, publish, chosen_group) = {
        let u = ui.read();
        (
            u.filter.clone(),
            u.notice.clone(),
            u.reset.clone(),
            u.create.clone(),
            u.publish.clone(),
            u.selected_group.clone(),
        )
    };
    let group: Option<ConsumerGroup> = chosen_group
        .and_then(|id| group_list.iter().find(|g| g.id == id).cloned())
        .or_else(|| group_list.first().cloned());
    let rows = matching(&list, &filter);
    let side_style = region_style(&layout.read(), SURFACE, "side", SIDE_SIZE);

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                Panel {
                    surface: SURFACE,
                    id: "topics",
                    title: "Topics",
                    size: LIST_SIZE,
                    actions: rsx! {
                        button {
                            class: "btn sm primary",
                            title: "Create a durable topic",
                            onclick: move |_| {
                                ui.write().create = Some(TopicSheet {
                                    form: NewTopic::blank(),
                                    tried: false,
                                    busy: false,
                                    error: None,
                                });
                            },
                            "Create topic"
                        }
                    },
                    div { class: "toolbar nowrap",
                        div { class: "search-field grow",
                            Ico { icon: Icon::Search, size: 16 }
                            input {
                                class: "input",
                                placeholder: "Filter topics…",
                                "aria-label": "Filter topics",
                                value: "{filter}",
                                oninput: move |e| ui.write().filter = e.value(),
                            }
                        }
                    }
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| topics.restart(),
                        empty_message: "This database has no topics. Create one with Create topic.",
                    }
                    if state.loaded().is_some() {
                        div { class: "list topic-list",
                            for t in rows.iter() {
                                {
                                    let id = t.id.clone();
                                    let is_selected = selected_id.read().as_deref() == Some(t.id.as_str());
                                    rsx! {
                                        button {
                                            key: "{t.id}",
                                            class: if is_selected { "list-item selected" } else { "list-item" },
                                            "aria-selected": "{is_selected}",
                                            onclick: move |_| ui.write().select(&id),
                                            span { class: "status-dot ok", title: "Active" }
                                            span { class: "row-text",
                                                span { class: "row-name", "{t.name}" }
                                                span { class: "row-sub", "{topic_meta(t)}" }
                                            }
                                            Ico { icon: Icon::ChevronRight, size: 16 }
                                        }
                                    }
                                }
                            }
                            if rows.is_empty() {
                                div { class: "async-empty", "No topic matches the filter." }
                            }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "topics", size: LIST_SIZE, side: Side::Before }
                Panel {
                    surface: SURFACE,
                    id: "messages",
                    title: match selected.as_ref() {
                        Some(t) => format!("Messages — {}", t.name),
                        None => "Messages".to_string(),
                    },
                    class: "fill",
                    NoticeLine { notice, on_dismiss: move |_| ui.write().notice = None }
                    if let Some(t) = selected.clone() {
                        MessagesPanel { topic: t }
                    } else {
                        div { class: "async-empty", "Select a topic to read its messages." }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "side", size: SIDE_SIZE, side: Side::After }
                div { class: "workbench col side-col", style: "{side_style}",
                    Panel {
                        surface: SURFACE,
                        id: "groups",
                        title: match group.as_ref() {
                            Some(g) => format!("Consumer group — {}", g.name),
                            None => "Consumer group".to_string(),
                        },
                        class: "fill",
                        GroupPanel {
                            groups: group_list.clone(),
                            group: group.clone(),
                            loading: group_state.is_loading(),
                            error: group_state.error_message(),
                            on_retry: move |_| groups.restart(),
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "details",
                        title: match selected.as_ref() {
                            Some(t) => format!("Topic details — {}", t.name),
                            None => "Topic details".to_string(),
                        },
                        size: DETAILS_SIZE,
                        if let Some(t) = selected.clone() {
                            TopicDetails { topic: t, group: group.clone() }
                        }
                    }
                }
                if let Some(r) = reset {
                    ResetSheet { form: r, group: group.clone() }
                }
                if let Some(c) = create {
                    CreateTopicSheet { sheet: c, topics: list.clone() }
                }
                if let Some(p) = publish {
                    PublishSheetView { sheet: p, topic: selected.clone(), partitions: selected.as_ref().map_or(0, |t| t.partitions) }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::topic_data::TopicData;

    #[tokio::test]
    async fn the_filter_matches_names_ignoring_case() {
        let topics = MockConnectionService::ready()
            .topics()
            .await
            .expect("topics");
        assert_eq!(matching(&topics, "").len(), 2);
        assert_eq!(matching(&topics, "ORDERS").len(), 1);
        assert!(matching(&topics, "nope").is_empty());
    }

    #[tokio::test]
    async fn the_meta_line_matches_the_design() {
        let topics = MockConnectionService::ready()
            .topics()
            .await
            .expect("topics");
        assert_eq!(topic_meta(&topics[0]), "3 partitions · 7 days");
        assert_eq!(topic_meta(&topics[1]), "6 partitions · 1 day");
    }
}
