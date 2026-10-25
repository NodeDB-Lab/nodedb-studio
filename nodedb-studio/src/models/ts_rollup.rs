//! Rollup definitions: refresh policy, stored aggregates, and the
//! `CREATE CONTINUOUS AGGREGATE` statement a definition renders as.

use crate::models::timeseries::Bucket;
use crate::models::ts_lifecycle::Retention;

/// When a rollup folds new rows in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshPolicy {
    /// A fixed interval such as `1m`.
    Interval(String),
    OnSeal,
    OnFlush,
    Manual,
}

impl RefreshPolicy {
    pub fn label(&self) -> String {
        match self {
            RefreshPolicy::Interval(i) => format!("Every {i}"),
            RefreshPolicy::OnSeal => "On seal".to_string(),
            RefreshPolicy::OnFlush => "On flush".to_string(),
            RefreshPolicy::Manual => "Manual".to_string(),
        }
    }

    /// The `WITH (...)` entry.
    pub fn sql(&self) -> String {
        match self {
            RefreshPolicy::Interval(i) => format!("refresh_interval='{i}'"),
            RefreshPolicy::OnSeal => "refresh_policy='on_seal'".to_string(),
            RefreshPolicy::OnFlush => "refresh_policy='on_flush'".to_string(),
            RefreshPolicy::Manual => "refresh_policy='manual'".to_string(),
        }
    }
}

/// An aggregate a rollup stores. Averages are not stored: store SUM and
/// COUNT, which merge exactly, and divide when reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollupFn {
    Sum,
    Count,
    Min,
    Max,
}

impl RollupFn {
    pub const ALL: [RollupFn; 4] = [RollupFn::Sum, RollupFn::Count, RollupFn::Min, RollupFn::Max];

    pub fn sql(self) -> &'static str {
        match self {
            RollupFn::Sum => "sum",
            RollupFn::Count => "count",
            RollupFn::Min => "min",
            RollupFn::Max => "max",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RollupFn::Sum => "SUM",
            RollupFn::Count => "COUNT",
            RollupFn::Min => "MIN",
            RollupFn::Max => "MAX",
        }
    }

    /// The function that merges stored values of this aggregate into a
    /// coarser bucket.
    fn merge(self) -> RollupFn {
        match self {
            RollupFn::Sum | RollupFn::Count => RollupFn::Sum,
            RollupFn::Min => RollupFn::Min,
            RollupFn::Max => RollupFn::Max,
        }
    }
}

/// One stored output of a rollup, in terms of the collection's columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupOutput {
    pub func: RollupFn,
    /// `None` is `COUNT(*)`.
    pub column: Option<String>,
    pub alias: String,
}

impl RollupOutput {
    /// `SUM(revenue)` or `COUNT(*)`.
    pub fn logical(&self) -> String {
        format!(
            "{}({})",
            self.func.label(),
            self.column.as_deref().unwrap_or("*")
        )
    }

    /// `revenue = SUM(revenue)`.
    pub fn line(&self) -> String {
        format!("{} = {}", self.alias, self.logical())
    }
}

/// A saved continuous aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupDef {
    pub name: String,
    /// The collection, or the rollup this one is cut from.
    pub input: String,
    pub bucket: Bucket,
    pub outputs: Vec<RollupOutput>,
    pub group_by: Vec<String>,
    pub refresh: RefreshPolicy,
    pub keep: Retention,
}

/// The aggregate an output reads from `input`, when `input` is a rollup:
/// its stored output of the same function and column.
pub(super) fn source_output<'a>(
    input: &'a RollupDef,
    o: &RollupOutput,
) -> Option<&'a RollupOutput> {
    input
        .outputs
        .iter()
        .find(|i| i.func == o.func && i.column == o.column)
}

impl RollupDef {
    /// The `CREATE CONTINUOUS AGGREGATE` statement. `input_def` is the
    /// input rollup when this one is cut from another: its outputs merge
    /// the stored ones, so a count becomes a sum of counts.
    pub fn sql(&self, collection_input: bool, input_def: Option<&RollupDef>) -> String {
        let outputs: Vec<String> = self
            .outputs
            .iter()
            .map(|o| {
                match (
                    collection_input,
                    input_def.and_then(|d| source_output(d, o)),
                ) {
                    (false, Some(src)) => {
                        format!("{}({}) AS {}", o.func.merge().sql(), src.alias, o.alias)
                    }
                    _ => match &o.column {
                        Some(c) => format!("{}({c}) AS {}", o.func.sql(), o.alias),
                        None => format!("count(*) AS {}", o.alias),
                    },
                }
            })
            .collect();
        let mut sql = format!(
            "CREATE CONTINUOUS AGGREGATE {}\nON {}\nBUCKET '{}'\nAGGREGATE {}",
            self.name,
            self.input,
            self.bucket.label(),
            outputs.join(",\n          ")
        );
        if !self.group_by.is_empty() {
            sql.push_str(&format!("\nGROUP BY {}", self.group_by.join(", ")));
        }
        sql.push_str(&format!(
            "\nWITH ({},\n      retention='{}');",
            self.refresh.sql(),
            self.keep.label()
        ));
        sql
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(func: RollupFn, column: Option<&str>, alias: &str) -> RollupOutput {
        RollupOutput {
            func,
            column: column.map(str::to_string),
            alias: alias.into(),
        }
    }

    fn hourly() -> RollupDef {
        RollupDef {
            name: "sales_hourly".into(),
            input: "sales".into(),
            bucket: Bucket::Hour1,
            outputs: vec![
                out(RollupFn::Sum, Some("revenue"), "revenue"),
                out(RollupFn::Count, None, "sale_count"),
            ],
            group_by: vec!["store_id".into(), "currency".into()],
            refresh: RefreshPolicy::Interval("1m".into()),
            keep: Retention::Days(365),
        }
    }

    fn daily() -> RollupDef {
        RollupDef {
            name: "sales_daily".into(),
            input: "sales_hourly".into(),
            bucket: Bucket::Day1,
            outputs: vec![
                out(RollupFn::Sum, Some("revenue"), "revenue"),
                out(RollupFn::Count, None, "sale_count"),
            ],
            group_by: vec!["store_id".into(), "currency".into()],
            refresh: RefreshPolicy::OnSeal,
            keep: Retention::CalendarYears(3),
        }
    }

    #[test]
    fn the_hourly_definition_renders_as_the_server_syntax() {
        let sql = hourly().sql(true, None);
        assert!(sql.starts_with("CREATE CONTINUOUS AGGREGATE sales_hourly\nON sales\nBUCKET '1h'"));
        assert!(sql.contains("sum(revenue) AS revenue"));
        assert!(sql.contains("count(*) AS sale_count"));
        assert!(sql.contains("GROUP BY store_id, currency"));
        assert!(sql.ends_with("WITH (refresh_interval='1m',\n      retention='365d');"));
    }

    /// A daily count is the sum of the hourly counts, not a recount.
    #[test]
    fn a_chained_rollup_merges_what_its_input_stored() {
        let sql = daily().sql(false, Some(&hourly()));
        assert!(sql.contains("sum(revenue) AS revenue"), "{sql}");
        assert!(sql.contains("sum(sale_count) AS sale_count"), "{sql}");
        assert!(sql.contains("ON sales_hourly"), "{sql}");
    }
}
