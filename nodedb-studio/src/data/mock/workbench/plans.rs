//! Plan fixtures. EXPLAIN carries estimates only. ANALYZE adds the rows and
//! time each operator measured.

use crate::data::mock::workbench::results::{Shape, shape};
use crate::models::query_plan::{ExplainMode, OperatorKind, PlanOperator, QueryPlan};

struct Op {
    kind: OperatorKind,
    target: Option<&'static str>,
    est: u64,
    /// Measured rows and microseconds, used by ANALYZE only.
    measured: (u64, u64),
    description: &'static str,
    facts: &'static [(&'static str, &'static str)],
}

const ORDERS: &[Op] = &[
    Op {
        kind: OperatorKind::Projection,
        target: None,
        est: 1_024,
        measured: (1_019, 210),
        description: "Computes the final output columns.",
        facts: &[("Output columns", "customer_id, SUM(total)")],
    },
    Op {
        kind: OperatorKind::HashAggregate,
        target: None,
        est: 1_024,
        measured: (1_019, 5_400),
        description: "Groups rows by key and computes each aggregate in a hash table.",
        facts: &[("Group keys", "customer_id"), ("Aggregates", "SUM(total)")],
    },
    Op {
        kind: OperatorKind::Filter,
        target: None,
        est: 12_480,
        measured: (12_311, 2_900),
        description: "Keeps the rows that satisfy the predicate.",
        facts: &[("Predicate", "currency = 'USD'")],
    },
    Op {
        kind: OperatorKind::ColumnarScan,
        target: Some("orders \u{b7} Document strict"),
        est: 442_003,
        measured: (442_003, 18_700),
        description: "Reads the columns the plan needs from every segment of the collection.",
        facts: &[
            ("Collection", "orders"),
            ("Columns read", "customer_id, total, currency"),
        ],
    },
];

const EVENTS: &[Op] = &[
    Op {
        kind: OperatorKind::Sort,
        target: None,
        est: 7,
        measured: (7, 60),
        description: "Orders the rows by the sort keys.",
        facts: &[("Sort keys", "n DESC")],
    },
    Op {
        kind: OperatorKind::HashAggregate,
        target: None,
        est: 7,
        measured: (7, 8_900),
        description: "Groups rows by key and computes each aggregate in a hash table.",
        facts: &[
            ("Group keys", "type"),
            ("Aggregates", "COUNT(*), AVG(props->>'ms_to_load')"),
        ],
    },
    Op {
        kind: OperatorKind::Filter,
        target: None,
        est: 14_820,
        measured: (14_820, 4_100),
        description: "Keeps the rows that satisfy the predicate.",
        facts: &[("Predicate", "ts > NOW() - '1 hour'::interval")],
    },
    Op {
        kind: OperatorKind::IndexScan,
        target: Some("events \u{b7} events_ts_idx"),
        est: 14_820,
        measured: (14_820, 21_000),
        description: "Reads the rows of the index range in order.",
        facts: &[("Index", "events_ts_idx"), ("Range", "ts > now - 1 hour")],
    },
];

const GENERIC: &[Op] = &[Op {
    kind: OperatorKind::ColumnarScan,
    target: Some("Collection scan"),
    est: 4,
    measured: (4, 300),
    description: "Reads every row of the collection.",
    facts: &[],
}];

fn ops(sql: &str) -> &'static [Op] {
    match shape(sql) {
        Shape::OrdersByCustomer => ORDERS,
        Shape::EventsByType => EVENTS,
        Shape::CustomerSpend | Shape::TopUsers | Shape::Generic => GENERIC,
    }
}

/// The plan of `sql`. An estimate carries no measurement at all: a number
/// shown under EXPLAIN is a guess, never a timing.
pub fn query_plan(sql: &str, mode: ExplainMode) -> QueryPlan {
    let list = ops(sql);
    let operators = list
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let analyzed = mode == ExplainMode::Analyze;
            PlanOperator {
                id: i as u32 + 1,
                kind: op.kind,
                target: op.target.map(str::to_string),
                est_rows: op.est,
                actual_rows: analyzed.then_some(op.measured.0),
                time_us: analyzed.then_some(op.measured.1),
                input: (i + 1 < list.len()).then_some(i as u32 + 2),
                description: op.description.to_string(),
                facts: op
                    .facts
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            }
        })
        .collect();
    QueryPlan {
        mode,
        operators,
        elapsed_ms: (mode == ExplainMode::Analyze).then_some(31),
    }
}
