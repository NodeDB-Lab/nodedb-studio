//! The reusable loading/empty/error UI-state primitive.
//!
//! Kept in plain Rust (no renderer) so the state-mapping logic is unit-
//! testable. Every later wiring phase maps a `use_resource` result into an
//! `AsyncState<T>` via `from_value` and hands it to the `AsyncView` component.

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

/// The canonical, unit-tested mapping from a `use_resource` read to the four UI
/// states. Wired views call `from_value` directly (cloning the resource value
/// out of its guard — `StudioError` is `Clone`) and then drive `AsyncView` via
/// the accessors below, so no view re-implements the match arms inline.
pub enum AsyncState<T> {
    Loading,
    Empty,
    Loaded(T),
    Error(StudioError),
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
