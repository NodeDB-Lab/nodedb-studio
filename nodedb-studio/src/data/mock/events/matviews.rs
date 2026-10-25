//! Materialized view fixtures and the in-memory store the mock backend writes.

use std::collections::BTreeMap;

use crate::data::mock::events::topics::NOW;
use crate::models::format::{date, date_second, epoch, thousands};
use crate::models::matviews::{
    MaterializedView, RefreshError, RefreshPolicy, RefreshRun, RefreshStatus, Trigger, ViewColumn,
    ViewRows,
};
use crate::services::error::StudioError;

const DATABASE: &str = "analytics";

const DAILY_SQL: &str = "SELECT\n  time_bucket('1 day', sold_at) AS day,\n  store_id,\n  SUM(revenue) AS revenue_usd,\n  COUNT(*) AS order_count\nFROM sales\nWHERE currency = 'USD'\nGROUP BY day, store_id";

fn view(
    n: u32,
    name: &str,
    sql: &str,
    policy: RefreshPolicy,
    watermark: Option<&str>,
    rows: u64,
    deps: &[&str],
) -> MaterializedView {
    MaterializedView {
        id: format!("matview_{n:02}"),
        name: name.to_string(),
        database: DATABASE.to_string(),
        definition: sql.to_string(),
        policy,
        watermark: watermark.map(str::to_string),
        lag_secs: None,
        dependencies: deps.iter().map(|d| (*d).to_string()).collect(),
        row_count: rows,
    }
}

pub fn seeded_views() -> Vec<MaterializedView> {
    use RefreshPolicy::{Full, Incremental};
    let mut top = view(
        3,
        "mv_top_products",
        "SELECT product_id, SUM(quantity) AS units, SUM(revenue) AS revenue_usd\nFROM sales\nWHERE currency = 'USD'\nGROUP BY product_id\nORDER BY revenue_usd DESC\nLIMIT 25",
        Full,
        Some("2026-06-13 12:00:00 UTC"),
        25,
        &["analytics.sales", "analytics.products"],
    );
    // The one view whose server reports its lag.
    top.lag_secs = Some(2562);
    vec![
        view(
            1,
            "mv_sales_daily",
            DAILY_SQL,
            Incremental,
            Some("2026-06-13 13:39:00 UTC"),
            90,
            &["analytics.sales"],
        ),
        view(
            2,
            "mv_sales_monthly",
            "SELECT time_bucket('1 month', sold_at) AS month,\n  store_id,\n  SUM(revenue) AS revenue_usd,\n  COUNT(*) AS order_count\nFROM sales\nWHERE currency = 'USD'\nGROUP BY month, store_id",
            Incremental,
            Some("2026-06-13 13:39:00 UTC"),
            18,
            &["analytics.sales"],
        ),
        top,
        view(
            4,
            "mv_store_performance",
            "SELECT store_id, AVG(revenue) AS avg_basket_usd, COUNT(*) AS orders\nFROM sales\nGROUP BY store_id",
            Incremental,
            Some("2026-06-13 13:39:00 UTC"),
            3,
            &["analytics.sales"],
        ),
        view(
            5,
            "mv_customer_segments",
            "SELECT segment, COUNT(*) AS customers, AVG(lifetime_value) AS avg_ltv_usd\nFROM customers\nGROUP BY segment",
            Full,
            None,
            6,
            &["analytics.customers"],
        ),
    ]
}

fn run(view: &str, n: u32, at: &str, status: RefreshStatus, detail: &str) -> RefreshRun {
    RefreshRun {
        id: format!("{view}_r{n}"),
        at: at.to_string(),
        status,
        detail: detail.to_string(),
        started_by: Trigger::Scheduled,
        error: None,
    }
}

fn seeded_history(v: &MaterializedView) -> Vec<RefreshRun> {
    use RefreshStatus::{Completed, Error};
    let id = v.id.as_str();
    if id == "matview_01" {
        let mut failed = run(
            id,
            3,
            "2026-06-13 11:39:00",
            Error,
            "Partition read failed: source data unavailable",
        );
        failed.error = Some(RefreshError {
            message: "Partition read failed: source data unavailable".to_string(),
            source: Some("analytics.sales, partition 2026-06-13T11".to_string()),
            hint: "The next scheduled refresh retries from the last applied watermark. Run Refresh to retry now.".to_string(),
        });
        return vec![
            run(
                id,
                4,
                "2026-06-13 13:39:00",
                Completed,
                "Processed 18,432 rows (12.4 MB)",
            ),
            run(
                id,
                5,
                "2026-06-13 12:39:00",
                Completed,
                "Processed 18,405 rows (12.1 MB)",
            ),
            failed,
            run(
                id,
                1,
                "2026-06-13 10:39:00",
                Completed,
                "Processed 18,298 rows (12.0 MB)",
            ),
        ];
    }
    let rows = thousands(v.row_count * 211);
    vec![
        run(
            id,
            3,
            "2026-06-13 12:00:00",
            Completed,
            &format!("Processed {rows} rows (4.1 MB)"),
        ),
        run(
            id,
            2,
            "2026-06-13 06:00:00",
            Completed,
            &format!("Processed {rows} rows (4.1 MB)"),
        ),
        run(
            id,
            1,
            "2026-06-13 00:00:00",
            Completed,
            &format!("Processed {rows} rows (4.0 MB)"),
        ),
    ]
}

