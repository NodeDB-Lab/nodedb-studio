//! What the connected server cannot do, and so what the UI must not offer.
//!
//! One function per decision, over the connection's `Capabilities`. `None`
//! (no connection) offers everything: the gated screens are unreachable then.

use dioxus::prelude::*;

use crate::models::workbench::TxAccess;
use crate::models::workbench::query_plan::ExplainMode;
use crate::routes::destination::Destination;
use crate::routes::tabs::AdminTab;
use crate::state::connection::{ActiveConnection, Capabilities};

/// The active connection's capabilities, read reactively. `None` when
/// disconnected or when no connection context exists.
pub fn use_caps() -> Option<Capabilities> {
    try_use_context::<Signal<Option<ActiveConnection>>>()
        .and_then(|s| s.read().as_ref().map(|c| c.capabilities))
}

/// Whether the connection serves a destination. Unlike the other gates,
/// no connection serves none: a gated screen is never rendered then.
pub fn serves(caps: Option<&Capabilities>, d: Destination) -> bool {
    match d.required_cap() {
        Some(cap) => caps.is_some_and(|c| c.has(cap)),
        None => true,
    }
}

/// Show a record's revision number.
pub fn shows_revisions(caps: Option<&Capabilities>) -> bool {
    caps.is_none_or(|c| c.record_revisions)
}

/// The access modes a transaction can begin with.
pub fn tx_accesses(caps: Option<&Capabilities>) -> &'static [TxAccess] {
    if caps.is_none_or(|c| c.read_only_transactions) {
        &[TxAccess::ReadOnly, TxAccess::ReadWrite]
    } else {
        &[TxAccess::ReadWrite]
    }
}

/// The plan modes the Plan view offers.
pub fn explain_modes(caps: Option<&Capabilities>) -> &'static [ExplainMode] {
    if caps.is_none_or(|c| c.explain_analyze) {
        &[ExplainMode::Explain, ExplainMode::Analyze]
    } else {
        &[ExplainMode::Explain]
    }
}

/// The plan mode to use: `chosen` when offered, else plain EXPLAIN. A tab
/// keeps its mode across a switch to a server without ANALYZE.
pub fn explain_mode(chosen: ExplainMode, caps: Option<&Capabilities>) -> ExplainMode {
    if explain_modes(caps).contains(&chosen) {
        chosen
    } else {
        ExplainMode::Explain
    }
}

/// Offer the array viewer and its actions.
pub fn describes_arrays(caps: Option<&Capabilities>) -> bool {
    caps.is_none_or(|c| c.arrays)
}

/// The Admin tabs this connection offers, in display order.
pub fn admin_tabs(caps: Option<&Capabilities>) -> Vec<AdminTab> {
    AdminTab::ALL
        .iter()
        .copied()
        .filter(|t| match t {
            AdminTab::Access | AdminTab::RowPolicies | AdminTab::Audit => true,
            AdminTab::Backups => caps.is_none_or(|c| c.backups),
            AdminTab::Config => caps.is_none_or(|c| c.config),
            AdminTab::Credentials => caps.is_none_or(|c| c.credentials),
        })
        .collect()
}

/// The tab to show for a route: itself when offered, else the first offered.
pub fn admin_tab(tab: AdminTab, caps: Option<&Capabilities>) -> AdminTab {
    let offered = admin_tabs(caps);
    if offered.contains(&tab) {
        tab
    } else {
        offered.first().copied().unwrap_or_default()
    }
}

/// Every flag on, for tests of views that must not reach `data::mock`.
#[cfg(test)]
pub fn test_caps() -> Capabilities {
    Capabilities {
        graph: true,
        vector: true,
        events: true,
        timeseries: true,
        spatial: true,
        search: true,
        sync: true,
        cluster: true,
        readonly: false,
        record_revisions: true,
        read_only_transactions: true,
        arrays: true,
        backups: true,
        config: true,
        credentials: true,
        explain_analyze: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock;

    fn mock_caps() -> Capabilities {
        mock::connections()
            .into_iter()
            .find_map(|c| c.profile)
            .map(|p| p.capabilities)
            .expect("a mock profile")
    }

    fn bare() -> Capabilities {
        Capabilities {
            record_revisions: false,
            read_only_transactions: false,
            arrays: false,
            backups: false,
            config: false,
            credentials: false,
            explain_analyze: false,
            ..mock_caps()
        }
    }

    #[test]
    fn the_mock_offers_everything() {
        let c = mock_caps();
        assert!(shows_revisions(Some(&c)));
        assert!(describes_arrays(Some(&c)));
        assert_eq!(tx_accesses(Some(&c)).len(), 2);
        assert_eq!(admin_tabs(Some(&c)), AdminTab::ALL);
        assert_eq!(explain_modes(Some(&c)).len(), 2);
        assert_eq!(
            explain_mode(ExplainMode::Analyze, Some(&c)),
            ExplainMode::Analyze
        );
    }

    #[test]
    fn a_server_without_the_features_hides_them() {
        let c = bare();
        assert!(!shows_revisions(Some(&c)));
        assert!(!describes_arrays(Some(&c)));
        assert_eq!(tx_accesses(Some(&c)), &[TxAccess::ReadWrite]);
        assert_eq!(explain_modes(Some(&c)), &[ExplainMode::Explain]);
        assert_eq!(
            explain_mode(ExplainMode::Analyze, Some(&c)),
            ExplainMode::Explain
        );
        assert_eq!(
            admin_tabs(Some(&c)),
            [AdminTab::Access, AdminTab::RowPolicies, AdminTab::Audit]
        );
    }

    #[test]
    fn a_hidden_admin_tab_falls_back_to_the_first_offered() {
        let c = bare();
        assert_eq!(admin_tab(AdminTab::Backups, Some(&c)), AdminTab::Access);
        assert_eq!(admin_tab(AdminTab::Audit, Some(&c)), AdminTab::Audit);
        assert_eq!(
            admin_tab(AdminTab::Config, Some(&mock_caps())),
            AdminTab::Config
        );
    }

    /// With `sync` off no Sync route is served, no palette entry or
    /// notification leads there, and so no `SyncData` request starts: those
    /// requests live only in `views/sync`, which only the Sync route renders.
    #[test]
    fn a_server_without_sync_serves_no_sync_screen() {
        use crate::models::notification::NotificationTarget;
        use crate::routes::Route;
        use crate::routes::tabs::SyncTab;

        let mut c = test_caps();
        c.sync = false;
        for tab in SyncTab::ALL {
            let route = Route::Sync { tab: *tab };
            assert!(!serves(Some(&c), route.destination()), "{route:?}");
            assert!(!serves(None, route.destination()), "{route:?}");
        }
        assert!(!serves(
            Some(&c),
            NotificationTarget::SyncRejectedWrites.route().destination()
        ));
        assert!(serves(Some(&test_caps()), Destination::Sync));
        assert!(serves(Some(&c), Destination::Explorer));
    }

    #[test]
    fn no_connection_offers_everything() {
        assert!(shows_revisions(None));
        assert_eq!(explain_modes(None).len(), 2);
        assert_eq!(admin_tabs(None), AdminTab::ALL);
    }
}
