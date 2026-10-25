//! The change feed form: its fields, what it checks before the server does,
//! and what the server's preview reports back.
//!
//! The checks here are the ones a person can fix without a round trip: a
//! name, a filter the feed grammar accepts, an endpoint that holds no
//! credentials. Permission, schema references, and target requirements are
//! the server's, and arrive as a `FeedPreview`.

use serde::{Deserialize, Serialize};

use crate::models::feeds::{
    Backoff, ChangeFeed, ChangeOp, DeliveryKind, RetentionUnit, StartPosition,
};

/// A field of the form, so an issue sits under its own control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftField {
    Name,
    Source,
    Operations,
    Filter,
    Projection,
    Checkpoint,
    Endpoint,
    Alias,
    MaxRetries,
    Retention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub at: DraftField,
    pub message: String,
}

fn issue(at: DraftField, message: impl Into<String>) -> Issue {
    Issue {
        at,
        message: message.into(),
    }
}

/// The Create change feed form. Numbers stay text while they are edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedDraft {
    pub name: String,
    /// `database.collection`.
    pub source: String,
    pub operations: Vec<ChangeOp>,
    pub filter: String,
    pub projection: String,
    pub before_image: bool,
    pub start: StartPosition,
    /// The saved checkpoint to resume from.
    pub checkpoint: String,
    pub delivery: DeliveryKind,
    pub endpoint: String,
    pub alias: String,
    pub backoff: Backoff,
    pub max_retries: String,
    pub retention: String,
    pub retention_unit: RetentionUnit,
}

impl FeedDraft {
    /// A blank form on `source`, with the policy defaults.
    pub fn blank(source: &str) -> Self {
        Self {
            name: String::new(),
            source: source.to_string(),
            operations: ChangeOp::ALL.to_vec(),
            filter: String::new(),
            projection: String::new(),
            before_image: false,
            start: StartPosition::SnapshotThenTail,
            checkpoint: String::new(),
            delivery: DeliveryKind::Webhook,
            endpoint: String::new(),
            alias: String::new(),
            backoff: Backoff::Exponential,
            max_retries: "10".into(),
            retention: "7".into(),
            retention_unit: RetentionUnit::Days,
        }
    }

    /// The form that edits `feed`.
    pub fn of(feed: &ChangeFeed) -> Self {
        let (retention, retention_unit) = if feed.retention_hours.is_multiple_of(24) {
            (feed.retention_hours / 24, RetentionUnit::Days)
        } else {
            (feed.retention_hours, RetentionUnit::Hours)
        };
        Self {
            name: feed.name.clone(),
            source: feed.source(),
            operations: feed.operations.clone(),
            filter: feed.filter.clone(),
            projection: feed.projection.clone(),
            before_image: feed.before_image,
            start: feed.start,
            checkpoint: String::new(),
            delivery: feed.delivery,
            // A feed stores its endpoint without the scheme; the form has it.
            endpoint: feed
                .endpoint
                .as_ref()
                .map(|e| format!("https://{e}"))
                .unwrap_or_default(),
            alias: feed.consumer.clone(),
            backoff: feed.backoff,
            max_retries: feed.max_retries.to_string(),
            retention: retention.to_string(),
            retention_unit,
        }
    }

    pub fn toggle(&mut self, op: ChangeOp) {
        if let Some(i) = self.operations.iter().position(|o| *o == op) {
            self.operations.remove(i);
        } else {
            self.operations.push(op);
            self.operations.sort_by_key(|o| *o as u8);
        }
    }

    /// The database part of `source`.
    pub fn database(&self) -> &str {
        self.source.split_once('.').map_or("", |(d, _)| d)
    }

    /// The collection part of `source`.
    pub fn collection(&self) -> &str {
        self.source
            .split_once('.')
            .map_or(self.source.as_str(), |(_, c)| c)
    }

    pub fn max_retries_value(&self) -> Option<u32> {
        self.max_retries.trim().parse().ok().filter(|n| *n <= 100)
    }

    pub fn retention_hours(&self) -> Option<u32> {
        let n: u32 = self.retention.trim().parse().ok()?;
        let hours = self.retention_unit.hours(n);
        (1..=365 * 24).contains(&hours).then_some(hours)
    }

