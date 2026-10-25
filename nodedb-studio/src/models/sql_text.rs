//! A small lexer for the editor: keywords, functions, strings, numbers,
//! comments, and parameters. It colors text and finds the `:name`
//! parameters of a statement. It does not parse.

use crate::models::console::Protocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Function,
    Str,
    Number,
    Comment,
    /// `:name` in SQL, `$name` in Cypher.
    Param,
    /// A type after `::`, or a Cypher label after `:`.
    Type,
    Ident,
    /// Whitespace and punctuation.
    Plain,
}

impl TokenKind {
    /// The CSS class of the token, in `controls.css`.
    pub fn class(self) -> &'static str {
        match self {
            TokenKind::Keyword | TokenKind::Type => "tok-kw",
            TokenKind::Function => "tok-func",
            TokenKind::Str => "tok-str",
            TokenKind::Number => "tok-num",
            TokenKind::Comment => "tok-com",
            TokenKind::Param => "tok-param",
            TokenKind::Ident | TokenKind::Plain => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub text: &'a str,
}

const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "GROUP",
    "BY",
    "ORDER",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "OUTER",
    "CROSS",
    "ON",
    "USING",
    "AS",
    "AND",
    "OR",
    "NOT",
    "IN",
    "IS",
    "NULL",
    "LIKE",
    "ILIKE",
    "BETWEEN",
    "EXISTS",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "DISTINCT",
    "UNION",
    "ALL",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "ALTER",
    "DROP",
    "TABLE",
    "INDEX",
    "COLLECTION",
    "WITH",
    "ASC",
    "DESC",
    "TRUE",
    "FALSE",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "START",
    "TRANSACTION",
    "EXPLAIN",
    "ANALYZE",
    "INT",
    "INTEGER",
    "TEXT",
    "BOOLEAN",
    "TIMESTAMP",
    "INTERVAL",
    "DECIMAL",
    "NUMERIC",
    "VARCHAR",
    "JSONB",
    "UUID",
];

const CYPHER_KEYWORDS: &[&str] = &[
    "MATCH", "OPTIONAL", "WHERE", "RETURN", "WITH", "ORDER", "BY", "LIMIT", "SKIP", "CREATE",
    "MERGE", "DELETE", "DETACH", "SET", "REMOVE", "UNWIND", "AS", "AND", "OR", "NOT", "IN", "IS",
    "NULL", "DISTINCT", "ASC", "DESC", "CASE", "WHEN", "THEN", "ELSE", "END", "EXISTS", "TRUE",
    "FALSE", "STARTS", "ENDS", "CONTAINS", "EXPLAIN",
];

const ADMIN_KEYWORDS: &[&str] = &[
    "SHOW",
    "CREATE",
    "ALTER",
    "DROP",
    "GRANT",
    "REVOKE",
    "USE",
    "SET",
    "DATABASES",
    "DATABASE",
    "CLUSTER",
    "STATUS",
    "PEER",
    "NODES",
    "GROUPS",
    "ROLES",
    "USERS",
    "SESSIONS",
    "SLOW",
    "QUERIES",
    "SINCE",
    "FOR",
    "ON",
    "TO",
    "FROM",
    "KILL",
    "SESSION",
];

fn keywords(protocol: Protocol) -> &'static [&'static str] {
    match protocol {
        Protocol::Sql => SQL_KEYWORDS,
        Protocol::Cypher => CYPHER_KEYWORDS,
        Protocol::Admin => ADMIN_KEYWORDS,
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The byte length of the run at the start of `s` whose chars satisfy `f`.
fn run(s: &str, f: impl Fn(char) -> bool) -> usize {
    s.char_indices()
        .find(|(_, c)| !f(*c))
        .map_or(s.len(), |(i, _)| i)
}

/// Split `text` into tokens. The tokens cover `text` exactly, in order.
pub fn tokenize(text: &str, protocol: Protocol) -> Vec<Token<'_>> {
    let line_comment = match protocol {
        Protocol::Cypher => "//",
        Protocol::Sql | Protocol::Admin => "--",
    };
    let words = keywords(protocol);
    let mut out: Vec<Token<'_>> = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        let Some(c) = rest.chars().next() else { break };
        let (kind, len) = if rest.starts_with(line_comment) {
            (TokenKind::Comment, run(rest, |c| c != '\n'))
        } else if c == '\'' {
            (TokenKind::Str, quoted(rest, '\''))
        } else if c == '"' {
            (TokenKind::Ident, quoted(rest, '"'))
        } else if c.is_ascii_digit() {
            (
                TokenKind::Number,
                run(rest, |c| c.is_ascii_digit() || c == '.'),
            )
        } else if rest.starts_with("::") {
            (TokenKind::Plain, 2)
        } else if c == ':' && protocol == Protocol::Cypher {
            let n = run(&rest[1..], is_ident);
            if n > 0 {
                (TokenKind::Type, 1 + n)
            } else {
                (TokenKind::Plain, 1)
            }
        } else if (c == ':' && protocol != Protocol::Cypher)
            || (c == '$' && protocol == Protocol::Cypher)
        {
            let n = rest[1..]
                .chars()
                .next()
                .filter(|c| is_ident_start(*c))
                .map_or(0, |_| run(&rest[1..], is_ident));
            if n > 0 {
                (TokenKind::Param, 1 + n)
            } else {
                (TokenKind::Plain, 1)
            }
        } else if is_ident_start(c) {
            let n = run(rest, is_ident);
            let word = &rest[..n];
            let after_cast = out.last().is_some_and(|t| t.text == "::");
            let kind = if after_cast {
                TokenKind::Type
            } else if words.iter().any(|k| k.eq_ignore_ascii_case(word)) {
                TokenKind::Keyword
            } else if rest[n..].trim_start_matches([' ', '\t']).starts_with('(') {
                TokenKind::Function
            } else {
                TokenKind::Ident
            };
            (kind, n)
        } else {
            (
                TokenKind::Plain,
                run(rest, |c| {
                    !(is_ident(c) || matches!(c, '\'' | '"' | ':' | '$' | '-' | '/'))
                })
                .max(c.len_utf8()),
            )
        };
        out.push(Token {
            kind,
            text: &rest[..len],
        });
        at += len;
    }
    out
}

