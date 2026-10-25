//! Topics page state: the selection, the range read, and the three sheets.

use crate::models::topics::{NewTopic, PublishDraft, ResetScope, ResetTarget};

/// What the range inputs hold. The applied range is what the table reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeInput {
    pub partition: u32,
    pub from: String,
    pub to: String,
}

impl Default for RangeInput {
    fn default() -> Self {
        Self {
            partition: 1,
            from: "90".into(),
            to: "120".into(),
        }
    }
}

/// The offset reset review.
#[derive(Debug, Clone, PartialEq)]
pub struct ResetForm {
    pub group: String,
    pub scope: ResetScope,
    pub target: ResetTarget,
    /// The specific offset while it is typed.
    pub offset: String,
    pub busy: bool,
    pub error: Option<String>,
}

impl ResetForm {
    pub fn new(group: &str, scope: ResetScope, target: ResetTarget) -> Self {
        let offset = match target {
            ResetTarget::Offset(n) => n.to_string(),
            ResetTarget::Earliest | ResetTarget::Latest => String::new(),
        };
        Self {
            group: group.to_string(),
            scope,
            target,
            offset,
            busy: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopicSheet {
    pub form: NewTopic,
    pub tried: bool,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishSheet {
    pub topic: String,
    pub draft: PublishDraft,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopicsUi {
    pub selected: Option<String>,
    pub filter: String,
    pub input: RangeInput,
    /// The range the table reads: partition, first offset, last offset.
    pub applied: (u32, u64, u64),
    pub selected_group: Option<String>,
    /// The partition row selected in the group table.
    pub selected_partition: Option<u32>,
    pub selected_message: Option<(u32, u64)>,
    pub reset: Option<ResetForm>,
    pub create: Option<TopicSheet>,
    pub publish: Option<PublishSheet>,
    pub export_open: bool,
    pub notice: Option<crate::state::events::feeds::Notice>,
    /// Bumps after a write, so reads run again.
    pub epoch: u32,
}

impl Default for TopicsUi {
    fn default() -> Self {
        Self {
            selected: None,
            filter: String::new(),
            input: RangeInput::default(),
            // The first read the design shows: partition 1, offsets 90 to 120.
            applied: (1, 90, 120),
            selected_group: None,
            selected_partition: None,
            selected_message: None,
            reset: None,
            create: None,
            publish: None,
            export_open: false,
            notice: None,
            epoch: 0,
        }
    }
}

impl TopicsUi {
    /// Select topic `id`. Its range starts at the first 500 offsets of
    /// partition 0, since another topic's offsets mean nothing here.
    pub fn select(&mut self, id: &str) {
        self.selected = Some(id.to_string());
        self.selected_group = None;
        self.selected_partition = None;
        self.selected_message = None;
        self.input = RangeInput {
            partition: 0,
            from: "0".into(),
            to: "499".into(),
        };
        self.applied = (0, 0, 499);
        self.reset = None;
        self.notice = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_read_is_the_design_range() {
        assert_eq!(TopicsUi::default().applied, (1, 90, 120));
    }

    #[test]
    fn selecting_another_topic_resets_what_belonged_to_the_last() {
        let mut ui = TopicsUi {
            selected_group: Some("grp_billing".into()),
            selected_message: Some((1, 104)),
            ..TopicsUi::default()
        };
        ui.select("topic_02");
        assert_eq!(ui.selected.as_deref(), Some("topic_02"));
        assert_eq!((ui.selected_group, ui.selected_message), (None, None));
        assert_eq!(ui.applied, (0, 0, 499));
    }
}
