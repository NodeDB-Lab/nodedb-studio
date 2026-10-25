//! The Results table: the chart's rows, as the query returned them. Series
//! the legend hides are out of the table too. Rows are aggregated buckets,
//! never raw samples, and the footer says so.

use std::collections::BTreeSet;

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::{date_minute, thousands};
use crate::models::timeseries::{ExploreResult, PointState, ResultRow};
use crate::state::timeseries::{ExploreStatus, SortCol};
use crate::views::timeseries::chart::value_text;

/// The most rows the table lays out. A copy still takes every row.
pub const MAX_ROWS: usize = 500;

/// Rows in the order the header chose.
pub fn sorted(mut rows: Vec<ResultRow>, sort: (SortCol, bool)) -> Vec<ResultRow> {
    let (col, descending) = sort;
    rows.sort_by(|a, b| {
        let o = match col {
            SortCol::Bucket => a.bucket.cmp(&b.bucket).then_with(|| a.group.cmp(&b.group)),
            SortCol::Group => a.group.cmp(&b.group).then_with(|| a.bucket.cmp(&b.bucket)),
            // A null sorts below every value.
            SortCol::Value => match (a.value, b.value) {
                (Some(x), Some(y)) => x.total_cmp(&y),
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
            }
            .then_with(|| a.bucket.cmp(&b.bucket)),
        };
        if descending { o.reverse() } else { o }
    });
    rows
}

/// What the footer says about the rows: how many show, and what they are.
pub fn footer_text(shown: usize, total: usize, status: ExploreStatus) -> String {
    let mut text = if total > shown {
        format!(
            "First {} of {} rows. Copy CSV takes all of them.",
            thousands(shown as u64),
            thousands(total as u64)
        )
    } else if total == 1 {
        "1 row shown.".to_string()
    } else {
        format!("{} rows shown.", thousands(total as u64))
    };
    text.push_str(" Aggregated buckets, not raw samples.");
    match status {
        ExploreStatus::Running => text.push_str(" Running."),
        ExploreStatus::Cancelled => text.push_str(" Cancelled. This is the last result."),
        ExploreStatus::Error => text.push_str(" The last run failed. This is the last result."),
        ExploreStatus::Draft | ExploreStatus::Result | ExploreStatus::Empty => {}
    }
    text
}

#[component]
pub fn ResultsBody(
    result: ExploreResult,
    hidden: BTreeSet<String>,
    sort: (SortCol, bool),
    status: ExploreStatus,
    on_sort: EventHandler<SortCol>,
) -> Element {
    let rows = sorted(result.visible_rows(&hidden), sort);
    let total = rows.len();
    let grouped = result.query.group_by.clone();
    let agg = result.query.measure.aggregate;
    let alias = result.query.measure.alias();
    let shown: Vec<ResultRow> = rows.into_iter().take(MAX_ROWS).collect();
    let footer = footer_text(shown.len(), total, status);
    let head = |col: SortCol, label: String, num: bool| {
        let active = sort.0 == col;
        let icon = if active && sort.1 {
            Icon::ChevronUp
        } else {
            Icon::ChevronDown
        };
        rsx! {
            th {
                class: if num { "num" } else { "" },
                "aria-sort": if active { if sort.1 { "descending" } else { "ascending" } } else { "none" },
                button { class: "ts-sort", onclick: move |_| on_sort.call(col),
                    "{label}"
                    span { class: if active { "ts-sort-icon on" } else { "ts-sort-icon" },
                        Ico { icon, size: 16 }
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "dt-wrap ts-table-wrap",
            table { class: "dt ts-table",
                thead {
                    tr {
                        {head(SortCol::Bucket, "Bucket (UTC)".to_string(), false)}
                        if let Some(g) = grouped.clone() {
                            {head(SortCol::Group, g, false)}
                        }
                        {head(SortCol::Value, alias.clone(), true)}
                    }
                }
                tbody {
                    for (i , r) in shown.iter().enumerate() {
                        tr { key: "{r.bucket}-{r.group}-{i}",
                            td { class: "mono", "{date_minute(r.bucket, 0)}" }
                            if grouped.is_some() {
                                td { "{r.group}" }
                            }
                            td { class: "num",
                                match r.value {
                                    Some(v) => rsx! { "{value_text(agg, v)}" },
                                    None => rsx! { span { class: "ts-null-text", title: "Every value in the bucket is null", "null" } },
                                }
                                match r.state {
                                    PointState::Observed => rsx! {},
                                    PointState::NullOnly => rsx! {},
                                    PointState::Filled(f) => rsx! { span { class: "tag ts-note", title: "Added by the gap policy. Not stored.", "{f.label()}" } },
                                }
                            }
                        }
                    }
                }
            }
        }
        div { class: "dt-footer ts-footer", role: "status", "{footer}" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(bucket: i64, group: &str, value: Option<f64>) -> ResultRow {
        ResultRow {
            bucket,
            group: group.into(),
            value,
            state: PointState::Observed,
        }
    }

    #[test]
    fn rows_sort_by_the_chosen_column_in_either_direction() {
        let rows = vec![
            row(2, "b", Some(5.0)),
            row(1, "b", Some(9.0)),
            row(1, "a", None),
        ];
        let by_bucket: Vec<(i64, &str)> = sorted(rows.clone(), (SortCol::Bucket, false))
            .iter()
            .map(|r| (r.bucket, if r.group == "a" { "a" } else { "b" }))
            .collect();
        assert_eq!(by_bucket, [(1, "a"), (1, "b"), (2, "b")]);
        let by_value = sorted(rows.clone(), (SortCol::Value, true));
        assert_eq!(by_value[0].value, Some(9.0));
        assert_eq!(by_value[2].value, None, "a null sorts last when descending");
        let by_group = sorted(rows, (SortCol::Group, false));
        assert_eq!(by_group[0].group, "a");
    }

    #[test]
    fn the_footer_never_calls_aggregates_raw_samples() {
        let t = footer_text(4, 4, ExploreStatus::Result);
        assert!(t.starts_with("4 rows shown."));
        assert!(t.contains("not raw samples"));
        assert!(footer_text(1, 1, ExploreStatus::Result).starts_with("1 row shown."));
    }

    #[test]
    fn a_truncated_table_says_how_many_rows_exist() {
        let t = footer_text(500, 2_094, ExploreStatus::Result);
        assert!(t.starts_with("First 500 of 2,094 rows."), "{t}");
    }

    #[test]
    fn a_stale_table_says_so() {
        assert!(footer_text(2, 2, ExploreStatus::Error).contains("last run failed"));
        assert!(footer_text(2, 2, ExploreStatus::Cancelled).contains("Cancelled"));
    }
}
