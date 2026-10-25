//! The rollup being defined: its draft, and the checks that decide whether it
//! can become a definition.

use crate::models::timeseries::{Bucket, is_identifier};
use crate::models::ts_lifecycle::{Lifecycle, Retention};
use crate::models::ts_rollup::{RefreshPolicy, RollupDef, RollupFn, RollupOutput, source_output};

/// A rollup the user is defining.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupDraft {
    pub name: String,
    pub input: String,
    pub bucket: Bucket,
    pub outputs: Vec<RollupOutput>,
    pub group_by: Vec<String>,
    pub refresh: RefreshPolicy,
    pub keep: Retention,
    /// The user accepts a coarser grouping than the input has.
    pub allow_lossy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftField {
    Name,
    Input,
    Bucket,
    Outputs,
    Group,
    Keep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftIssue {
    pub field: DraftField,
    pub message: String,
}

fn bad(field: DraftField, message: impl Into<String>) -> DraftIssue {
    DraftIssue {
        field,
        message: message.into(),
    }
}

impl RollupDraft {
    /// A draft over the collection with the usual outputs, ready to edit.
    pub fn blank(l: &Lifecycle) -> Self {
        Self {
            name: String::new(),
            input: l.collection.clone(),
            bucket: Bucket::Hour1,
            outputs: Vec::new(),
            group_by: l.group_fields.clone(),
            refresh: RefreshPolicy::Interval("1m".to_string()),
            keep: Retention::Days(365),
            allow_lossy: false,
        }
    }

    pub fn def(&self) -> RollupDef {
        RollupDef {
            name: self.name.trim().to_string(),
            input: self.input.clone(),
            bucket: self.bucket,
            outputs: self.outputs.clone(),
            group_by: self.group_by.clone(),
            refresh: self.refresh.clone(),
            keep: self.keep,
        }
    }

    /// What stops this draft from becoming a definition.
    pub fn issues(&self, l: &Lifecycle) -> Vec<DraftIssue> {
        let mut out = Vec::new();
        let name = self.name.trim();
        if !is_identifier(name) {
            out.push(bad(
                DraftField::Name,
                "Name the rollup with letters, digits, and underscores.",
            ));
        } else if l.inputs().iter().any(|n| n == name) {
            out.push(bad(
                DraftField::Name,
                format!("`{name}` already exists in this database."),
            ));
        }
        let input_def = l.rollup(&self.input);
        let input_bucket = input_def.map(|d| d.bucket);
        if !l.inputs().contains(&self.input) {
            out.push(bad(
                DraftField::Input,
                format!("`{}` is not an input of {}.", self.input, l.collection),
            ));
        }
        if let Some(b) = input_bucket
            && (self.bucket.seconds() <= b.seconds() || self.bucket.seconds() % b.seconds() != 0)
        {
            out.push(bad(
                DraftField::Bucket,
                format!(
                    "{} must be a whole multiple of {}'s bucket of {}.",
                    self.bucket.label(),
                    self.input,
                    b.label()
                ),
            ));
        }
        if self.outputs.is_empty() {
            out.push(bad(DraftField::Outputs, "Store at least one output."));
        }
        for o in &self.outputs {
            if let Some(d) = input_def
                && source_output(d, o).is_none()
            {
                out.push(bad(
                    DraftField::Outputs,
                    format!(
                        "{} does not store {}. A rollup merges what its input stored.",
                        d.name,
                        o.logical()
                    ),
                ));
            }
            if o.func != RollupFn::Count && o.column.is_none() {
                out.push(bad(
                    DraftField::Outputs,
                    format!("{} needs a column.", o.func.label()),
                ));
            }
            if !is_identifier(&o.alias) {
                out.push(bad(
                    DraftField::Outputs,
                    format!("`{}` is not a valid output name.", o.alias),
                ));
            }
        }
        let mut aliases: Vec<&str> = self.outputs.iter().map(|o| o.alias.as_str()).collect();
        aliases.sort_unstable();
        if aliases.windows(2).any(|w| w[0] == w[1]) {
            out.push(bad(DraftField::Outputs, "Two outputs share a name."));
        }
        let available: Vec<String> = match input_def {
            Some(d) => d.group_by.clone(),
            None => l.group_fields.clone(),
        };
        if let Some(g) = self.group_by.iter().find(|g| !available.contains(g)) {
            out.push(bad(
                DraftField::Group,
                format!("`{g}` is not a dimension of {}.", self.input),
            ));
        }
        let dropped: Vec<&String> = available
            .iter()
            .filter(|a| !self.group_by.contains(a))
            .collect();
        if !dropped.is_empty() && !self.allow_lossy {
            out.push(bad(
                DraftField::Group,
                format!(
                    "Dropping {} merges series that differ in it, and the merge cannot be undone. Allow dropping dimensions to continue.",
                    dropped.iter().map(|d| d.as_str()).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        if !self.keep.valid() {
            out.push(bad(
                DraftField::Keep,
                "Retention must be at least 1 and at most 100 years.",
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ts_lifecycle::{BeyondHorizon, LatePolicy, LateWrites};

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

    fn lifecycle() -> Lifecycle {
        Lifecycle {
            collection: "sales".into(),
            time_field: "sold_at".into(),
            partition: "1d".into(),
            raw_keep: Retention::Days(90),
            archive: None,
            rollups: vec![hourly()],
            late: LatePolicy {
                horizon_days: 7,
                within: LateWrites::RecomputeAffected,
                beyond: BeyondHorizon::Reject,
            },
            numeric_fields: vec!["revenue".into(), "quantity".into()],
            group_fields: vec!["store_id".into(), "product_id".into(), "currency".into()],
            revision: 1,
        }
    }

    fn draft() -> RollupDraft {
        RollupDraft {
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
            allow_lossy: false,
        }
    }

    #[test]
    fn a_valid_chained_draft_has_no_issues() {
        assert!(draft().issues(&lifecycle()).is_empty());
    }

    #[test]
    fn a_name_in_use_is_refused() {
        let mut d = draft();
        d.name = "sales_hourly".into();
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Name)
        );
        d.name = "sales".into();
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Name)
        );
    }

    #[test]
    fn a_bucket_that_is_not_a_multiple_of_its_input_is_refused() {
        let mut d = draft();
        d.bucket = Bucket::Hour1;
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Bucket)
        );
    }

    #[test]
    fn an_output_the_input_never_stored_is_refused() {
        let mut d = draft();
        d.outputs
            .push(out(RollupFn::Min, Some("revenue"), "min_revenue"));
        let issues = d.issues(&lifecycle());
        assert!(
            issues
                .iter()
                .any(|i| i.field == DraftField::Outputs && i.message.contains("MIN(revenue)"))
        );
    }

    #[test]
    fn dropping_a_dimension_needs_consent() {
        let mut d = draft();
        d.group_by = vec!["currency".into()];
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Group)
        );
        d.allow_lossy = true;
        assert!(d.issues(&lifecycle()).is_empty());
    }

    #[test]
    fn a_dimension_the_input_lacks_is_refused() {
        let mut d = draft();
        d.group_by.push("product_id".into());
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Group)
        );
    }
}
