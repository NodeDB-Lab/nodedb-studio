//! Subscription fixtures. d17 (checkout-ipad) holds the three store shapes
//! the design shows. Other devices hold their own, so the Subscriptions tab
//! has something to filter.

use crate::data::mock::sync::devices::t;
use crate::models::sync_sub::{
    ShapeKind, Snapshot, SubEvent, SubEventKind, SubState, Subscription,
};

/// Rows a store's orders hold. The server estimates it. An unknown store
/// matches none.
pub fn store_rows(store: &str) -> u64 {
    match store {
        "nyc" => 128_431,
        "sf" => 96_204,
        "la" => 88_112,
        "chi" => 71_950,
        _ => 0,
    }
}

fn ev(h: u32, m: u32, s: u32, kind: SubEventKind, count: Option<u64>) -> SubEvent {
    SubEvent {
        at: t(h, m, s),
        kind,
        count,
    }
}

/// Hour, minute, second.
type Clock = (u32, u32, u32);

struct Seed {
    id: &'static str,
    name: &'static str,
    device: (&'static str, &'static str),
    collection: &'static str,
    filter: &'static str,
    parameter: (&'static str, &'static str),
    state: SubState,
    snapshot: Snapshot,
    checkpoint: Option<u64>,
    seen: Option<(Clock, Clock)>,
    events: Vec<SubEvent>,
}

fn sub(seed: Seed) -> Subscription {
    Subscription {
        id: seed.id.to_string(),
        name: seed.name.to_string(),
        device_id: seed.device.0.to_string(),
        device_name: seed.device.1.to_string(),
        shape: ShapeKind::FilteredRecordSet,
        database: "analytics".to_string(),
        collection: seed.collection.to_string(),
        filter: seed.filter.to_string(),
        parameters: vec![(seed.parameter.0.to_string(), seed.parameter.1.to_string())],
        permission_scope: "tenant_a".to_string(),
        preset: seed
            .filter
            .starts_with("store_id")
            .then(|| "StoreOrders".to_string()),
        state: seed.state,
        snapshot: seed.snapshot,
        checkpoint: seed.checkpoint,
        last_received: seed.seen.map(|(r, _)| t(r.0, r.1, r.2)),
        last_applied: seed.seen.map(|(_, a)| t(a.0, a.1, a.2)),
        revision: 3,
        events: seed.events,
    }
}

pub fn seeded_subs() -> Vec<Subscription> {
    use SubEventKind as K;
    let store = "store_id = :store";
    vec![
        sub(Seed {
            id: "sub_17_orders",
            name: "orders_for_store",
            device: ("d17", "checkout-ipad"),
            collection: "orders",
            filter: store,
            parameter: ("store", "nyc"),
            state: SubState::Active,
            snapshot: Snapshot::Complete { rows: 128_431 },
            checkpoint: Some(90_238),
            seen: Some(((13, 39, 31), (13, 39, 32))),
            events: vec![
                ev(13, 39, 32, K::AppliedLive, Some(12)),
                ev(13, 39, 31, K::ReceivedLive, Some(12)),
                ev(13, 32, 5, K::CheckpointUpdated, Some(90_238)),
                ev(12, 10, 44, K::SnapshotComplete, Some(128_431)),
                ev(12, 8, 12, K::ApplyingSnapshot, None),
                ev(12, 6, 40, K::StartingSnapshot, None),
                ev(12, 6, 10, K::Activated, None),
            ],
        }),
        sub(Seed {
            id: "sub_17_customers",
            name: "customers_for_store",
            device: ("d17", "checkout-ipad"),
            collection: "customers",
            filter: store,
            parameter: ("store", "nyc"),
            state: SubState::Active,
            snapshot: Snapshot::Complete { rows: 18_204 },
            checkpoint: Some(90_201),
            seen: Some(((13, 38, 2), (13, 38, 2))),
            events: vec![
                ev(13, 38, 2, K::AppliedLive, Some(3)),
                ev(13, 38, 2, K::ReceivedLive, Some(3)),
                ev(12, 9, 30, K::SnapshotComplete, Some(18_204)),
                ev(12, 6, 12, K::Activated, None),
            ],
        }),
        sub(Seed {
            id: "sub_17_inventory",
            name: "inventory_for_store",
            device: ("d17", "checkout-ipad"),
            collection: "inventory",
            filter: store,
            parameter: ("store", "nyc"),
            state: SubState::Paused,
            snapshot: Snapshot::Complete { rows: 4_812 },
            checkpoint: Some(88_117),
            seen: Some(((11, 52, 40), (11, 52, 40))),
            events: vec![
                ev(11, 53, 1, K::CheckpointUpdated, Some(88_117)),
                ev(10, 14, 20, K::SnapshotComplete, Some(4_812)),
                ev(10, 13, 55, K::Activated, None),
            ],
        }),
        sub(Seed {
            id: "sub_007_sales",
            name: "sales_apac",
            device: ("lite_007", "lite-sg-07"),
            collection: "sales",
            filter: "region = :region",
            parameter: ("region", "APAC"),
            state: SubState::Active,
            snapshot: Snapshot::Complete { rows: 402_918 },
            checkpoint: Some(240_188),
            seen: Some(((13, 40, 3), (13, 40, 3))),
            events: vec![
                ev(13, 40, 3, K::AppliedLive, Some(41)),
                ev(13, 40, 3, K::ReceivedLive, Some(41)),
                ev(9, 2, 18, K::SnapshotComplete, Some(402_918)),
                ev(9, 0, 3, K::Activated, None),
            ],
        }),
        sub(Seed {
            id: "sub_007_customers",
            name: "customers_apac",
            device: ("lite_007", "lite-sg-07"),
            collection: "customers",
            filter: "region = :region",
            parameter: ("region", "APAC"),
            state: SubState::Active,
            snapshot: Snapshot::Complete { rows: 61_377 },
            checkpoint: Some(240_131),
            seen: Some(((13, 39, 12), (13, 39, 12))),
            events: vec![
                ev(13, 39, 12, K::AppliedLive, Some(2)),
                ev(13, 39, 12, K::ReceivedLive, Some(2)),
                ev(9, 1, 40, K::SnapshotComplete, Some(61_377)),
                ev(9, 0, 3, K::Activated, None),
            ],
        }),
        sub(Seed {
            id: "sub_08_orders",
            name: "orders_for_store",
            device: ("d08", "front-desk-ipad"),
            collection: "orders",
            filter: store,
            parameter: ("store", "sf"),
            state: SubState::Active,
            snapshot: Snapshot::Complete { rows: 96_204 },
            checkpoint: Some(90_190),
            seen: Some(((13, 39, 3), (13, 39, 3))),
            events: vec![
                ev(13, 39, 3, K::AppliedLive, Some(5)),
                ev(13, 39, 3, K::ReceivedLive, Some(5)),
                ev(12, 12, 9, K::SnapshotComplete, Some(96_204)),
            ],
        }),
        sub(Seed {
            id: "sub_21_orders",
            name: "orders_for_store",
            device: ("d21", "kiosk-21"),
            collection: "orders",
            filter: store,
            parameter: ("store", "la"),
            state: SubState::Bootstrapping,
            snapshot: Snapshot::Running {
                done: 41_200,
                total: 88_112,
            },
            checkpoint: None,
            seen: None,
            events: vec![
                ev(13, 36, 40, K::ApplyingSnapshot, None),
                ev(13, 35, 52, K::StartingSnapshot, None),
                ev(13, 35, 30, K::Activated, None),
            ],
        }),
    ]
}
