//! The five tabs' state, provided by `App` above the router so a tab keeps
//! its selection, filter, and open sheet when the user leaves the page.

use dioxus::prelude::*;

use crate::state::events::feeds::FeedsUi;
use crate::state::events::jobs::JobsUi;
use crate::state::events::matviews::ViewsUi;
use crate::state::events::notify::NotifyUi;
use crate::state::events::topics::TopicsUi;

#[derive(Clone, Copy)]
pub struct EventsUi {
    pub feeds: Signal<FeedsUi>,
    pub topics: Signal<TopicsUi>,
    pub views: Signal<ViewsUi>,
    pub notify: Signal<NotifyUi>,
    pub jobs: Signal<JobsUi>,
}

impl EventsUi {
    /// Create the state and provide it to the tree below the caller.
    pub fn provide() -> Self {
        use_context_provider(|| Self {
            feeds: Signal::new(FeedsUi::default()),
            topics: Signal::new(TopicsUi::default()),
            views: Signal::new(ViewsUi::default()),
            notify: Signal::new(NotifyUi::default()),
            jobs: Signal::new(JobsUi::default()),
        })
    }
}

/// What a screenshot scene sets. `None` keeps a tab's own state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventsSeed {
    pub feeds: Option<FeedsUi>,
    pub topics: Option<TopicsUi>,
    pub views: Option<ViewsUi>,
    pub notify: Option<NotifyUi>,
    pub jobs: Option<JobsUi>,
}

impl EventsUi {
    /// Replace the tabs' state with a seed's.
    pub fn apply(mut self, seed: EventsSeed) {
        if let Some(s) = seed.feeds {
            self.feeds.set(s);
        }
        if let Some(s) = seed.topics {
            self.topics.set(s);
        }
        if let Some(s) = seed.views {
            self.views.set(s);
        }
        if let Some(s) = seed.notify {
            self.notify.set(s);
        }
        if let Some(s) = seed.jobs {
            self.jobs.set(s);
        }
    }
}
