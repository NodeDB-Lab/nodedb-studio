//! The Events & Jobs states the design shows. Each seed names every tab's
//! state, so one scene's open drawer or running tail never leaks into the
//! next image.

use crate::data::mock::seen_messages;
use crate::models::feed_draft::FeedDraft;
use crate::models::feeds::FeedSession;
use crate::models::listen::ListenSession;
use crate::state::events::EventsSeed;
use crate::state::events::feeds::{FeedEditor, FeedsUi, Tail};
use crate::state::events::jobs::JobsUi;
use crate::state::events::matviews::ViewsUi;
use crate::state::events::notify::NotifyUi;
use crate::state::events::topics::TopicsUi;

/// Every tab at its default, ready to override one.
fn base() -> EventsSeed {
    EventsSeed {
        feeds: Some(FeedsUi::default()),
        topics: Some(TopicsUi::default()),
        views: Some(ViewsUi::default()),
        notify: Some(NotifyUi::default()),
        jobs: Some(JobsUi::default()),
    }
}

/// `sales_changes` selected, with Studio's tail running on it.
pub fn changefeeds() -> EventsSeed {
    let tail = Tail {
        feed: Some("feed_01".into()),
        session: Some(FeedSession {
            feed: "feed_01".into(),
            group: "studio_feed_01".into(),
        }),
        ..Tail::default()
    };
    EventsSeed {
        feeds: Some(FeedsUi {
            selected: Some("feed_01".into()),
            tail,
            ..FeedsUi::default()
        }),
        ..base()
    }
}

/// The Create change feed drawer, filled in as the design shows.
pub fn feed_editor() -> EventsSeed {
    let mut draft = FeedDraft::blank("analytics.sales");
    draft.name = "export_sales".into();
    draft.filter = "store_id = 'nyc'".into();
    draft.projection = "_id, order_id, store_id, amount, status, updated_at".into();
    draft.before_image = true;
    draft.endpoint = "https://warehouse.example.com/webhooks/sales".into();
    draft.alias = "warehouse_sink".into();
    EventsSeed {
        feeds: Some(FeedsUi {
            editor: Some(FeedEditor::create("analytics.sales").with_draft(draft)),
            ..FeedsUi::default()
        }),
        ..base()
    }
}

pub fn topics() -> EventsSeed {
    base()
}

pub fn materialized_views() -> EventsSeed {
    base()
}

/// Studio's session 12 listening on `user_events`, ten notifications shown.
pub fn notifications() -> EventsSeed {
    EventsSeed {
        notify: Some(NotifyUi {
            session: Some(ListenSession {
                id: 12,
                channel: "user_events".into(),
            }),
            shown: seen_messages(),
            ..NotifyUi::default()
        }),
        ..base()
    }
}

pub fn scheduled_jobs() -> EventsSeed {
    base()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::feeds::ChangeOp;

    #[test]
    fn every_seed_names_every_tab() {
        for seed in [
            changefeeds(),
            feed_editor(),
            topics(),
            materialized_views(),
            notifications(),
            scheduled_jobs(),
        ] {
            assert!(
                seed.feeds.is_some()
                    && seed.topics.is_some()
                    && seed.views.is_some()
                    && seed.notify.is_some()
                    && seed.jobs.is_some()
            );
        }
    }

    #[test]
    fn the_editor_seed_is_a_form_the_server_accepts() {
        let seed = feed_editor();
        let editor = seed.feeds.and_then(|f| f.editor).expect("drawer open");
        let feeds = crate::data::mock::seeded_feeds();
        assert!(editor.draft.issues(&feeds, None).is_empty());
        assert_eq!(editor.draft.operations, ChangeOp::ALL);
    }

    #[test]
    fn the_tail_seed_runs_on_the_selected_feed() {
        let feeds = changefeeds().feeds.expect("feeds");
        assert!(feeds.tail.running());
        assert_eq!(feeds.selected.as_deref(), feeds.tail.feed.as_deref());
    }
}