    /// What is wrong with the form. `others` are the feeds of the database,
    /// `editing` the id of the one being edited.
    pub fn issues(&self, others: &[ChangeFeed], editing: Option<&str>) -> Vec<Issue> {
        let mut out = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            out.push(issue(DraftField::Name, "Name the feed."));
        } else if !valid_identifier(name) {
            out.push(issue(
                DraftField::Name,
                "Use lowercase letters, digits, and underscores, starting with a letter.",
            ));
        } else if others
            .iter()
            .any(|f| f.name == name && Some(f.id.as_str()) != editing)
        {
            out.push(issue(
                DraftField::Name,
                format!("A change feed named `{name}` exists. Choose another name."),
            ));
        }
        if self
            .source
            .split_once('.')
            .is_none_or(|(d, c)| d.is_empty() || c.is_empty())
        {
            out.push(issue(DraftField::Source, "Pick a collection."));
        }
        if self.operations.is_empty() {
            out.push(issue(
                DraftField::Operations,
                "Select at least one operation.",
            ));
        }
        if let Err(m) = parse_filter(&self.filter) {
            out.push(issue(DraftField::Filter, m));
        }
        if let Err(m) = parse_projection(&self.projection) {
            out.push(issue(DraftField::Projection, m));
        }
        if self.start == StartPosition::SavedCheckpoint && self.checkpoint.is_empty() {
            out.push(issue(
                DraftField::Checkpoint,
                "Select the checkpoint to resume from.",
            ));
        }
        match self.delivery {
            DeliveryKind::Webhook => {
                if let Err(m) = check_endpoint(&self.endpoint) {
                    out.push(issue(DraftField::Endpoint, m));
                }
            }
            DeliveryKind::ClientSubscription => {}
        }
        let alias = self.alias.trim();
        if alias.is_empty() {
            out.push(issue(DraftField::Alias, "Name the consumer."));
        } else if !valid_identifier(alias) {
            out.push(issue(
                DraftField::Alias,
                "Use lowercase letters, digits, and underscores, starting with a letter.",
            ));
        }
        if self.max_retries_value().is_none() {
            out.push(issue(DraftField::MaxRetries, "Enter 0 to 100 retries."));
        }
        if self.retention_hours().is_none() {
            out.push(issue(
                DraftField::Retention,
                "Enter a retention of 1 hour to 365 days.",
            ));
        }
        out
    }

    /// The fields that differ from `feed`, as `(label, was, now)`.
    pub fn changes_from(&self, feed: &ChangeFeed) -> Vec<(&'static str, String, String)> {
        let before = FeedDraft::of(feed);
        let mut out = Vec::new();
        let mut diff = |label: &'static str, a: String, b: String| {
            if a != b {
                out.push((label, a, b));
            }
        };
        diff("Name", before.name.clone(), self.name.clone());
        diff(
            "Operations",
            ops_label(&before.operations),
            ops_label(&self.operations),
        );
        diff(
            "Filter",
            none_if_empty(&before.filter),
            none_if_empty(&self.filter),
        );
        diff(
            "Projection",
            none_if_empty(&before.projection),
            none_if_empty(&self.projection),
        );
        diff(
            "Before image",
            yes_no(before.before_image),
            yes_no(self.before_image),
        );
        diff(
            "Consumer",
            before.delivery.consumer_label().into(),
            self.delivery.consumer_label().into(),
        );
        diff(
            "Endpoint",
            none_if_empty(&before.endpoint),
            none_if_empty(&self.endpoint),
        );
        diff("Alias", before.alias.clone(), self.alias.clone());
        diff(
            "Retry backoff",
            before.backoff.label().into(),
            self.backoff.label().into(),
        );
        diff(
            "Max retries",
            before.max_retries.clone(),
            self.max_retries.clone(),
        );
        diff(
            "Retention",
            format!(
                "{} {}",
                before.retention,
                before.retention_unit.label().to_lowercase()
            ),
            format!(
                "{} {}",
                self.retention,
                self.retention_unit.label().to_lowercase()
            ),
        );
        out
    }
}

fn ops_label(ops: &[ChangeOp]) -> String {
    ops.iter().map(|o| o.title()).collect::<Vec<_>>().join(", ")
}

fn none_if_empty(s: &str) -> String {
    if s.trim().is_empty() {
        "None".into()
    } else {
        s.trim().to_string()
    }
}

fn yes_no(b: bool) -> String {
    if b { "Included" } else { "Not included" }.into()
}

fn valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && s.len() <= 63
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// A comparison the feed filter grammar accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub field: String,
    pub op: &'static str,
    pub value: String,
}

