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
}
