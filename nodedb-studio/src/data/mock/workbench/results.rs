//! Result fixtures. A statement's text picks the table it answers with, so
//! the editor tabs, the saved queries, and the console each show their own
//! rows.

use crate::models::explorer::RecordRow;
use crate::models::format::thousands;
use crate::models::workbench::{QueryRequest, ResultSet};

/// What kind of table a statement reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    EventsByType,
    CustomerSpend,
    TopUsers,
    OrdersByCustomer,
    Generic,
}

/// Pick the fixture a statement reads, by the tables it names.
pub fn shape(sql: &str) -> Shape {
    let s = sql.to_lowercase();
    if s.contains("customers c") {
        Shape::CustomerSpend
    } else if s.contains("from events") && s.contains("group by type") {
        Shape::EventsByType
    } else if s.contains("group by user_id") {
        Shape::TopUsers
    } else if s.contains("from orders") && s.contains("customer_id") {
        Shape::OrdersByCustomer
    } else {
        Shape::Generic
    }
}

fn rows(table: Vec<Vec<String>>) -> Vec<RecordRow> {
    table
        .into_iter()
        .enumerate()
        .map(|(i, cells)| RecordRow {
            id: format!("row-{i}"),
            cells,
        })
        .collect()
}

fn cols(names: &[&str]) -> Vec<String> {
    names.iter().map(|c| c.to_string()).collect()
}

fn text(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect()
}

/// A fixture is rectangular by construction. The fallback keeps this
/// infallible without an unwrap; `workbench_data`'s tests pin the shapes.
fn build(columns: Vec<String>, table: Vec<Vec<String>>, ms: u32, scanned: &str) -> ResultSet {
    ResultSet::new(columns, rows(table), ms, scanned.to_string()).unwrap_or_default()
}

fn events_by_type() -> ResultSet {
    build(
        cols(&["type", "n", "avg_load"]),
        text(&[
            &["page_view", "14,820", "312"],
            &["click", "8,041", "\u{2013}"],
            &["scroll", "5,209", "\u{2013}"],
            &["form_submit", "481", "1,820"],
            &["error", "104", "\u{2013}"],
            &["signup", "22", "2,140"],
            &["purchase", "9", "1,491"],
        ]),
        142,
        "14M",
    )
    .with_access_path("index_scan(events_ts_idx)")
}

const CUSTOMERS: &[&str] = &[
    "Acme Corp",
    "Globex Inc",
    "Initech",
    "Umbrella Co",
    "Soylent Corp",
    "Hooli",
    "Stark Industries",
    "Wayne Enterprises",
    "Wonka Industries",
    "Cyberdyne",
];

/// 127 customers, richest first. The first five are the fixture the design
/// shows. The rest follow a fixed pattern, so every page reads the same.
const CUSTOMER_TOTAL: usize = 127;

fn customer_row(i: usize) -> Vec<String> {
    const HEAD: [(&str, &str, &str, &str); 5] = [
        ("C1001", "Acme Corp", "45,231.80", "28"),
        ("C1042", "Globex Inc", "38,942.15", "24"),
        ("C0788", "Initech", "24,891.00", "17"),
        ("C0933", "Umbrella Co", "22,156.75", "14"),
        ("C1107", "Soylent Corp", "19,776.50", "12"),
    ];
    if let Some((id, name, spend, orders)) = HEAD.get(i) {
        return vec![
            id.to_string(),
            name.to_string(),
            spend.to_string(),
            orders.to_string(),
        ];
    }
    let cents = 1_977_650u64
        .saturating_sub((i as u64 - 4) * 11_325)
        .max(1_500);
    let orders = (12usize).saturating_sub((i - 4) / 12).max(1);
    vec![
        format!("C{:04}", 1100 + (i * 37) % 900),
        format!("{} {}", CUSTOMERS[i % CUSTOMERS.len()], i),
        format!("{}.{:02}", thousands(cents / 100), cents % 100),
        orders.to_string(),
    ]
}

fn customer_spend(page: usize, size: usize) -> ResultSet {
    let first = (page * size).min(CUSTOMER_TOTAL);
    let last = (first + size).min(CUSTOMER_TOTAL);
    build(
        cols(&["customer_id", "name", "total_spend", "order_count"]),
        (first..last).map(customer_row).collect(),
        64,
        "442K",
    )
    .with_access_path("hash_join(customers, orders)")
    .with_page(page, size, CUSTOMER_TOTAL)
}

fn top_users() -> ResultSet {
    let table = (0..10)
        .map(|i| {
            vec![
                format!("u{}", 44_182 - i * 311),
                thousands(9_840 - (i as u64) * 733),
            ]
        })
        .collect();
    build(cols(&["user_id", "events"]), table, 88, "2.4M")
        .with_access_path("hash_aggregate(events)")
}

fn orders_by_customer() -> ResultSet {
    build(
        cols(&["customer_id", "sum"]),
        text(&[
            &["C1001", "45,231.80"],
            &["C1042", "38,942.15"],
            &["C0788", "24,891.00"],
            &["C0933", "22,156.75"],
        ]),
        37,
        "12K",
    )
    .with_access_path("columnar_scan(orders)")
}

fn generic() -> ResultSet {
    let table = (0..4)
        .map(|i| {
            vec![
                format!("row-{i}"),
                format!("item {i}"),
                format!("2026-08-0{} 10:0{}:00", (i % 9) + 1, i),
            ]
        })
        .collect();
    build(cols(&["id", "name", "created_at"]), table, 12, "4")
}

/// The rows `request` reads, one page of them.
pub fn result_set(request: &QueryRequest) -> ResultSet {
    match shape(&request.sql) {
        Shape::EventsByType => events_by_type(),
        Shape::CustomerSpend => customer_spend(request.page, request.page_size.max(1)),
        Shape::TopUsers => top_users(),
        Shape::OrdersByCustomer => orders_by_customer(),
        Shape::Generic => generic(),
    }
}

/// The columns of `request`'s fixture with no rows: the genuinely empty
/// answer, the most common non-error outcome of a query.
pub fn empty_result_set(request: &QueryRequest) -> ResultSet {
    let full = result_set(request);
    ResultSet::new(full.columns().to_vec(), Vec::new(), 3, "0".to_string()).unwrap_or_default()
}

/// The answer to a write: how many rows it changed.
pub fn affected(count: u64) -> ResultSet {
    build(
        cols(&["affected_rows"]),
        vec![vec![count.to_string()]],
        9,
        "",
    )
}
