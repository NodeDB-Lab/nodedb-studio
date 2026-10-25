//! A statement's plan: the operator tree EXPLAIN describes, or ANALYZE
//! measures.

use crate::components::icon::Icon;

/// Whether the plan is an estimate or a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExplainMode {
    /// Plan only. The statement does not run.
    #[default]
    Explain,
    /// Run the statement and measure each operator.
    Analyze,
}

impl ExplainMode {
    pub fn label(self) -> &'static str {
        match self {
            ExplainMode::Explain => "Explain",
            ExplainMode::Analyze => "Analyze",
        }
    }

    /// The line under the toggle.
    pub fn note(self) -> &'static str {
        match self {
            ExplainMode::Explain => "Shows estimated plan. Query not executed.",
            ExplainMode::Analyze => "Runs the query and reports measured rows and time.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorKind {
    Projection,
    HashAggregate,
    Filter,
    Sort,
    ColumnarScan,
    IndexScan,
}

impl OperatorKind {
    pub fn label(self) -> &'static str {
        match self {
            OperatorKind::Projection => "Projection",
            OperatorKind::HashAggregate => "HashAggregate",
            OperatorKind::Filter => "Filter",
            OperatorKind::Sort => "Sort",
            OperatorKind::ColumnarScan => "ColumnarScan",
            OperatorKind::IndexScan => "IndexScan",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            OperatorKind::Projection => Icon::FileCode,
            OperatorKind::HashAggregate => Icon::List,
            OperatorKind::Filter => Icon::Filter,
            OperatorKind::Sort => Icon::ArrowUpDown,
            OperatorKind::ColumnarScan | OperatorKind::IndexScan => Icon::Table,
        }
    }
}

/// One operator. `input` names the operator below it, whose rows it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanOperator {
    pub id: u32,
    pub kind: OperatorKind,
    /// What it works on, shown under the name when the estimate is not.
    pub target: Option<String>,
    pub est_rows: u64,
    /// Rows the operator produced. Only ANALYZE measures it.
    pub actual_rows: Option<u64>,
    /// Time in the operator, in microseconds. Only ANALYZE measures it.
    pub time_us: Option<u64>,
    pub input: Option<u32>,
    pub description: String,
    /// Labeled facts for the details panel, after the type and estimate.
    pub facts: Vec<(String, String)>,
}

/// A plan, root operator first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPlan {
    pub mode: ExplainMode,
    pub operators: Vec<PlanOperator>,
    /// Wall time of the whole statement. Only ANALYZE measures it.
    pub elapsed_ms: Option<u32>,
}

impl QueryPlan {
    pub fn operator(&self, id: u32) -> Option<&PlanOperator> {
        self.operators.iter().find(|o| o.id == id)
    }
}

/// Microseconds as `0.4 ms`, `12 ms`, or `1.2 s`.
pub fn duration(us: u64) -> String {
    match us {
        0..1_000 => format!("{us} \u{b5}s"),
        1_000..10_000 => format!("{:.1} ms", us as f64 / 1_000.0),
        10_000..1_000_000 => format!("{} ms", us / 1_000),
        _ => format!("{:.1} s", us as f64 / 1_000_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_pick_a_readable_unit() {
        assert_eq!(duration(400), "400 \u{b5}s");
        assert_eq!(duration(1_200), "1.2 ms");
        assert_eq!(duration(42_000), "42 ms");
        assert_eq!(duration(1_500_000), "1.5 s");
    }

    #[test]
    fn explain_never_claims_to_run_the_query() {
        assert!(ExplainMode::Explain.note().contains("not executed"));
        assert!(ExplainMode::Analyze.note().contains("Runs the query"));
    }
}
