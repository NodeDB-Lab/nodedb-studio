//! The saved-query library, its version history, and the tabs the session
//! restores.

use std::collections::BTreeMap;

use crate::models::format::epoch;
use crate::models::saved_query::{QueryLibrary, QueryScope, SavedQuery, SavedQueryVersion};
use crate::models::workbench::{ParamBinding, ParamType, QueryDraft};

/// The signed-in person, owner of My workspace.
pub const OWNER: &str = "Hana";
pub const DATABASE: &str = "analytics";

pub const CUSTOMER_SPEND_ID: &str = "q-customer-spend";

const EVENTS_SQL: &str = "-- events from the last hour, grouped by type\nSELECT\n    type,\n    COUNT(*) AS n,\n    AVG((props->>'ms_to_load')::int) AS avg_load\nFROM events\nWHERE ts > NOW() - '1 hour'::interval\nGROUP BY type ORDER BY n DESC;";

const TOP_USERS_SQL: &str = "SELECT user_id, COUNT(*) AS events\nFROM events\nGROUP BY user_id\nORDER BY events DESC\nLIMIT 10;";
const TOP_USERS_SAVED: &str =
    "SELECT user_id, COUNT(*) AS events\nFROM events\nGROUP BY user_id\nORDER BY events DESC;";

pub const ORDERS_SQL: &str =
    "SELECT customer_id, SUM(total)\nFROM orders\nWHERE currency = 'USD'\nGROUP BY customer_id";

pub const CUSTOMER_SPEND_SQL: &str = "SELECT\n  c.customer_id,\n  c.name,\n  SUM(o.total_amount) AS total_spend,\n  COUNT(o.order_id) AS order_count\nFROM customers c\nJOIN orders o ON o.customer_id = c.customer_id\nWHERE o.currency = :currency\nGROUP BY c.customer_id, c.name\nORDER BY total_spend DESC;";

const CUSTOMER_SPEND_V2: &str = "SELECT\n  c.customer_id,\n  c.name,\n  SUM(o.total_amount) AS total_spend\nFROM customers c\nJOIN orders o ON o.customer_id = c.customer_id\nWHERE o.currency = :currency\nGROUP BY c.customer_id, c.name\nORDER BY total_spend DESC;";

const CUSTOMER_SPEND_V1: &str = "SELECT c.customer_id, c.name, SUM(o.total_amount) AS total_spend\nFROM customers c\nJOIN orders o ON o.customer_id = c.customer_id\nGROUP BY c.customer_id, c.name;";

/// The three tabs the session opens with. The sidebar badge counts them.
pub fn query_session() -> Vec<QueryDraft> {
    let tab = |id: &str, title: &str, sql: &str, saved: &str| QueryDraft {
        id: id.to_string(),
        title: title.to_string(),
        sql: sql.to_string(),
        saved_sql: saved.to_string(),
        saved_query: None,
        database: DATABASE.to_string(),
    };
    vec![
        tab("tab-1", "events \u{b7} last hour", EVENTS_SQL, EVENTS_SQL),
        tab("tab-2", "top users", TOP_USERS_SQL, TOP_USERS_SAVED),
        tab("tab-3", "orders by region", ORDERS_SQL, ORDERS_SQL),
    ]
}

#[allow(clippy::too_many_arguments)]
fn query(
    id: &str,
    name: &str,
    scope: QueryScope,
    owner: &str,
    sql: &str,
    tags: &[&str],
    version: u32,
    updated: i64,
) -> SavedQuery {
    SavedQuery {
        id: id.to_string(),
        name: name.to_string(),
        folder: None,
        scope,
        owner: owner.to_string(),
        database: DATABASE.to_string(),
        sql: sql.to_string(),
        params: Vec::new(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        version,
        updated_at: updated,
        last_error: None,
    }
}

pub fn library() -> QueryLibrary {
    use QueryScope::{MyWorkspace, Shared};
    let mut spend = query(
        CUSTOMER_SPEND_ID,
        "Customer spend",
        MyWorkspace,
        OWNER,
        CUSTOMER_SPEND_SQL,
        &["customers", "revenue"],
        3,
        epoch(2026, 6, 13, 12, 48, 21),
    );
    spend.params = vec![ParamBinding::new("currency", ParamType::Text, "USD")];
    let mut failed = query(
        "q-failed-orders",
        "Failed orders",
        MyWorkspace,
        OWNER,
        "SELECT * FROM order_items WHERE order_id IN (SELECT id FROM orders WHERE status = 'failed');",
        &["orders"],
        1,
        epoch(2026, 6, 13, 11, 5, 40),
    );
    failed.last_error = Some("collection `order_items` does not exist".to_string());
    QueryLibrary {
        folders: Vec::new(),
        queries: vec![
            spend,
            query(
                "q-sales-daily",
                "Sales daily",
                MyWorkspace,
                OWNER,
                "SELECT date_trunc('day', ts) AS day, SUM(total) AS revenue\nFROM sales\nGROUP BY day\nORDER BY day DESC;",
                &["sales"],
                2,
                epoch(2026, 6, 11, 9, 30, 2),
            ),
            query(
                "q-product-performance",
                "Product performance",
                Shared,
                "Amir",
                "SELECT p.name, COUNT(*) AS orders\nFROM products p\nJOIN order_items i ON i.product_id = p.id\nGROUP BY p.name\nORDER BY orders DESC\nLIMIT 20;",
                &["products"],
                4,
                epoch(2026, 6, 9, 15, 12, 48),
            ),
            query(
                "q-top-customers",
                "Top customers",
                Shared,
                "Amir",
                "SELECT customer_id, SUM(total) AS spend\nFROM orders\nGROUP BY customer_id\nORDER BY spend DESC\nLIMIT 10;",
                &["customers"],
                1,
                epoch(2026, 6, 5, 8, 0, 0),
            ),
            failed,
        ],
    }
}

/// The version history of every saved query, newest first. Customer spend
/// shows the three versions the design lists.
pub fn versions(lib: &QueryLibrary) -> BTreeMap<String, Vec<SavedQueryVersion>> {
    let v = |version, at, sql: &str| SavedQueryVersion {
        version,
        saved_at: at,
        author: OWNER.to_string(),
        sql: sql.to_string(),
    };
    let mut out = BTreeMap::new();
    for q in &lib.queries {
        let list = if q.id == CUSTOMER_SPEND_ID {
            vec![
                v(3, epoch(2026, 6, 13, 12, 48, 21), CUSTOMER_SPEND_SQL),
                v(2, epoch(2026, 6, 10, 9, 15, 4), CUSTOMER_SPEND_V2),
                v(1, epoch(2026, 6, 8, 16, 2, 40), CUSTOMER_SPEND_V1),
            ]
        } else {
            (1..=q.version)
                .rev()
                .map(|n| SavedQueryVersion {
                    version: n,
                    saved_at: q.updated_at - i64::from(q.version - n) * 86_400,
                    author: q.owner.clone(),
                    sql: q.sql.clone(),
                })
                .collect()
        };
        out.insert(q.id.clone(), list);
    }
    out
}
