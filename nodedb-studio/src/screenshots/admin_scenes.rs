//! The Administration states the design shows. Each seed names every tab's
//! state, so one scene's selection or open review never leaks into the next.

use crate::data::mock::AdminStore;
use crate::models::admin::AdminChange;
use crate::models::admin_access::AccessChange;
use crate::models::admin_backup::{BackupChange, RestoreRequest};
use crate::models::admin_config::{ConfigScope, ConfigTarget};
use crate::models::admin_policy::PolicyTest;
use crate::models::format::epoch;
use crate::state::admin::{
    AccessUi, AdminSeed, BackupsUi, ConfigUi, CredUi, PolicyUi, Proposed, ReviewState, TestRun,
};

/// analyst selected, with its two direct grants and one inherited.
pub fn access() -> AdminSeed {
    AdminSeed {
        access: Some(AccessUi {
            principal: Some("pr_02".into()),
            ..AccessUi::default()
        }),
        ..AdminSeed::default()
    }
}

/// analyst with a role staged, and the review of it open.
pub fn access_review() -> AdminSeed {
    let staged = vec![AccessChange::AddMembership {
        principal: "pr_02".into(),
        role: "tenant_reader".into(),
        database: "analytics".into(),
    }];
    let store = AdminStore::seeded();
    let change = AdminChange::Access(staged.clone());
    let review = store.preview(&change).ok();
    AdminSeed {
        access: Some(AccessUi {
            principal: Some("pr_02".into()),
            staged,
            review: review.map(|r| ReviewState::new(change, r)),
            ..AccessUi::default()
        }),
        ..AdminSeed::default()
    }
}

/// sales_tenant_read selected, tested as tenant_a with tA.
pub fn row_policies() -> AdminSeed {
    let store = AdminStore::seeded();
    let result = store.test_policy(&PolicyTest {
        database: "analytics".into(),
        collection: "analytics.sales".into(),
        using_expr: "tenant_id = current_setting('app.tenant_id')".into(),
        user: "tenant_a".into(),
        session: vec![("app.tenant_id".into(), "tA".into())],
    });
    AdminSeed {
        policies: Some(PolicyUi {
            selected: Some("pol_01".into()),
            test: result.map(TestRun::Done).unwrap_or_default(),
            ..PolicyUi::default()
        }),
        ..AdminSeed::default()
    }
}

/// audit104 selected on the first page of the last seven days.
pub fn audit() -> AdminSeed {
    AdminSeed {
        audit: Some(crate::state::admin::AuditUi {
            selected: Some("audit104".into()),
            ..Default::default()
        }),
        ..AdminSeed::default()
    }
}

/// nightly-analytics selected, with analytics_restore typed.
pub fn backups() -> AdminSeed {
    AdminSeed {
        backups: Some(BackupsUi {
            selected: Some("snap-2026-06-13-020000-4f8c2d".into()),
            ..BackupsUi::default()
        }),
        ..AdminSeed::default()
    }
}

/// The review of a restore into analytics_restore.
pub fn restore_review() -> AdminSeed {
    let change = AdminChange::Backup(BackupChange::Restore(RestoreRequest {
        snapshot: "snap-2026-06-13-020000-4f8c2d".into(),
        target: "analytics_restore".into(),
        point: epoch(2026, 6, 13, 2, 0, 0),
    }));
    let review = AdminStore::seeded().preview(&change).ok();
    let mut seed = backups();
    if let Some(b) = seed.backups.as_mut() {
        b.review = review.map(|r| ReviewState::new(change, r));
    }
    seed
}

/// node-2 selected, query_timeout proposed at 60s.
pub fn config() -> AdminSeed {
    let target = ConfigTarget {
        scope: ConfigScope::Node,
        node: Some("nd_02".into()),
    };
    AdminSeed {
        config: Some(ConfigUi {
            scope: ConfigScope::Node,
            node: Some("nd_02".into()),
            selected: Some("query_timeout".into()),
            proposed: vec![Proposed {
                target,
                key: "query_timeout".into(),
                value: "60s".into(),
            }],
            ..ConfigUi::default()
        }),
        ..AdminSeed::default()
    }
}

/// search_embeddings selected.
pub fn credentials() -> AdminSeed {
    AdminSeed {
        credentials: Some(CredUi {
            selected: Some("search_embeddings".into()),
            ..CredUi::default()
        }),
        ..AdminSeed::default()
    }
}
