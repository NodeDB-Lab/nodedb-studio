//! The Builder's form and the Cypher it writes.
//!
//! The form is the source of truth. `cypher` renders it as query text, and
//! the same text goes to the server when the user presses Run, so the
//! preview is what runs. Tests parse the text back and compare it with the
//! form.

use crate::models::cypher::{Condition, CypherError, parse_filter};
use crate::models::graph::{NodeKind, Relation};

/// Which way the relationship points, seen from the start node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Outbound,
    Inbound,
}

impl Direction {
    pub const ALL: [Direction; 2] = [Direction::Outbound, Direction::Inbound];

    pub fn label(self) -> &'static str {
        match self {
            Direction::Outbound => "Outbound",
            Direction::Inbound => "Inbound",
        }
    }
}

pub const MAX_DEPTH: u8 = 4;
pub const MAX_LIMIT: u32 = 1000;

#[derive(Debug, Clone, PartialEq)]
pub struct BuilderForm {
    pub start: NodeKind,
    pub start_filter: String,
    pub relation: Relation,
    pub direction: Direction,
    pub end: NodeKind,
    pub end_filter: String,
    pub max_depth: u8,
    pub limit: u32,
}

impl Default for BuilderForm {
    fn default() -> Self {
        Self {
            start: NodeKind::User,
            start_filter: "status = 'active'".to_string(),
            relation: Relation::Purchased,
            direction: Direction::Outbound,
            end: NodeKind::Order,
            end_filter: "total > 100".to_string(),
            max_depth: 1,
            limit: 100,
        }
    }
}

/// A field the form refuses, with the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    StartFilter,
    EndFilter,
    Relation,
    MaxDepth,
    Limit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub field: Field,
    pub message: String,
}

impl BuilderForm {
    /// The two variable names: the first letter of each kind, the second
    /// suffixed when both kinds share a letter.
    fn vars(&self) -> (String, String) {
        let letter = |k: NodeKind| {
            k.label()
                .chars()
                .next()
                .map_or('n', |c| c.to_ascii_lowercase())
                .to_string()
        };
        let (a, b) = (letter(self.start), letter(self.end));
        let b = if a == b { format!("{b}2") } else { b };
        (a, b)
    }

    /// The conditions of both filters, or the first reason one is refused.
    fn conditions(&self) -> Result<Vec<Condition>, Vec<Issue>> {
        let (a, b) = self.vars();
        let mut issues = Vec::new();
        let mut out = Vec::new();
        for (text, var, field) in [
            (&self.start_filter, &a, Field::StartFilter),
            (&self.end_filter, &b, Field::EndFilter),
        ] {
            match parse_filter(text, var) {
                Ok(c) => out.extend(c),
                Err(e) => issues.push(Issue {
                    field,
                    message: filter_message(&e),
                }),
            }
        }
        if issues.is_empty() {
            Ok(out)
        } else {
            Err(issues)
        }
    }

    /// Every reason the form cannot run. Empty means it can.
    pub fn issues(&self) -> Vec<Issue> {
        let mut issues = self.conditions().err().unwrap_or_default();
        let (from, to) = self.relation.ends();
        let (want_start, want_end) = match self.direction {
            Direction::Outbound => (from, to),
            Direction::Inbound => (to, from),
        };
        if (self.start, self.end) != (want_start, want_end) {
            issues.push(Issue {
                field: Field::Relation,
                message: format!(
                    "{} joins {} to {}. {} from {} reaches {}.",
                    self.relation.label(),
                    from.label(),
                    to.label(),
                    self.direction.label(),
                    want_start.label(),
                    want_end.label()
                ),
            });
        }
        if !(1..=MAX_DEPTH).contains(&self.max_depth) {
            issues.push(Issue {
                field: Field::MaxDepth,
                message: format!("Max depth runs from 1 to {MAX_DEPTH}."),
            });
        }
        if !(1..=MAX_LIMIT).contains(&self.limit) {
            issues.push(Issue {
                field: Field::Limit,
                message: format!("Limit runs from 1 to {MAX_LIMIT}."),
            });
        }
        issues
    }

    pub fn issue(&self, field: Field) -> Option<String> {
        self.issues()
            .into_iter()
            .find(|i| i.field == field)
            .map(|i| i.message)
    }

