//! Materialized views page state.

pub const PAGE_SIZE: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewsUi {
    pub selected: Option<String>,
    pub filter: String,
    pub page: usize,
    /// The refresh whose error is open.
    pub open_run: Option<String>,
    pub refreshing: bool,
    pub notice: Option<crate::state::events::feeds::Notice>,
    /// Bumps after a refresh, so reads run again.
    pub epoch: u32,
}
