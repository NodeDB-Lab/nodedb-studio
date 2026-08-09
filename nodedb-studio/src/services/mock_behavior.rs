//! Drives which result every mock seam method returns, so all four async
//! states are reachable on every screen rather than only on CDC.

use std::time::Duration;

use crate::services::error::StudioError;

/// Which result the mock produces. `Delayed` exists so tests can observe the
/// Loading state and catch guards held across an await.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MockBehavior {
    #[default]
    Ready,
    Empty,
    Erroring,
    Delayed(Duration),
}

/// Apply the behaviour to a fixture thunk. Every mock seam method is a
/// one-liner over this, which is what keeps the four states uniform.
pub async fn apply<T>(
    behavior: MockBehavior,
    ready: impl FnOnce() -> Vec<T>,
) -> Result<Vec<T>, StudioError> {
    match behavior {
        MockBehavior::Ready => Ok(ready()),
        MockBehavior::Empty => Ok(Vec::new()),
        MockBehavior::Erroring => Err(StudioError::from(
            nodedb_client::NodeDbError::node_unreachable("mock"),
        )),
        MockBehavior::Delayed(d) => {
            tokio::time::sleep(d).await;
            Ok(ready())
        }
    }
}

/// Apply the behaviour to a single-value fixture thunk (used by seam methods
/// that read one value with no "empty" shape at all — `session_info`,
/// `nav_badges`, `record_detail`, `explain`). `Ready` and `Empty` both call
/// the thunk: a fetched single value here is never "empty", it either
/// arrived or it errored, so `Empty` folds into `Ready` rather than
/// inventing an absent value.
pub async fn apply_one<T>(
    behavior: MockBehavior,
    ready: impl FnOnce() -> T,
) -> Result<T, StudioError> {
    match behavior {
        MockBehavior::Ready | MockBehavior::Empty => Ok(ready()),
        MockBehavior::Erroring => Err(StudioError::from(
            nodedb_client::NodeDbError::node_unreachable("mock"),
        )),
        MockBehavior::Delayed(d) => {
            tokio::time::sleep(d).await;
            Ok(ready())
        }
    }
}

/// Apply the behaviour to a single-value fixture thunk whose value can
/// itself be "empty" (used by `run_query` and `sub_graph`: a zero-row result
/// set or a zero-node graph is a real, common outcome, not an absent value).
/// Unlike `apply_one`, `Empty` calls `empty` rather than `ready`, so the
/// caller controls exactly what the empty shape looks like.
pub async fn apply_one_or_empty<T>(
    behavior: MockBehavior,
    ready: impl FnOnce() -> T,
    empty: impl FnOnce() -> T,
) -> Result<T, StudioError> {
    match behavior {
        MockBehavior::Ready => Ok(ready()),
        MockBehavior::Empty => Ok(empty()),
        MockBehavior::Erroring => Err(StudioError::from(
            nodedb_client::NodeDbError::node_unreachable("mock"),
        )),
        MockBehavior::Delayed(d) => {
            tokio::time::sleep(d).await;
            Ok(ready())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ready_returns_the_fixture() {
        let out = apply(MockBehavior::Ready, || vec![1u8, 2]).await;
        assert_eq!(out.expect("ready yields data"), vec![1u8, 2]);
    }

    #[tokio::test]
    async fn empty_returns_no_rows() {
        let out = apply(MockBehavior::Empty, || vec![1u8, 2]).await;
        assert!(out.expect("empty yields Ok").is_empty());
    }

    #[tokio::test]
    async fn erroring_returns_a_retriable_error() {
        let out = apply(MockBehavior::Erroring, || vec![1u8]).await;
        let err = out.expect_err("erroring yields Err");
        assert!(
            err.is_retriable(),
            "demo error must exercise the retry path"
        );
    }

    #[tokio::test]
    async fn delayed_still_returns_the_fixture() {
        let out = apply(MockBehavior::Delayed(Duration::from_millis(5)), || {
            vec![9u8]
        })
        .await;
        assert_eq!(out.expect("delayed yields data"), vec![9u8]);
    }

    #[tokio::test]
    async fn apply_one_ready_returns_the_fixture() {
        let out = apply_one(MockBehavior::Ready, || 7u8).await;
        assert_eq!(out.expect("ready yields data"), 7u8);
    }

    #[tokio::test]
    async fn apply_one_empty_still_returns_the_fixture() {
        // Unlike `apply`, a single value has no "empty" shape: `Empty` folds
        // into `Ready` rather than inventing an absent value.
        let out = apply_one(MockBehavior::Empty, || 7u8).await;
        assert_eq!(out.expect("empty folds into ready"), 7u8);
    }

    #[tokio::test]
    async fn apply_one_erroring_returns_a_retriable_error() {
        let out = apply_one(MockBehavior::Erroring, || 7u8).await;
        let err = out.expect_err("erroring yields Err");
        assert!(
            err.is_retriable(),
            "demo error must exercise the retry path"
        );
    }

    #[tokio::test]
    async fn apply_one_delayed_still_returns_the_fixture() {
        let out = apply_one(MockBehavior::Delayed(Duration::from_millis(5)), || 9u8).await;
        assert_eq!(out.expect("delayed yields data"), 9u8);
    }

    #[tokio::test]
    async fn apply_one_or_empty_ready_returns_the_fixture() {
        let out = apply_one_or_empty(MockBehavior::Ready, || 7u8, || 0u8).await;
        assert_eq!(out.expect("ready yields data"), 7u8);
    }

    #[tokio::test]
    async fn apply_one_or_empty_empty_returns_the_empty_thunk() {
        // Unlike `apply_one`, `Empty` does NOT fold into `Ready` here: the
        // caller's `empty` thunk runs instead, so a genuinely empty payload
        // is reachable for single-value reads that have a real empty shape.
        let out = apply_one_or_empty(MockBehavior::Empty, || 7u8, || 0u8).await;
        assert_eq!(out.expect("empty yields the empty thunk"), 0u8);
    }

    #[tokio::test]
    async fn apply_one_or_empty_erroring_returns_a_retriable_error() {
        let out = apply_one_or_empty(MockBehavior::Erroring, || 7u8, || 0u8).await;
        let err = out.expect_err("erroring yields Err");
        assert!(
            err.is_retriable(),
            "demo error must exercise the retry path"
        );
    }

    #[tokio::test]
    async fn apply_one_or_empty_delayed_still_returns_the_fixture() {
        let out = apply_one_or_empty(
            MockBehavior::Delayed(Duration::from_millis(5)),
            || 9u8,
            || 0u8,
        )
        .await;
        assert_eq!(out.expect("delayed yields data"), 9u8);
    }
}