    /// The query text, or the form's issues. Never text that would not parse.
    pub fn cypher(&self) -> Result<String, Vec<Issue>> {
        let issues = self.issues();
        if !issues.is_empty() {
            return Err(issues);
        }
        let conditions = self.conditions()?;
        let (a, b) = self.vars();
        let depth = if self.max_depth > 1 {
            format!("*1..{}", self.max_depth)
        } else {
            String::new()
        };
        let rel = self.relation.label();
        let hop = match self.direction {
            Direction::Outbound => format!("-[:{rel}{depth}]->"),
            Direction::Inbound => format!("<-[:{rel}{depth}]-"),
        };
        let mut text = format!(
            "MATCH ({a}:{}){hop}({b}:{})",
            self.start.query_label(),
            self.end.query_label()
        );
        if !conditions.is_empty() {
            let joined: Vec<String> = conditions.iter().map(Condition::to_string).collect();
            text.push_str(&format!("\nWHERE {}", joined.join(" AND ")));
        }
        text.push_str(&format!("\nRETURN {a}, {b}\nLIMIT {}", self.limit));
        Ok(text)
    }
}

fn filter_message(e: &CypherError) -> String {
    format!("Filter: {e}. Write field = 'text' or field > 10, joined by AND.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cypher::{Way, parse};

    #[test]
    fn the_default_form_writes_the_design_query() {
        let text = BuilderForm::default().cypher().expect("valid");
        assert_eq!(
            text,
            "MATCH (u:Users)-[:PURCHASED]->(o:Orders)\nWHERE u.status = 'active' AND o.total > 100\nRETURN u, o\nLIMIT 100"
        );
    }

    #[test]
    fn the_text_parses_back_into_the_form() {
        let form = BuilderForm {
            max_depth: 2,
            ..BuilderForm::default()
        };
        let p = parse(&form.cypher().expect("valid")).expect("parses");
        assert_eq!(p.nodes.len(), 2);
        assert_eq!(p.hops[0].relation.as_deref(), Some("PURCHASED"));
        assert_eq!((p.hops[0].min, p.hops[0].max), (1, 2));
        assert_eq!(p.conditions.len(), 2);
        assert_eq!(p.limit, Some(100));
        assert_eq!(
            NodeKind::from_label(p.nodes[0].label.as_deref().unwrap_or("")),
            Some(form.start)
        );
    }

    #[test]
    fn an_inbound_form_writes_a_reversed_arrow() {
        let form = BuilderForm {
            start: NodeKind::Order,
            end: NodeKind::User,
            direction: Direction::Inbound,
            start_filter: String::new(),
            end_filter: String::new(),
            ..BuilderForm::default()
        };
        let text = form.cypher().expect("valid");
        assert!(text.starts_with("MATCH (o:Orders)<-[:PURCHASED]-(u:Users)\nRETURN o, u"));
        let p = parse(&text).expect("parses");
        assert_eq!(p.hops[0].way, Way::In);
    }

    #[test]
    fn two_nodes_of_one_kind_get_distinct_variables() {
        let form = BuilderForm {
            start: NodeKind::User,
            end: NodeKind::User,
            relation: Relation::Follows,
            start_filter: String::new(),
            end_filter: "country = 'MY'".into(),
            ..BuilderForm::default()
        };
        let text = form.cypher().expect("valid");
        assert!(text.contains("(u:Users)-[:FOLLOWS]->(u2:Users)"), "{text}");
        assert!(text.contains("WHERE u2.country = 'MY'"), "{text}");
        assert!(parse(&text).is_ok());
    }

    #[test]
    fn a_relation_that_does_not_join_the_chosen_kinds_is_refused() {
        let form = BuilderForm {
            relation: Relation::Contains,
            ..BuilderForm::default()
        };
        let issue = form.issue(Field::Relation).expect("refused");
        assert!(issue.contains("CONTAINS joins Order to Product"), "{issue}");
        assert!(form.cypher().is_err());
    }

    #[test]
    fn a_bad_filter_names_its_field() {
        let form = BuilderForm {
            end_filter: "total >".into(),
            ..BuilderForm::default()
        };
        assert!(form.issue(Field::EndFilter).is_some());
        assert!(form.issue(Field::StartFilter).is_none());
    }

    #[test]
    fn depth_and_limit_have_bounds() {
        let form = BuilderForm {
            max_depth: 0,
            limit: 0,
            ..BuilderForm::default()
        };
        assert!(form.issue(Field::MaxDepth).is_some());
        assert!(form.issue(Field::Limit).is_some());
        let big = BuilderForm {
            limit: MAX_LIMIT + 1,
            ..BuilderForm::default()
        };
        assert!(big.issue(Field::Limit).is_some());
    }
}
