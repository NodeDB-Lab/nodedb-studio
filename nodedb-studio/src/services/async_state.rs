//! The reusable loading/empty/error UI-state primitive.
//!
//! Kept in plain Rust (no renderer) so the state-mapping logic is unit-
//! testable. Every later wiring phase maps a `use_resource` result into an
//! `AsyncState<T>` via `from_value` and hands it to the `AsyncView` component.

use crate::models::explorer::RecordDetail;
use crate::models::shell::{NavBadges, SessionInfo};
use crate::models::viewers::SubGraph;
use crate::models::workbench::{QueryPlan, ResultSet};
use crate::services::error::StudioError;

/// Anything that can report emptiness, so `from_value` can distinguish a
/// loaded-but-empty result (-> Empty) from a loaded-with-data result.
pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        Vec::is_empty(self)
    }
}

// Single-value seam reads have no "empty" shape: a fetched value is never
// "empty", it either arrived or it errored (`MockBehavior::Empty` folds into
// `Ready` for these — see `record_detail`'s precedent). Each impl is listed
// explicitly, deliberately not a blanket `impl<T> IsEmpty for T`, so a future
// single-value model must opt in here rather than silently inheriting a
// meaning that may not fit it.
impl IsEmpty for SessionInfo {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for NavBadges {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for RecordDetail {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for QueryPlan {
    fn is_empty(&self) -> bool {
        false
    }
}

// `ResultSet` and `SubGraph` are also single fetched values, but unlike the
// four above they wrap a list (rows / nodes) whose emptiness IS a real,
// common outcome — "your query returned no rows" for a workbench, or "this
// collection has no nodes" for a graph viewer. Folding `MockBehavior::Empty`
// into `Ready` for these would make `AsyncState::Empty` unreachable for the
// query and graph panes, so they report their own emptiness instead of the
// blanket `false` above (see `apply_one_or_empty` in `mock_behavior`, which
// gives the mock seam methods a way to actually deliver an empty payload).
impl IsEmpty for ResultSet {
    fn is_empty(&self) -> bool {
        self.rows().is_empty()
    }
}

impl IsEmpty for SubGraph {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// The canonical, unit-tested mapping from a `use_resource` read to the four UI
/// states. Wired views call `from_value` directly (cloning the resource value
/// out of its guard — `StudioError` is `Clone`) and then drive `AsyncView` via
/// the accessors below, so no view re-implements the match arms inline.
///
/// `Clone` allows this to be a Dioxus prop. `PartialEq` is hand-written so we
/// do not depend on `NodeDbError: PartialEq` — errors compare equal by their
/// `Display` output, which is cheap and sufficient for prop diffing.
#[derive(Clone)]
pub enum AsyncState<T> {
    Loading,
    Empty,
    Loaded(T),
    Error(StudioError),
}

impl<T: PartialEq> PartialEq for AsyncState<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AsyncState::Loading, AsyncState::Loading) => true,
            (AsyncState::Empty, AsyncState::Empty) => true,
            (AsyncState::Loaded(a), AsyncState::Loaded(b)) => a == b,
            // Compare errors by Display — avoids requiring NodeDbError: PartialEq.
            (AsyncState::Error(a), AsyncState::Error(b)) => a.to_string() == b.to_string(),
            _ => false,
        }
    }
}

impl<T: IsEmpty> AsyncState<T> {
    /// Pure mapping from a `use_resource` read (`Option<Result<T, StudioError>>`):
    ///   None              -> Loading  (first run not finished)
    ///   Some(Err(e))      -> Error(e)
    ///   Some(Ok(empty))   -> Empty
    ///   Some(Ok(data))    -> Loaded(data)
    ///
    /// To make `Empty` reflect a post-filtered view (e.g. capability filtering),
    /// map the `Ok` payload to the filtered collection *before* calling this.
    pub fn from_value(v: Option<Result<T, StudioError>>) -> Self {
        match v {
            None => AsyncState::Loading,
            Some(Err(e)) => AsyncState::Error(e),
            Some(Ok(t)) if t.is_empty() => AsyncState::Empty,
            Some(Ok(t)) => AsyncState::Loaded(t),
        }
    }
}

impl<T> AsyncState<T> {
    /// True while the first fetch is pending. Feeds `AsyncView`'s `loading` prop.
    pub fn is_loading(&self) -> bool {
        matches!(self, AsyncState::Loading)
    }

    /// True when the fetch resolved to no rows. Feeds `AsyncView`'s `empty` prop.
    pub fn is_empty(&self) -> bool {
        matches!(self, AsyncState::Empty)
    }

