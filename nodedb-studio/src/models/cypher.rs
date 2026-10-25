//! The Cypher subset Graph Explorer reads: one linear path pattern with
//! property filters, `RETURN`, and `LIMIT`.
//!
//! ```text
//! MATCH (u:User)-[:POSTED]->(p:Post)-[:TAGGED*1..2]->(t:Topic {name: "databases"})
//! WHERE u.country = "MY" AND p.score >= 3
//! RETURN u, p, t
//! LIMIT 50
//! ```
//!
//! The Builder writes this text and the editor accepts it. The parser names
//! the first thing it cannot read, so the editor can show it before a run.

use std::fmt;

use thiserror::Error;

use super::cypher_lex::{Tok, lex};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum CypherError {
    #[error("the query is empty")]
    Empty,
    #[error("expected {expected}, found {found}")]
    Expected { expected: String, found: String },
    #[error("unterminated string")]
    UnterminatedString,
    #[error("unexpected character `{0}`")]
    Unexpected(char),
    #[error("`{0}` is not a node in the pattern")]
    UnknownVariable(String),
    #[error("LIMIT must be a whole number from 1 up")]
    BadLimit,
    #[error("a path range must read *min..max with 1 <= min <= max <= {max}")]
    BadRange { max: usize },
}

/// The longest variable-length hop the reader accepts.
pub const MAX_RANGE: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Str(String),
    Num(f64),
    Bool(bool),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Str(s) => write!(f, "'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
            Literal::Num(n) => write!(f, "{n}"),
            Literal::Bool(b) => write!(f, "{b}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl Cmp {
    pub fn symbol(self) -> &'static str {
        match self {
            Cmp::Eq => "=",
            Cmp::Ne => "<>",
            Cmp::Gt => ">",
            Cmp::Ge => ">=",
            Cmp::Lt => "<",
            Cmp::Le => "<=",
        }
    }
}

/// `u.country = "MY"`.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub var: String,
    pub field: String,
    pub op: Cmp,
    pub value: Literal,
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{} {} {}",
            self.var,
            self.field,
            self.op.symbol(),
            self.value
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodePattern {
    pub var: String,
    pub label: Option<String>,
    /// `{name: "databases"}`: every pair must equal the node's property.
    pub props: Vec<(String, Literal)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// `-[:R]->`
    Out,
    /// `<-[:R]-`
    In,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    pub relation: Option<String>,
    pub way: Way,
    /// `*1..3`. A plain hop is one step.
    pub min: usize,
    pub max: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub nodes: Vec<NodePattern>,
    /// `hops[i]` joins `nodes[i]` and `nodes[i + 1]`.
    pub hops: Vec<Hop>,
    pub conditions: Vec<Condition>,
    pub returns: Vec<String>,
    pub limit: Option<usize>,
}

struct Reader {
    toks: Vec<Tok>,
    at: usize,
}

impl Reader {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn found(&self) -> String {
        self.peek()
            .map_or_else(|| "the end of the query".to_string(), Tok::to_string)
    }

    fn fail<T>(&self, expected: &str) -> Result<T, CypherError> {
        Err(CypherError::Expected {
            expected: expected.to_string(),
            found: self.found(),
        })
    }