/// The length of a quoted run at the start of `s`. A doubled quote inside
/// is an escape. An unclosed quote runs to the end of the line.
fn quoted(s: &str, quote: char) -> usize {
    let mut chars = s.char_indices().skip(1).peekable();
    while let Some((i, c)) = chars.next() {
        if c == quote {
            if chars.peek().is_some_and(|(_, n)| *n == quote) {
                chars.next();
                continue;
            }
            return i + 1;
        }
        if c == '\n' {
            return i;
        }
    }
    s.len()
}

/// The distinct `:name` parameters of a SQL statement, in order of first use.
pub fn parameters(sql: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for t in tokenize(sql, Protocol::Sql) {
        if t.kind == TokenKind::Param {
            let name = &t.text[1..];
            if !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str, p: Protocol) -> Vec<(TokenKind, &str)> {
        tokenize(text, p)
            .into_iter()
            .filter(|t| t.kind != TokenKind::Plain)
            .map(|t| (t.kind, t.text))
            .collect()
    }

    #[test]
    fn tokens_cover_the_text_exactly() {
        let sql = "-- c\nSELECT a, SUM(b)::int FROM t WHERE x = ':y' AND z > :p;\n";
        let joined: String = tokenize(sql, Protocol::Sql)
            .iter()
            .map(|t| t.text)
            .collect();
        assert_eq!(joined, sql);
    }

    #[test]
    fn keywords_functions_and_casts_have_their_own_kinds() {
        let got = kinds("SELECT COUNT(*) AS n, ts::interval", Protocol::Sql);
        assert_eq!(
            got,
            vec![
                (TokenKind::Keyword, "SELECT"),
                (TokenKind::Function, "COUNT"),
                (TokenKind::Keyword, "AS"),
                (TokenKind::Ident, "n"),
                (TokenKind::Ident, "ts"),
                (TokenKind::Type, "interval"),
            ]
        );
    }

    #[test]
    fn a_cast_is_not_a_parameter() {
        assert!(parameters("SELECT x::int, y::text").is_empty());
    }

    #[test]
    fn parameters_skip_strings_and_comments_and_repeat_once() {
        let sql = "SELECT 1 -- :ignored\nWHERE a = :cur AND b = ':nope' AND c = :cur AND d = :n2";
        assert_eq!(parameters(sql), vec!["cur", "n2"]);
    }

    #[test]
    fn a_doubled_quote_stays_inside_the_string() {
        let got = kinds("SELECT 'it''s'", Protocol::Sql);
        assert_eq!(got[1], (TokenKind::Str, "'it''s'"));
    }

    #[test]
    fn cypher_labels_and_parameters() {
        let got = kinds(
            "MATCH (u:User) WHERE u.id = $id RETURN u // done",
            Protocol::Cypher,
        );
        assert!(got.contains(&(TokenKind::Type, ":User")));
        assert!(got.contains(&(TokenKind::Param, "$id")));
        assert!(got.contains(&(TokenKind::Comment, "// done")));
    }

    #[test]
    fn non_ascii_text_does_not_split_a_char() {
        let text = "SELECT 'héllo' -- ünï";
        let joined: String = tokenize(text, Protocol::Sql)
            .iter()
            .map(|t| t.text)
            .collect();
        assert_eq!(joined, text);
    }
}
