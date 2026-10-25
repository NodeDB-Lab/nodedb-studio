//! Rejected write fixtures. m104 collides with orders/o104, which the
//! Explorer holds, so the authoritative record the panel shows is the one
//! the Explorer shows.

use nodedb_types::Value;

use crate::data::mock::explorer::build::{dec, s, ts};
use crate::data::mock::sync::devices::t;
use crate::models::sync_reject::{Hint, RejectReason, RejectState, Rejection};

fn object(fields: Vec<(&str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

struct Seed {
    mutation: &'static str,
    device: (&'static str, &'static str),
    collection: &'static str,
    reason: RejectReason,
    constraint: &'static str,
    received: (u32, u32, u32),
    payload: Value,
    existing: Option<&'static str>,
    hint: Hint,
    corrected: bool,
}

fn reject(seed: Seed) -> Rejection {
    let received_at = t(seed.received.0, seed.received.1, seed.received.2);
    Rejection {
        mutation: seed.mutation.to_string(),
        device_id: seed.device.0.to_string(),
        device_name: seed.device.1.to_string(),
        database: "analytics".to_string(),
        collection: seed.collection.to_string(),
        reason: seed.reason,
        state: RejectState::Quarantined,
        constraint: seed.constraint.to_string(),
        idempotency_key: seed.mutation.replace('m', "mutation"),
        device_time: received_at - 2,
        received_at,
        payload: seed.payload,
        existing: seed.existing.map(str::to_string),
        hint: seed.hint,
        corrected: seed.corrected,
        revision: 1,
    }
}

/// Newest first.
pub fn seeded_rejects() -> Vec<Rejection> {
    use RejectReason as R;
    vec![
        reject(Seed {
            mutation: "m104",
            device: ("d17", "checkout-ipad"),
            collection: "orders",
            reason: R::UniqueViolation,
            constraint: "orders_pkey (order_id)",
            received: (13, 40, 0),
            payload: object(vec![
                ("order_id", s("o104")),
                ("customer_id", s("c178")),
                ("total", dec("249.99")),
                ("currency", s("USD")),
                ("status", s("processing")),
                ("created_at", ts(2026, 6, 13, 13, 39, 51)),
            ]),
            existing: Some("o104"),
            hint: Hint::RetryWithDifferentValue {
                field: "order_id".into(),
                conflicting_value: "o104".into(),
                suggestion: "o105".into(),
            },
            corrected: false,
        }),
        reject(Seed {
            mutation: "m103",
            device: ("d08", "front-desk-ipad"),
            collection: "customers",
            reason: R::ForeignKeyViolation,
            constraint: "customers_region_fk (region_id)",
            received: (12, 18, 0),
            payload: object(vec![
                ("customer_id", s("c301")),
                ("name", s("Mei Tan")),
                ("region_id", s("r77")),
            ]),
            existing: None,
            hint: Hint::CreateReferencedRow {
                collection: "regions".into(),
                key: "r77".into(),
            },
            // The Origin created region r77 after the rejection.
            corrected: true,
        }),
        reject(Seed {
            mutation: "m102",
            device: ("d21", "kiosk-21"),
            collection: "orders",
            reason: R::CheckConstraint,
            constraint: "orders_total_positive (total > 0)",
            received: (10, 3, 0),
            payload: object(vec![
                ("order_id", s("o220")),
                ("customer_id", s("c155")),
                ("total", dec("-12.00")),
                ("currency", s("USD")),
                ("status", s("paid")),
                ("created_at", ts(2026, 6, 13, 10, 2, 41)),
            ]),
            existing: None,
            hint: Hint::ManualIntervention {
                reason: "total must be greater than 0. Correct the amount or discard the write."
                    .into(),
            },
            corrected: false,
        }),
        reject(Seed {
            mutation: "m101",
            device: ("d09", "stock-ipad"),
            collection: "products",
            reason: R::NotNullViolation,
            constraint: "products.name (not null)",
            received: (9, 27, 0),
            payload: object(vec![
                ("sku", s("sku-904")),
                ("name", Value::Null),
                ("price", dec("19.99")),
            ]),
            existing: None,
            hint: Hint::ProvideRequiredField {
                field: "name".into(),
            },
            corrected: false,
        }),
        reject(Seed {
            mutation: "m100",
            device: ("d14", "manager-ipad"),
            collection: "orders",
            reason: R::UniqueViolation,
            constraint: "orders_pkey (order_id)",
            received: (8, 11, 0),
            payload: object(vec![
                ("order_id", s("o100")),
                ("customer_id", s("c155")),
                ("total", dec("42.00")),
                ("currency", s("USD")),
                ("status", s("paid")),
                ("created_at", ts(2026, 6, 13, 8, 10, 52)),
            ]),
            existing: Some("o100"),
            hint: Hint::RetryWithDifferentValue {
                field: "order_id".into(),
                conflicting_value: "o100".into(),
                suggestion: "o106".into(),
            },
            corrected: false,
        }),
    ]
}