/// `12,453.20` from cents.
fn money(cents: u64) -> String {
    format!("{}.{:02}", thousands(cents / 100), cents % 100)
}

/// The first ten rows of `mv_sales_daily`, as the design shows them.
const DAILY_HEAD: [(&str, u32, u64, u64); 10] = [
    ("2026-06-01", 101, 1_245_320, 342),
    ("2026-06-01", 102, 891_255, 280),
    ("2026-06-01", 103, 1_620_418, 411),
    ("2026-06-02", 101, 1_300_677, 356),
    ("2026-06-02", 102, 944_592, 301),
    ("2026-06-02", 103, 1_588_940, 398),
    ("2026-06-03", 101, 1_422_119, 372),
    ("2026-06-03", 102, 980_106, 315),
    ("2026-06-03", 103, 1_734_062, 436),
    ("2026-06-04", 101, 1_366_733, 365),
];

fn col(name: &str, numeric: bool) -> ViewColumn {
    ViewColumn {
        name: name.to_string(),
        numeric,
    }
}

/// Every row of a view, as display text, and its columns.
fn all_rows(v: &MaterializedView) -> (Vec<ViewColumn>, Vec<Vec<String>>) {
    let n = v.row_count as usize;
    let day0 = epoch(2026, 6, 1, 0, 0, 0);
    let s = |x: String| x;
    match v.name.as_str() {
        "mv_sales_daily" => {
            let rows = (0..n)
                .map(|i| {
                    if let Some((d, store, cents, orders)) = DAILY_HEAD.get(i) {
                        return vec![
                            d.to_string(),
                            store.to_string(),
                            money(*cents),
                            orders.to_string(),
                        ];
                    }
                    let store = 101 + (i % 3) as u32;
                    let cents = 800_000 + (i as u64 * 7_919) % 1_000_000;
                    vec![
                        date(day0 + (i as i64 / 3) * 86_400, 0),
                        store.to_string(),
                        money(cents),
                        (250 + (i as u64 * 31) % 200).to_string(),
                    ]
                })
                .collect();
            (
                vec![
                    col("day", false),
                    col("store_id", false),
                    col("revenue_usd", true),
                    col("order_count", true),
                ],
                rows,
            )
        }
        "mv_sales_monthly" => (
            vec![
                col("month", false),
                col("store_id", false),
                col("revenue_usd", true),
                col("order_count", true),
            ],
            (0..n)
                .map(|i| {
                    let month = 1 + i / 3;
                    vec![
                        format!("2026-{month:02}"),
                        s((101 + i % 3).to_string()),
                        money(38_000_000 + (i as u64 * 104_729) % 9_000_000),
                        thousands(9_000 + (i as u64 * 53) % 3_000),
                    ]
                })
                .collect(),
        ),
        "mv_top_products" => (
            vec![
                col("product_id", false),
                col("units", true),
                col("revenue_usd", true),
            ],
            (0..n)
                .map(|i| {
                    vec![
                        format!("p{:04}", 1000 + (i * 37) % 900),
                        thousands(4_800 - i as u64 * 151),
                        money(9_800_000 - i as u64 * 301_700),
                    ]
                })
                .collect(),
        ),
        "mv_store_performance" => (
            vec![
                col("store_id", false),
                col("avg_basket_usd", true),
                col("orders", true),
            ],
            (0..n)
                .map(|i| {
                    vec![
                        s((101 + i).to_string()),
                        money(4_310 + i as u64 * 275),
                        thousands(11_204 - i as u64 * 1_903),
                    ]
                })
                .collect(),
        ),
        _ => (
            vec![
                col("segment", false),
                col("customers", true),
                col("avg_ltv_usd", true),
            ],
            [
                "enterprise",
                "mid_market",
                "smb",
                "startup",
                "consumer",
                "partner",
            ]
            .iter()
            .enumerate()
            .take(n)
            .map(|(i, seg)| {
                vec![
                    (*seg).to_string(),
                    thousands(120 + i as u64 * 1_340),
                    money(2_400_000 / (i as u64 + 1)),
                ]
            })
            .collect(),
        ),
    }
}

#[derive(Debug)]
pub struct ViewStore {
    views: Vec<MaterializedView>,
    history: BTreeMap<String, Vec<RefreshRun>>,
    manual: u32,
}

impl ViewStore {
    pub fn seeded() -> Self {
        let views = seeded_views();
        let history = views
            .iter()
            .map(|v| (v.id.clone(), seeded_history(v)))
            .collect();
        Self {
            views,
            history,
            manual: 0,
        }
    }

