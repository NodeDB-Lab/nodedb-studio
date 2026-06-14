//! The reusable loading/empty/error UI-state primitive.
//!
//! Kept in plain Rust (no renderer) so the state-mapping logic is unit-
//! testable. Every later wiring phase maps a `use_resource` result into an
//! `AsyncState<T>` via `from_value` and hands it to the `AsyncView` component.

use crate::services::error::StudioError;

/// Anything that can report emptiness, so `from_value` can distinguish a
/// loaded-but-empty result (-> Empty) from a loaded-with-data result.
// Scoped allow: see the note on `AsyncState` below. `IsEmpty` exists only to
// bound `from_value`, which is exercised by tests + mirrored (not called) in the
// popover render path, so the non-test build sees it as unused.
#[allow(dead_code)]
pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        Vec::is_empty(self)
    }
}

// Scoped allow: `AsyncState`/`from_value` are the canonical, unit-tested mapping
// from a `use_resource` read to the four UI states. The 01-04 popover render path
// uses approach (A) from the plan — it MIRRORS these four match arms inline
// rather than CALLING `from_value`, because `StudioError` is not `Clone` and
// cannot be owned out of a `Resource` read guard for the Error arm. `from_value`
// is therefore referenced only by tests, so the non-test build flags it (and the
// enum it returns) as dead. The allow is kept deliberately; phases 3-6 that can
// own their resource values WILL call `from_value` directly, removing the need.
#[allow(dead_code)]
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
    // Scoped allow: see the note on `AsyncState` above (test-only call site this
    // phase; the popover mirrors these arms inline because StudioError is not Clone).
    #[allow(dead_code)]
    pub fn from_value(v: Option<Result<T, StudioError>>) -> Self {
        match v {
            None => AsyncState::Loading,
            Some(Err(e)) => AsyncState::Error(e),
            Some(Ok(t)) if t.is_empty() => AsyncState::Empty,
            Some(Ok(t)) => AsyncState::Loaded(t),
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
}