/// The filter grammar: `field op literal`, joined by `AND`. `op` is one of
/// `=`, `!=`, `<`, `<=`, `>`, `>=`. A literal is a single-quoted string, a
/// number, `true`, or `false`. Anything else is not a supported filter.
pub fn parse_filter(text: &str) -> Result<Vec<Condition>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let unsupported = |part: &str| {
        format!("`{part}` is not a supported filter. Use `field = 'value'`, joined by AND.")
    };
    let mut out = Vec::new();
    for part in split_and(text) {
        let part = part.trim();
        let (field, op, rest) = split_comparison(part).ok_or_else(|| unsupported(part))?;
        if !field
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            || field.is_empty()
        {
            return Err(unsupported(part));
        }
        let rest = rest.trim();
        let quoted = rest.len() >= 2 && rest.starts_with('\'') && rest.ends_with('\'');
        let literal = if quoted {
            !rest[1..rest.len() - 1].contains('\'')
        } else {
            rest.parse::<f64>().is_ok() || rest == "true" || rest == "false"
        };
        if !literal {
            return Err(unsupported(part));
        }
        out.push(Condition {
            field: field.to_string(),
            op,
            value: rest.to_string(),
        });
    }
    Ok(out)
}

/// Split on ` AND ` outside quotes, ignoring case.
fn split_and(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut parts = Vec::new();
    let (mut start, mut i, mut quoted) = (0, 0, false);
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => quoted = !quoted,
            b' ' if !quoted
                && i + 5 <= bytes.len()
                && text.is_char_boundary(i + 5)
                && text[i..i + 5].eq_ignore_ascii_case(" and ") =>
            {
                parts.push(&text[start..i]);
                start = i + 5;
                i += 4;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&text[start..]);
    parts
}

fn split_comparison(part: &str) -> Option<(&str, &'static str, &str)> {
    for op in ["<=", ">=", "!=", "=", "<", ">"] {
        if let Some((a, b)) = part.split_once(op) {
            return Some((a.trim(), op, b));
        }
    }
    None
}

/// Projection fields: comma-separated names, dots for nesting, no repeats.
pub fn parse_projection(text: &str) -> Result<Vec<String>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out: Vec<String> = Vec::new();
    for f in text.split(',').map(str::trim) {
        if f.is_empty() {
            return Err("Remove the empty field between commas.".into());
        }
        if !f
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        {
            return Err(format!("`{f}` is not a field name."));
        }
        if out.iter().any(|g| g == f) {
            return Err(format!("`{f}` appears twice."));
        }
        out.push(f.to_string());
    }
    Ok(out)
}

/// A webhook endpoint: HTTPS, a host, no credentials in the URL. A token in
/// the URL would be shown on screen and logged, so it belongs in a secret
/// reference.
pub fn check_endpoint(url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Enter the endpoint URL.".into());
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return Err("The endpoint must start with https://.".into());
    };
    let (authority, tail) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if authority.is_empty() {
        return Err("The endpoint has no host.".into());
    }
    if authority.contains('@') {
        return Err("Remove the user and password from the URL. Use a secret reference.".into());
    }
    if let Some((_, query)) = tail.split_once('?') {
        let lower = query.to_lowercase();
        if ["token", "key", "secret", "password", "signature", "sig="]
            .iter()
            .any(|w| lower.contains(w))
        {
            return Err("Remove the token from the query string. Use a secret reference.".into());
        }
    }
    if url.chars().any(char::is_whitespace) {
        return Err("The endpoint holds a space.".into());
    }
    Ok(())
}

/// A check the server ran for a preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    pub fn label(self) -> &'static str {
        match self {
            CheckStatus::Pass => "Passed",
            CheckStatus::Warn => "Warning",
            CheckStatus::Fail => "Failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewCheck {
    pub label: String,
    pub status: CheckStatus,
    pub detail: String,
}

/// One node of the event schema the preview shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaNode {
    pub name: String,
    pub ty: String,
    /// A fixed value, such as the database name.
    pub value: Option<String>,
    pub children: Vec<SchemaNode>,
}

/// What the server reports for a draft: its checks, the event shape, and a
/// sample event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedPreview {
    pub checks: Vec<PreviewCheck>,
    pub schema: Vec<SchemaNode>,
    pub sample: String,
    /// Checkpoints a feed on this source saved before.
    pub checkpoints: Vec<String>,
}