    /// `Some(message)` when the fetch failed; `None` otherwise. Feeds
    /// `AsyncView`'s `error` prop (Display string, since `StudioError` is not a
    /// Dioxus-compatible prop type).
    pub fn error_message(&self) -> Option<String> {
        match self {
            AsyncState::Error(e) => Some(e.to_string()),
            _ => None,
        }
    }

    /// Whether the failed fetch is retriable. Feeds `AsyncView`'s `retriable`
    /// prop, which gates the Retry button.
    pub fn is_retriable(&self) -> bool {
        matches!(self, AsyncState::Error(e) if e.is_retriable())
    }

    /// The loaded payload, if any — the caller renders its own markup for it.
    pub fn loaded(&self) -> Option<&T> {
        match self {
            AsyncState::Loaded(t) => Some(t),
            _ => None,
        }
    }

    /// Mutable access to the loaded payload, for in-place updates on a shared
    /// store (e.g. marking notifications read). `None` in any non-loaded state.
    pub fn loaded_mut(&mut self) -> Option<&mut T> {
        match self {
            AsyncState::Loaded(t) => Some(t),
            _ => None,
        }
    }

    /// Project the loaded payload into a view-specific shape, re-deriving `Empty`
    /// when the projection is empty (via `from_value`). Lets a view read a shared
    /// raw store and apply its own filter — e.g. capability filtering — while
    /// keeping `Loading`/`Empty`/`Error` correct. The non-loaded states are
    /// carried through unchanged (`StudioError` is `Clone`).
    pub fn project<U: IsEmpty>(&self, f: impl FnOnce(&T) -> U) -> AsyncState<U> {
        match self {
            AsyncState::Loading => AsyncState::Loading,
            AsyncState::Empty => AsyncState::Empty,
            AsyncState::Error(e) => AsyncState::Error(e.clone()),
            AsyncState::Loaded(t) => AsyncState::from_value(Some(Ok(f(t)))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::explorer::RecordRow;
    use crate::models::viewers::GraphNode;

    #[test]
    fn async_state_none_is_loading() {
        let s = AsyncState::<Vec<u8>>::from_value(None);
        assert!(matches!(s, AsyncState::Loading));
    }

    #[test]
    fn async_state_empty_vec_is_empty() {
        let s = AsyncState::from_value(Some(Ok(Vec::<u8>::new())));
        assert!(matches!(s, AsyncState::Empty));
    }

    #[test]
    fn async_state_nonempty_is_loaded() {
        let s = AsyncState::from_value(Some(Ok(vec![1u8])));
        assert!(matches!(s, AsyncState::Loaded(_)));
    }

    #[test]
    fn async_state_err_is_error() {
        let s = AsyncState::<Vec<u8>>::from_value(Some(Err(StudioError::NotConnected)));
        assert!(matches!(s, AsyncState::Error(_)));
    }

    #[test]
    fn accessors_for_loading() {
        let s = AsyncState::<Vec<u8>>::from_value(None);
        assert!(s.is_loading());
        assert!(!s.is_empty());
        assert_eq!(s.error_message(), None);
        assert!(!s.is_retriable());
        assert!(s.loaded().is_none());
    }

    #[test]
    fn accessors_for_empty() {
        let s = AsyncState::from_value(Some(Ok(Vec::<u8>::new())));
        assert!(s.is_empty());
        assert!(!s.is_loading());
        assert!(s.loaded().is_none());
    }

    #[test]
    fn accessors_for_loaded() {
        let s = AsyncState::from_value(Some(Ok(vec![1u8, 2, 3])));
        assert_eq!(s.loaded(), Some(&vec![1u8, 2, 3]));
        assert!(!s.is_loading());
        assert!(!s.is_empty());
    }

    #[test]
    fn accessors_for_error() {
        // `NotConnected` is non-retriable; its Display message is surfaced.
        let s = AsyncState::<Vec<u8>>::from_value(Some(Err(StudioError::NotConnected)));
        assert!(s.error_message().is_some());
        assert!(!s.is_retriable());
        assert!(!s.is_loading());
    }

    #[test]
    fn loaded_mut_edits_in_place() {
        let mut s = AsyncState::from_value(Some(Ok(vec![1u8, 2, 3])));
        if let Some(v) = s.loaded_mut() {
            v.push(4);
        }
        assert_eq!(s.loaded(), Some(&vec![1u8, 2, 3, 4]));
        // Non-loaded states yield no handle.
        let mut loading = AsyncState::<Vec<u8>>::Loading;
        assert!(loading.loaded_mut().is_none());
    }

    #[test]
    fn project_carries_non_loaded_states() {
        // Loading / Empty / Error pass through unchanged.
        let loading = AsyncState::<Vec<u8>>::Loading;
        assert!(loading.project(|v: &Vec<u8>| v.clone()).is_loading());

        let empty = AsyncState::from_value(Some(Ok(Vec::<u8>::new())));
        assert!(empty.project(|v: &Vec<u8>| v.clone()).is_empty());

        let err = AsyncState::<Vec<u8>>::from_value(Some(Err(StudioError::NotConnected)));
        assert!(
            err.project(|v: &Vec<u8>| v.clone())
                .error_message()
                .is_some()
        );
    }

    #[test]
    fn single_value_seam_models_are_loaded_not_empty() {
        // Before their `IsEmpty` impls existed, these four single-value seam
        // models could not satisfy `AsyncState<T>::from_value`'s `T: IsEmpty`
        // bound at all, so they could never be rendered through `AsyncView`.
        // Each must map a fetched value straight to `Loaded`, never `Empty`:
        // none of the four has a meaningful "empty" shape.
        let session_info = AsyncState::from_value(Some(Ok(SessionInfo {
            database: String::new(),
            role: String::new(),
            server_version: String::new(),
            timezone: String::new(),
            read_only: false,
        })));
        assert!(matches!(session_info, AsyncState::Loaded(_)));

        let nav_badges = AsyncState::from_value(Some(Ok(NavBadges {
            query: 0,
            streams: 0,
        })));
        assert!(matches!(nav_badges, AsyncState::Loaded(_)));

        let record_detail = AsyncState::from_value(Some(Ok(RecordDetail {
            id: String::new(),
            title: String::new(),
            body_json: String::new(),
            footer: String::new(),
        })));
        assert!(matches!(record_detail, AsyncState::Loaded(_)));

        let query_plan = AsyncState::from_value(Some(Ok(QueryPlan {
            text: String::new(),
        })));
        assert!(matches!(query_plan, AsyncState::Loaded(_)));
    }

    #[test]
    fn zero_row_result_set_is_empty() {
        // Unlike the four single-value models above, `ResultSet` wraps a
        // list: a query that returns zero rows is the most common
        // non-error workbench outcome and must reach `AsyncState::Empty`,
        // not `Loaded` with an empty table.
        let result_set = AsyncState::from_value(Some(Ok(ResultSet::new(
            vec!["id".to_string()],
            Vec::new(),
            3,
            "0 rows".to_string(),
        )
        .expect("rectangular"))));
        assert!(matches!(result_set, AsyncState::Empty));
    }

    #[test]
    fn non_empty_result_set_is_loaded() {
        let result_set = AsyncState::from_value(Some(Ok(ResultSet::new(
            vec!["id".to_string()],
            vec![RecordRow {
                id: "1".to_string(),
                cells: vec!["1".to_string()],
            }],
            3,
            "1 row".to_string(),
        )
        .expect("rectangular"))));
        assert!(matches!(result_set, AsyncState::Loaded(_)));
    }

    #[test]
    fn zero_node_sub_graph_is_empty() {
        // A collection with no nodes is a real outcome for the graph viewer
        // and must reach `AsyncState::Empty`, not `Loaded` with an empty
        // graph.
        let sub_graph = AsyncState::from_value(Some(Ok(SubGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        })));
        assert!(matches!(sub_graph, AsyncState::Empty));
    }

    #[test]
    fn non_empty_sub_graph_is_loaded() {
        let sub_graph = AsyncState::from_value(Some(Ok(SubGraph {
            nodes: vec![GraphNode {
                id: "n1".to_string(),
                label: "alice".to_string(),
                x: 0.0,
                y: 0.0,
            }],
            edges: Vec::new(),
        })));
        assert!(matches!(sub_graph, AsyncState::Loaded(_)));
    }

    #[test]
    fn project_filters_loaded_and_redrives_empty() {
        // A filter that keeps elements -> Loaded with the filtered set.
        let s = AsyncState::from_value(Some(Ok(vec![1u8, 2, 3, 4])));
        let evens = s.project(|v: &Vec<u8>| v.iter().copied().filter(|n| n % 2 == 0).collect());
        assert_eq!(evens.loaded(), Some(&vec![2u8, 4]));

        // A filter that drops everything -> re-derives Empty (not Loaded(empty)).
        let s = AsyncState::from_value(Some(Ok(vec![1u8, 3, 5])));
        let evens: AsyncState<Vec<u8>> =
            s.project(|v| v.iter().copied().filter(|n| n % 2 == 0).collect());
        assert!(evens.is_empty());
    }
}