    pub fn views(&self) -> Vec<MaterializedView> {
        self.views.clone()
    }

    fn find(&self, id: &str) -> Result<&MaterializedView, StudioError> {
        self.views
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| StudioError::UnknownObject {
                kind: "materialized view",
                id: id.to_string(),
            })
    }

    pub fn rows(&self, id: &str, page: usize, size: usize) -> Result<ViewRows, StudioError> {
        let v = self.find(id)?;
        let size = size.clamp(1, 100);
        let (columns, all) = all_rows(v);
        let total = all.len() as u64;
        let rows = all
            .into_iter()
            .enumerate()
            .skip(page * size)
            .take(size)
            .map(|(i, cells)| (format!("{id}_row{i}"), cells))
            .collect();
        Ok(ViewRows {
            columns,
            rows,
            total,
            page,
            page_size: size,
        })
    }

    pub fn history(&self, id: &str) -> Result<Vec<RefreshRun>, StudioError> {
        self.find(id)?;
        Ok(self.history.get(id).cloned().unwrap_or_default())
    }

    /// A manual refresh: it completes at once in the mock, and moves the
    /// watermark to the start of the current minute.
    pub fn refresh(&mut self, id: &str) -> Result<RefreshRun, StudioError> {
        let rows = self.find(id)?.row_count;
        self.manual += 1;
        let at = NOW + i64::from(self.manual) * 60;
        let mut r = run(
            id,
            100 + self.manual,
            &date_second(at, 0),
            RefreshStatus::Completed,
            &format!("Processed {} rows (12.5 MB)", thousands(rows * 205)),
        );
        r.started_by = Trigger::Manual;
        if let Some(v) = self.views.iter_mut().find(|v| v.id == id) {
            v.watermark = Some(format!("{} UTC", date_second(at - at.rem_euclid(60), 0)));
        }
        self.history
            .entry(id.to_string())
            .or_default()
            .insert(0, r.clone());
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_page_matches_the_design() {
        let s = ViewStore::seeded();
        let p = s.rows("matview_01", 0, 10).expect("rows");
        assert_eq!(p.range_label(), "Rows 1–10 of 90");
        assert_eq!(p.pages(), 9);
        assert_eq!(p.rows[0].1, ["2026-06-01", "101", "12,453.20", "342"]);
        assert_eq!(p.rows[8].1, ["2026-06-03", "103", "17,340.62", "436"]);
        assert_eq!(p.rows[9].1, ["2026-06-04", "101", "13,667.33", "365"]);
    }

    #[test]
    fn later_pages_continue_and_the_last_is_full() {
        let s = ViewStore::seeded();
        let last = s.rows("matview_01", 8, 10).expect("rows");
        assert_eq!(last.rows.len(), 10);
        assert_eq!(last.range_label(), "Rows 81–90 of 90");
        assert!(s.rows("matview_01", 9, 10).expect("rows").rows.is_empty());
    }

    #[test]
    fn every_view_reads_its_declared_row_count_with_unique_keys() {
        let s = ViewStore::seeded();
        for v in s.views() {
            let all = s.rows(&v.id, 0, 100).expect("rows");
            assert_eq!(all.total, v.row_count, "{}", v.name);
            assert_eq!(all.rows.len() as u64, v.row_count.min(100), "{}", v.name);
            let mut keys: Vec<_> = all.rows.iter().map(|r| r.0.clone()).collect();
            keys.sort();
            keys.dedup();
            assert_eq!(keys.len(), all.rows.len());
            assert!(
                all.rows.iter().all(|r| r.1.len() == all.columns.len()),
                "{}",
                v.name
            );
        }
    }

    #[test]
    fn the_design_view_reports_no_lag_and_a_failed_refresh() {
        let s = ViewStore::seeded();
        let v = &s.views()[0];
        assert_eq!(v.lag_label(), "Not reported");
        assert_eq!(v.watermark.as_deref(), Some("2026-06-13 13:39:00 UTC"));
        let h = s.history(&v.id).expect("history");
        assert_eq!(h.len(), 4);
        assert_eq!(h[2].status, RefreshStatus::Error);
        assert!(h[2].error.is_some());
        assert_eq!(s.views()[2].lag_label(), "42m 42s");
    }

    #[test]
    fn a_manual_refresh_is_recorded_and_moves_the_watermark() {
        let mut s = ViewStore::seeded();
        let r = s.refresh("matview_01").expect("refreshes");
        assert_eq!(r.started_by, Trigger::Manual);
        let h = s.history("matview_01").expect("history");
        assert_eq!((h.len(), h[0].id.as_str()), (5, "matview_01_r101"));
        assert_eq!(
            s.views()[0].watermark.as_deref(),
            Some("2026-06-13 13:43:00 UTC")
        );
        assert!(s.refresh("matview_99").is_err());
    }
}
