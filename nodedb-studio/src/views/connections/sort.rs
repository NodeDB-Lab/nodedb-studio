//! Sorting the saved-connections table.

use crate::state::connections_registry::SavedConnection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Name,
    Target,
    Mode,
    LastUsed,
    Status,
}

impl Column {
    pub const ALL: [Column; 5] = [
        Column::Name,
        Column::Target,
        Column::Mode,
        Column::LastUsed,
        Column::Status,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Column::Name => "Name",
            Column::Target => "Target",
            Column::Mode => "Mode",
            Column::LastUsed => "Last used",
            Column::Status => "Status",
        }
    }
}

/// The column the table is sorted by and the direction. No sort keeps the
/// registry order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub ascending: bool,
}

/// A click on a header: a new column sorts ascending, the same column flips
/// the direction.
pub fn clicked(current: Option<Sort>, column: Column) -> Sort {
    match current {
        Some(s) if s.column == column => Sort {
            column,
            ascending: !s.ascending,
        },
        Some(_) | None => Sort {
            column,
            ascending: true,
        },
    }
}

/// `list` in the order of `sort`. Equal entries keep their registry order.
pub fn sorted(list: &[SavedConnection], sort: Option<Sort>) -> Vec<SavedConnection> {
    let mut out = list.to_vec();
    let Some(sort) = sort else {
        return out;
    };
    out.sort_by(|a, b| {
        let ord = match sort.column {
            Column::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            Column::Target => a.target.summary().cmp(&b.target.summary()),
            Column::Mode => a.kind().label().cmp(b.kind().label()),
            Column::LastUsed => a.last_used.cmp(&b.last_used),
            Column::Status => a.status.label().cmp(b.status.label()),
        };
        if sort.ascending { ord } else { ord.reverse() }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::connection_service::ConnectionService;

    /// The saved-connection registry, read through the seam as the app does.
    async fn registry() -> Vec<SavedConnection> {
        MockConnectionService::ready()
            .list_connections()
            .await
            .expect("mock registry")
    }

    fn names(list: &[SavedConnection]) -> Vec<&str> {
        list.iter().map(|c| c.name.as_str()).collect()
    }

    #[tokio::test]
    async fn no_sort_keeps_the_registry_order() {
        let saved = registry().await;
        assert_eq!(names(&sorted(&saved, None)), names(&saved));
    }

    #[tokio::test]
    async fn name_sort_ignores_case_and_flips() {
        let saved = registry().await;
        let asc = sorted(
            &saved,
            Some(Sort {
                column: Column::Name,
                ascending: true,
            }),
        );
        assert_eq!(
            names(&asc),
            [
                "Local development",
                "Production",
                "staging-cluster",
                "test-cluster"
            ]
        );
        let desc = sorted(
            &saved,
            Some(Sort {
                column: Column::Name,
                ascending: false,
            }),
        );
        assert_eq!(names(&desc)[0], "test-cluster");
    }

    #[tokio::test]
    async fn last_used_sorts_by_time() {
        let saved = registry().await;
        let desc = sorted(
            &saved,
            Some(Sort {
                column: Column::LastUsed,
                ascending: false,
            }),
        );
        assert_eq!(names(&desc)[0], "staging-cluster");
        assert_eq!(names(&desc)[3], "test-cluster");
    }

    #[test]
    fn a_click_picks_a_column_then_flips_it() {
        let first = clicked(None, Column::Name);
        assert_eq!(
            first,
            Sort {
                column: Column::Name,
                ascending: true
            }
        );
        let flipped = clicked(Some(first), Column::Name);
        assert!(!flipped.ascending);
        let other = clicked(Some(flipped), Column::Mode);
        assert_eq!(
            other,
            Sort {
                column: Column::Mode,
                ascending: true
            }
        );
    }
}
