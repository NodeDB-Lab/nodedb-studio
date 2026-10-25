//! The Sync states the design shows. Each seed names every tab's state, so
//! one scene's selection or open sheet never leaks into the next image.

use crate::data::mock::SyncStore;
use crate::models::sync_enroll::EnrollDraft;
use crate::models::sync_sub::SubChange;
use crate::state::sync::{
    DevicesUi, RejectUi, RepairSheet, SubDraft, SubTab, SubsUi, SyncActivityUi, SyncSeed,
};

/// lite-sg-07 selected, every device listed.
pub fn devices() -> SyncSeed {
    SyncSeed::default()
}

/// The Enroll Lite device sheet, filled in and not yet created.
pub fn enrollment() -> SyncSeed {
    let options = SyncStore::seeded().enroll_options();
    let mut draft = EnrollDraft::new(&options);
    draft.name = "Checkout iPad".into();
    SyncSeed {
        devices: Some(DevicesUi {
            enroll: Some(draft),
            ..DevicesUi::default()
        }),
        ..SyncSeed::default()
    }
}

/// d17's subscriptions, orders_for_store selected.
pub fn subscriptions() -> SyncSeed {
    SyncSeed {
        subs: Some(SubsUi {
            device: Some("d17".into()),
            selected: Some("sub_17_orders".into()),
            ..SubsUi::default()
        }),
        ..SyncSeed::default()
    }
}

/// orders_for_store with its store parameter edited, on Review change.
pub fn subscription_review() -> SyncSeed {
    let subs = SyncStore::seeded().subscriptions();
    let change = subs.get("sub_17_orders").map(|s| {
        let mut c: SubChange = s.as_change();
        if let Some(p) = c.parameters.first_mut() {
            p.1 = "sf".into();
        }
        c
    });
    let mut seed = subscriptions();
    if let (Some(ui), Some(change)) = (seed.subs.as_mut(), change) {
        ui.tab = SubTab::ReviewChange;
        ui.draft = Some(SubDraft {
            id: "sub_17_orders".into(),
            change,
        });
    }
    seed
}

/// m104 selected, with the Origin's record open beside the device's.
pub fn rejected_writes() -> SyncSeed {
    SyncSeed {
        rejects: Some(RejectUi {
            selected: Some("m104".into()),
            existing_open: true,
            ..RejectUi::default()
        }),
        ..SyncSeed::default()
    }
}

/// The edit-and-resubmit sheet over m104.
pub fn reject_edit() -> SyncSeed {
    let rows = SyncStore::seeded().rejected();
    let text = rows
        .get("m104")
        .map(|r| crate::models::record_json::pretty(&r.payload, &[]))
        .unwrap_or_default();
    SyncSeed {
        rejects: Some(RejectUi {
            selected: Some("m104".into()),
            sheet: Some(RepairSheet::Edit {
                mutation: "m104".into(),
                revision: 1,
                text,
                busy: false,
                error: None,
            }),
            ..RejectUi::default()
        }),
        ..SyncSeed::default()
    }
}

/// The log with the Batch applied event selected.
pub fn activity() -> SyncSeed {
    SyncSeed {
        activity: Some(SyncActivityUi {
            selected: Some("evt_s812".into()),
            ..SyncActivityUi::default()
        }),
        ..SyncSeed::default()
    }
}
