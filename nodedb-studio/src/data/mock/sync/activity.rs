//! Sync activity fixtures, newest first. Every event names the device and
//! subscription the other tabs name, and the rejects match the rejected
//! writes the Origin holds.

use crate::data::mock::sync::devices::t;
use crate::models::sync_activity::{Cursor, Direction, SyncEvent, SyncEventKind};

struct Seed {
    id: u32,
    at: (u32, u32, u32),
    kind: SyncEventKind,
    device: (&'static str, &'static str),
    shape: Option<&'static str>,
    direction: Option<Direction>,
    detail: Option<&'static str>,
}

fn event(seed: Seed) -> SyncEvent {
    SyncEvent {
        id: format!("evt_s{}", seed.id),
        at: t(seed.at.0, seed.at.1, seed.at.2),
        kind: seed.kind,
        device_id: seed.device.0.to_string(),
        device_name: seed.device.1.to_string(),
        database: "analytics".to_string(),
        shape: seed.shape.map(str::to_string),
        direction: seed.direction,
        detail: seed.detail.map(str::to_string),
        received: None,
        applied: None,
        acknowledged: None,
        mutation: None,
        actor: None,
    }
}

pub fn seeded_events() -> Vec<SyncEvent> {
    use Direction::{LiteToOrigin as Up, OriginToLite as Down};
    use SyncEventKind as K;
    let d17 = ("d17", "checkout-ipad");
    let orders = Some("orders_for_store");
    let mut rejected = event(Seed {
        id: 815,
        at: (13, 40, 0),
        kind: K::WriteRejected,
        device: d17,
        shape: orders,
        direction: Some(Up),
        detail: Some("write m104"),
    });
    rejected.mutation = Some("m104".to_string());
    let mut batch = event(Seed {
        id: 812,
        at: (13, 39, 48),
        kind: K::BatchApplied,
        device: d17,
        shape: orders,
        direction: Some(Down),
        detail: Some("cursor 90250"),
    });
    batch.received = Some(Cursor {
        position: 90_250,
        at: t(13, 39, 47),
    });
    batch.applied = Some(Cursor {
        position: 90_250,
        at: t(13, 39, 48),
    });
    let mut ack = event(Seed {
        id: 801,
        at: (13, 38, 7),
        kind: K::AckReceived,
        device: d17,
        shape: orders,
        direction: Some(Down),
        detail: Some("cursor 90238"),
    });
    ack.acknowledged = Some(Cursor {
        position: 90_238,
        at: t(13, 38, 7),
    });
    let mut paused = event(Seed {
        id: 781,
        at: (13, 33, 40),
        kind: K::DevicePaused,
        device: ("d09", "stock-ipad"),
        shape: None,
        direction: None,
        detail: None,
    });
    paused.actor = Some("admin".to_string());
    let mut changed = event(Seed {
        id: 744,
        at: (11, 53, 1),
        kind: K::SubscriptionChanged,
        device: d17,
        shape: Some("inventory_for_store"),
        direction: None,
        detail: Some("paused"),
    });
    changed.actor = Some("admin".to_string());
    let mut rejected_103 = event(Seed {
        id: 702,
        at: (12, 18, 0),
        kind: K::WriteRejected,
        device: ("d08", "front-desk-ipad"),
        shape: Some("customers_for_store"),
        direction: Some(Up),
        detail: Some("write m103"),
    });
    rejected_103.mutation = Some("m103".to_string());
    let mut rejected_102 = event(Seed {
        id: 655,
        at: (10, 3, 0),
        kind: K::WriteRejected,
        device: ("d21", "kiosk-21"),
        shape: orders,
        direction: Some(Up),
        detail: Some("write m102"),
    });
    rejected_102.mutation = Some("m102".to_string());
    vec![
        rejected,
        batch,
        event(Seed {
            id: 809,
            at: (13, 39, 31),
            kind: K::WriteAccepted,
            device: d17,
            shape: orders,
            direction: Some(Up),
            detail: Some("write w_991"),
        }),
        ack,
        event(Seed {
            id: 790,
            at: (13, 36, 20),
            kind: K::Reconnected,
            device: d17,
            shape: None,
            direction: None,
            detail: None,
        }),
        paused,
        event(Seed {
            id: 770,
            at: (13, 18, 6),
            kind: K::Disconnected,
            device: ("lite_003", "lite-kl-03"),
            shape: None,
            direction: None,
            detail: None,
        }),
        rejected_103,
        event(Seed {
            id: 690,
            at: (12, 10, 44),
            kind: K::SnapshotComplete,
            device: d17,
            shape: orders,
            direction: Some(Down),
            detail: Some("128,431 rows"),
        }),
        event(Seed {
            id: 688,
            at: (12, 6, 40),
            kind: K::SnapshotStarted,
            device: d17,
            shape: orders,
            direction: Some(Down),
            detail: None,
        }),
        event(Seed {
            id: 687,
            at: (12, 6, 20),
            kind: K::SchemaAnnounced,
            device: d17,
            shape: orders,
            direction: Some(Up),
            detail: Some("analytics.orders"),
        }),
        event(Seed {
            id: 686,
            at: (12, 6, 10),
            kind: K::SubscriptionStarted,
            device: d17,
            shape: orders,
            direction: None,
            detail: None,
        }),
        changed,
        rejected_102,
    ]
}