    fn sym(&mut self, s: &'static str) -> bool {
        if self.peek() == Some(&Tok::Sym(s)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn need(&mut self, s: &'static str) -> Result<(), CypherError> {
        if self.sym(s) {
            Ok(())
        } else {
            self.fail(&format!("`{s}`"))
        }
    }

    fn keyword(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(w)) if w.eq_ignore_ascii_case(word)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn ident(&mut self, what: &str) -> Result<String, CypherError> {
        match self.peek() {
            Some(Tok::Ident(w)) => {
                let w = w.clone();
                self.at += 1;
                Ok(w)
            }
            _ => self.fail(what),
        }
    }

    fn literal(&mut self) -> Result<Literal, CypherError> {
        let lit = match self.peek() {
            Some(Tok::Str(s)) => Literal::Str(s.clone()),
            Some(Tok::Num(n)) => Literal::Num(*n),
            Some(Tok::Sym("-")) => {
                self.at += 1;
                return match self.peek() {
                    Some(Tok::Num(n)) => {
                        let n = -*n;
                        self.at += 1;
                        Ok(Literal::Num(n))
                    }
                    _ => self.fail("a number"),
                };
            }
            Some(Tok::Ident(w)) if w.eq_ignore_ascii_case("true") => Literal::Bool(true),
            Some(Tok::Ident(w)) if w.eq_ignore_ascii_case("false") => Literal::Bool(false),
            _ => return self.fail("a value"),
        };
        self.at += 1;
        Ok(lit)
    }

    fn node(&mut self) -> Result<NodePattern, CypherError> {
        self.need("(")?;
        let var = match self.peek() {
            Some(Tok::Ident(_)) => self.ident("a variable")?,
            _ => String::new(),
        };
        let label = if self.sym(":") {
            Some(self.ident("a label")?)
        } else {
            None
        };
        let mut props = Vec::new();
        if self.sym("{") {
            loop {
                let key = self.ident("a property name")?;
                self.need(":")?;
                props.push((key, self.literal()?));
                if !self.sym(",") {
                    break;
                }
            }
            self.need("}")?;
        }
        self.need(")")?;
        Ok(NodePattern { var, label, props })
    }

    fn range(&mut self) -> Result<(usize, usize), CypherError> {
        let whole = |r: &mut Reader| match r.peek() {
            Some(Tok::Num(n)) if n.fract() == 0.0 && *n >= 1.0 => {
                let n = *n as usize;
                r.at += 1;
                Ok(n)
            }
            _ => Err(CypherError::BadRange { max: MAX_RANGE }),
        };
        let min = whole(self)?;
        let max = if self.sym("..") { whole(self)? } else { min };
        if min > max || max > MAX_RANGE {
            return Err(CypherError::BadRange { max: MAX_RANGE });
        }
        Ok((min, max))
    }

    fn hop(&mut self) -> Result<Hop, CypherError> {
        let way = if self.sym("<-") {
            Way::In
        } else if self.sym("-") {
            Way::Out
        } else {
            return self.fail("`-` or `<-`");
        };
        let (mut relation, mut min, mut max) = (None, 1, 1);
        if self.sym("[") {
            if self.sym(":") {
                relation = Some(self.ident("a relationship type")?);
            }
            if self.sym("*") {
                (min, max) = self.range()?;
            }
            self.need("]")?;
        }
        match way {
            Way::Out => self.need("->")?,
            Way::In => self.need("-")?,
        }
        Ok(Hop {
            relation,
            way,
            min,
            max,
        })
    }

    fn condition(&mut self) -> Result<Condition, CypherError> {
        let var = self.ident("a variable")?;
        self.need(".")?;
        let field = self.ident("a property name")?;
        let op = match self.peek() {
            Some(Tok::Sym("=")) => Cmp::Eq,
            Some(Tok::Sym("<>")) => Cmp::Ne,
            Some(Tok::Sym(">")) => Cmp::Gt,
            Some(Tok::Sym(">=")) => Cmp::Ge,
            Some(Tok::Sym("<")) => Cmp::Lt,
            Some(Tok::Sym("<=")) => Cmp::Le,
            _ => return self.fail("a comparison such as `=` or `>`"),
        };
        self.at += 1;
        Ok(Condition {
            var,
            field,
            op,
            value: self.literal()?,
        })
    }

    fn conditions(&mut self, out: &mut Vec<Condition>) -> Result<(), CypherError> {
        out.push(self.condition()?);
        while self.keyword("AND") {
            out.push(self.condition()?);
        }
        Ok(())
    }
}

/// Parse a query. Variables the pattern does not bind are an error, as is
/// anything after `LIMIT`.
pub fn parse(text: &str) -> Result<Pattern, CypherError> {
    let toks = lex(text)?;
    if toks.is_empty() {
        return Err(CypherError::Empty);
    }
    let mut r = Reader { toks, at: 0 };
    if !r.keyword("MATCH") {
        return r.fail("`MATCH`");
    }
    let mut nodes = vec![r.node()?];
    let mut hops = Vec::new();
    while matches!(r.peek(), Some(Tok::Sym("-" | "<-"))) {
        hops.push(r.hop()?);
        nodes.push(r.node()?);
    }
    let mut conditions = Vec::new();
    if r.keyword("WHERE") {
        r.conditions(&mut conditions)?;
    }
    if !r.keyword("RETURN") {
        return r.fail("`RETURN`");
    }
    let mut returns = vec![r.ident("a variable to return")?];
    while r.sym(",") {
        returns.push(r.ident("a variable to return")?);
    }
    let mut limit = None;
    if r.keyword("LIMIT") {
        match r.peek() {
            Some(Tok::Num(n)) if n.fract() == 0.0 && *n >= 1.0 => {
                limit = Some(*n as usize);
                r.at += 1;
            }
            _ => return Err(CypherError::BadLimit),
        }
    }
    if r.peek().is_some() {
        return r.fail("the end of the query");
    }
    let bound = |v: &str| nodes.iter().any(|n| n.var == v);
    for v in conditions.iter().map(|c| &c.var).chain(returns.iter()) {
        if !bound(v) {
            return Err(CypherError::UnknownVariable(v.clone()));
        }
    }
    Ok(Pattern {
        nodes,
        hops,
        conditions,
        returns,
        limit,
    })
}

/// A filter a person types in a form: `status = 'active' AND age > 30`.
/// Fields belong to `var`.
pub fn parse_filter(text: &str, var: &str) -> Result<Vec<Condition>, CypherError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    // Reuse the condition reader by qualifying each field with `var`.
    let toks = lex(trimmed)?;
    let mut qualified = Vec::with_capacity(toks.len() + 4);
    let mut expect_field = true;
    for t in toks {
        let is_and = matches!(&t, Tok::Ident(w) if w.eq_ignore_ascii_case("AND"));
        if expect_field && !is_and {
            qualified.push(Tok::Ident(var.to_string()));
            qualified.push(Tok::Sym("."));
            expect_field = false;
        }
        if is_and {
            expect_field = true;
        }
        qualified.push(t);
    }
    let mut r = Reader {
        toks: qualified,
        at: 0,
    };
    let mut out = Vec::new();
    r.conditions(&mut out)?;
    if r.peek().is_some() {
        return r.fail("`AND` or the end of the filter");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESIGN: &str = "MATCH (u:User)-[:POSTED]->(p:Post)\n      -[:TAGGED]->(t:Topic {name: \"databases\"})\nWHERE u.country = \"MY\"\nRETURN u, p, t\nLIMIT 50";

    #[test]
    fn the_design_query_parses_into_its_pattern() {
        let p = parse(DESIGN).expect("parses");
        assert_eq!(p.nodes.len(), 3);
        assert_eq!(p.nodes[2].label.as_deref(), Some("Topic"));
        assert_eq!(
            p.nodes[2].props,
            [("name".to_string(), Literal::Str("databases".into()))]
        );
        assert_eq!(p.hops.len(), 2);
        assert_eq!(p.hops[0].relation.as_deref(), Some("POSTED"));
        assert_eq!(p.hops[0].way, Way::Out);
        assert_eq!(p.conditions.len(), 1);
        assert_eq!(p.returns, ["u", "p", "t"]);
        assert_eq!(p.limit, Some(50));
    }

    #[test]
    fn inbound_hops_and_ranges_parse() {
        let p = parse("MATCH (o:Order)<-[:PURCHASED*1..3]-(u:User) RETURN o, u").expect("parses");
        assert_eq!(p.hops[0].way, Way::In);
        assert_eq!((p.hops[0].min, p.hops[0].max), (1, 3));
        assert_eq!(p.limit, None);
    }

    #[test]
    fn keywords_ignore_case() {
        assert!(parse("match (a) return a limit 3").is_ok());
    }

    #[test]
    fn errors_name_what_was_expected() {
        assert_eq!(parse("  "), Err(CypherError::Empty));
        assert!(matches!(
            parse("MATCH (u:User RETURN u"),
            Err(CypherError::Expected { .. })
        ));
        assert!(matches!(
            parse("MATCH (u) WHERE x.a = 1 RETURN u"),
            Err(CypherError::UnknownVariable(v)) if v == "x"
        ));
        assert_eq!(
            parse("MATCH (u) RETURN u LIMIT 0"),
            Err(CypherError::BadLimit)
        );
        assert!(matches!(
            parse("MATCH (a)-[:R*3..2]->(b) RETURN a"),
            Err(CypherError::BadRange { .. })
        ));
        assert_eq!(
            parse("MATCH (u) WHERE u.n = 'x RETURN u"),
            Err(CypherError::UnterminatedString)
        );
        assert!(matches!(
            parse("MATCH (u) RETURN u extra"),
            Err(CypherError::Expected { .. })
        ));
    }

    #[test]
    fn a_filter_qualifies_each_field() {
        let c = parse_filter("status = 'active' AND score >= 3", "u").expect("parses");
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].to_string(), "u.status = 'active'");
        assert_eq!(c[1].to_string(), "u.score >= 3");
        assert_eq!(parse_filter("  ", "u"), Ok(Vec::new()));
        assert!(parse_filter("status", "u").is_err());
        assert!(parse_filter("status = ", "u").is_err());
    }

    #[test]
    fn a_string_with_a_quote_survives_display_and_parse() {
        let c = parse_filter("name = 'O\\'Neil'", "u").expect("parses");
        assert_eq!(c[0].value, Literal::Str("O'Neil".into()));
        let again = parse_filter(&format!("name = {}", c[0].value), "u").expect("parses");
        assert_eq!(again, c);
    }
}