impl FeedPreview {
    /// Whether the feed can be created: no check failed.
    pub fn can_create(&self) -> bool {
        self.checks.iter().all(|c| c.status != CheckStatus::Fail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> FeedDraft {
        let mut d = FeedDraft::blank("analytics.sales");
        d.name = "export_sales".into();
        d.endpoint = "https://warehouse.example.com/webhooks/sales".into();
        d.alias = "warehouse_sink".into();
        d
    }

    #[test]
    fn a_complete_form_has_no_issues() {
        assert!(valid().issues(&[], None).is_empty());
    }

    #[test]
    fn the_blank_form_names_what_is_missing() {
        let issues = FeedDraft::blank("analytics.sales").issues(&[], None);
        let at: Vec<_> = issues.iter().map(|i| i.at).collect();
        assert_eq!(
            at,
            [DraftField::Name, DraftField::Endpoint, DraftField::Alias]
        );
    }

    #[test]
    fn a_name_in_use_is_refused_unless_it_is_the_feed_being_edited() {
        let feeds = crate::data::mock::seeded_feeds();
        let mut d = valid();
        d.name = "sales_changes".into();
        assert_eq!(d.issues(&feeds, None).len(), 1);
        let id = feeds[0].id.clone();
        assert!(d.issues(&feeds, Some(&id)).is_empty());
    }

    #[test]
    fn operations_toggle_in_a_stable_order() {
        let mut d = valid();
        d.toggle(ChangeOp::Update);
        assert_eq!(d.operations, [ChangeOp::Insert, ChangeOp::Delete]);
        d.toggle(ChangeOp::Update);
        assert_eq!(d.operations, ChangeOp::ALL);
        d.operations.clear();
        assert_eq!(d.issues(&[], None)[0].at, DraftField::Operations);
    }

    #[test]
    fn the_design_filter_parses() {
        let c = parse_filter("store_id = 'nyc'").expect("supported");
        assert_eq!(
            c,
            [Condition {
                field: "store_id".into(),
                op: "=",
                value: "'nyc'".into()
            }]
        );
        assert_eq!(
            parse_filter("amount >= 100 and status != 'void'").map(|c| c.len()),
            Ok(2)
        );
        assert_eq!(parse_filter("  "), Ok(Vec::new()));
    }

    #[test]
    fn a_quoted_and_is_not_a_separator() {
        assert_eq!(parse_filter("note = 'a AND b'").map(|c| c.len()), Ok(1));
    }

    #[test]
    fn unsupported_filters_are_named() {
        for bad in [
            "store_id",
            "store_id = nyc",
            "a = 'x' OR b = 'y'",
            "= 'x'",
            "a = 'x",
        ] {
            let e = parse_filter(bad).expect_err(bad);
            assert!(e.contains("not a supported filter"), "{bad}: {e}");
        }
    }

    #[test]
    fn projection_rejects_repeats_and_blanks() {
        assert_eq!(
            parse_projection("_id, order_id, store_id"),
            Ok(vec!["_id".into(), "order_id".into(), "store_id".into()])
        );
        assert!(parse_projection("a,,b").is_err());
        assert!(parse_projection("a, a").is_err());
        assert!(parse_projection("a b").is_err());
    }

    #[test]
    fn an_endpoint_never_holds_credentials() {
        assert!(check_endpoint("https://warehouse.example.com/webhooks/sales").is_ok());
        assert!(check_endpoint("http://warehouse.example.com").is_err());
        assert!(check_endpoint("https://user:pw@warehouse.example.com").is_err());
        assert!(check_endpoint("https://warehouse.example.com/h?token=abc").is_err());
        assert!(check_endpoint("https://warehouse.example.com/h?page=2").is_ok());
        assert!(check_endpoint("https:///path").is_err());
        assert!(check_endpoint("").is_err());
    }

    #[test]
    fn a_saved_checkpoint_start_needs_a_checkpoint() {
        let mut d = valid();
        d.start = StartPosition::SavedCheckpoint;
        assert_eq!(d.issues(&[], None)[0].at, DraftField::Checkpoint);
        d.checkpoint = "ckpt_d81".into();
        assert!(d.issues(&[], None).is_empty());
    }

    #[test]
    fn numbers_are_bounded() {
        let mut d = valid();
        d.max_retries = "101".into();
        d.retention = "0".into();
        let at: Vec<_> = d.issues(&[], None).iter().map(|i| i.at).collect();
        assert_eq!(at, [DraftField::MaxRetries, DraftField::Retention]);
        d.retention = "365".into();
        d.max_retries = "0".into();
        assert!(d.issues(&[], None).is_empty());
        d.retention = "366".into();
        assert!(!d.issues(&[], None).is_empty());
    }

    #[test]
    fn an_edit_lists_only_what_changed() {
        let feed = crate::data::mock::seeded_feeds().remove(0);
        let mut d = FeedDraft::of(&feed);
        assert!(d.changes_from(&feed).is_empty());
        d.filter = "store_id = 'nyc'".into();
        d.max_retries = "3".into();
        let changed: Vec<_> = d.changes_from(&feed).iter().map(|c| c.0).collect();
        assert_eq!(changed, ["Filter", "Max retries"]);
    }

    #[test]
    fn a_failed_check_blocks_creation() {
        let p = FeedPreview {
            checks: vec![PreviewCheck {
                label: "Permission".into(),
                status: CheckStatus::Fail,
                detail: String::new(),
            }],
            schema: Vec::new(),
            sample: String::new(),
            checkpoints: Vec::new(),
        };
        assert!(!p.can_create());
    }
}
