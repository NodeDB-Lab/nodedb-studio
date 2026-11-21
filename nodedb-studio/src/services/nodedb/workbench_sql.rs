//! Statement text, read the way NodeDB 0.6.2 reads it before it runs it.
//!
//! `control_of` mirrors the server's transaction-control classifier
//! (`control/server/shared/txn_control` at v0.6.2): the same tokens and the
//! same grammar. Comments, whitespace and a closing `;` are skipped, keywords
//! match in any case, and an unquoted savepoint name folds to lower case. A
//! statement the server runs as transaction control is one Studio tracks as
//! transaction control. The one exception is BEGIN: any BEGIN counts, so no
//! spelling the server accepts can open a transaction on a pooled connection.
//!
//! `first_word` and `is_plain_read` are Studio's own coarse checks.

use std::iter::Peekable;
use std::str::Chars;

/// One token of the transaction-control grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// An unquoted word. `upper` matches keywords. `folded` is the name the
    /// server gives an unquoted identifier.
    Word {
        upper: String,
        folded: String,
    },
    /// A double-quoted identifier, its case kept.
    Quoted(String),
    Comma,
}

impl Token {
    fn is(&self, keyword: &str) -> bool {
        matches!(self, Token::Word { upper, .. } if upper == keyword)
    }

    fn name(&self) -> Option<&str> {
        match self {
            Token::Word { folded, .. } => Some(folded),
            Token::Quoted(name) => Some(name),
            Token::Comma => None,
        }
    }
}

/// Skip the comment `c` opens. `Some(true)` when `c` opened one,
/// `Some(false)` when it did not, `None` for an unclosed block comment.
fn comment(c: char, chars: &mut Peekable<Chars<'_>>) -> Option<bool> {
    if c == '-' && chars.peek() == Some(&'-') {
        chars.by_ref().find(|&n| n == '\n');
        return Some(true);
    }
    if c == '/' && chars.peek() == Some(&'*') {
        chars.next();
        let mut depth = 1usize;
        while depth > 0 {
            match chars.next()? {
                '*' if chars.peek() == Some(&'/') => {
                    chars.next();
                    depth -= 1;
                }
                '/' if chars.peek() == Some(&'*') => {
                    chars.next();
                    depth += 1;
                }
                _ => {}
            }
        }
        return Some(true);
    }
    Some(false)
}

/// The word that starts with `first`.
fn word(first: char, chars: &mut Peekable<Chars<'_>>) -> String {
    let mut word = String::from(first);
    while let Some(&n) = chars
        .peek()
        .filter(|n| n.is_alphanumeric() || **n == '_' || **n == '$')
    {
        word.push(n);
        chars.next();
    }
    word
}

/// The tokens of `sql`. `None` when anything follows a `;`, a character is
/// outside the grammar, or a quote or comment is left open.
fn tokenize(sql: &str) -> Option<Vec<Token>> {
    let mut out = Vec::new();
    let mut chars = sql.chars().peekable();
    let mut ended = false;
    while let Some(c) = chars.next() {
        if c.is_whitespace() || comment(c, &mut chars)? {
            continue;
        }
        if c == ';' {
            ended = true;
            continue;
        }
        if ended {
            return None;
        }
        if c == ',' {
            out.push(Token::Comma);
        } else if c == '"' {
            let mut name = String::new();
            loop {
                match chars.next()? {
                    '"' if chars.peek() == Some(&'"') => {
                        chars.next();
                        name.push('"');
                    }
                    '"' => break,
                    other => name.push(other),
                }
            }
            if name.is_empty() {
                return None;
            }
            out.push(Token::Quoted(name));
        } else if c.is_alphabetic() || c == '_' {
            let w = word(c, &mut chars);
            out.push(Token::Word {
                upper: w.to_ascii_uppercase(),
                folded: w.to_ascii_lowercase(),
            });
        } else {
            return None;
        }
    }
    Some(out)
}

/// Transaction control written as a statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Control {
    Begin,
    Commit,
    Rollback,
    Savepoint(String),
    Release(String),
    RollbackTo(String),
}

/// Drop one leading `WORK` or `TRANSACTION`.
fn skip_noise(tokens: &[Token]) -> &[Token] {
    match tokens.split_first() {
        Some((first, rest)) if first.is("WORK") || first.is("TRANSACTION") => rest,
        _ => tokens,
    }
}

/// Nothing, or exactly `AND NO CHAIN`.
fn ends(tokens: &[Token]) -> bool {
    match tokens {
        [] => true,
        [and, no, chain] => and.is("AND") && no.is("NO") && chain.is("CHAIN"),
        _ => false,
    }
}

/// The one savepoint name `tokens` holds, after an optional `SAVEPOINT`
/// keyword when `keyword_optional` is set.
fn savepoint_name(tokens: &[Token], keyword_optional: bool) -> Option<String> {
    let tokens = match tokens {
        [first, rest @ ..] if keyword_optional && first.is("SAVEPOINT") && !rest.is_empty() => rest,
        _ => tokens,
    };
    match tokens {
        [name] => name.name().map(str::to_owned),
        _ => None,
    }
}

/// The transaction control `sql` spells, or `None` for any other statement.
pub(super) fn control_of(sql: &str) -> Option<Control> {
    let tokens = tokenize(sql)?;
    let (head, rest) = tokens.split_first()?;
    let Token::Word { upper, .. } = head else {
        return None;
    };
    match upper.as_str() {
        "BEGIN" => Some(Control::Begin),
        "START" => rest
            .first()
            .filter(|t| t.is("TRANSACTION"))
            .map(|_| Control::Begin),
        "COMMIT" | "END" => ends(skip_noise(rest)).then_some(Control::Commit),
        "ABORT" => ends(skip_noise(rest)).then_some(Control::Rollback),
        "ROLLBACK" => {
            let rest = skip_noise(rest);
            match rest.split_first() {
                Some((to, target)) if to.is("TO") => {
                    savepoint_name(target, true).map(Control::RollbackTo)
                }
                _ => ends(rest).then_some(Control::Rollback),
            }
        }
        "SAVEPOINT" => savepoint_name(rest, false).map(Control::Savepoint),
        "RELEASE" => savepoint_name(rest, true).map(Control::Release),
        _ => None,
    }
}

/// Whether a BEGIN asks for a read-only transaction.
pub(super) fn asks_read_only(sql: &str) -> bool {
    tokenize(sql).is_some_and(|t| t.windows(2).any(|p| p[0].is("READ") && p[1].is("ONLY")))
}

/// The first word of `sql`, upper-cased, after whitespace, comments and
/// `;`. `None` when the text starts with anything else.
pub(super) fn first_word(sql: &str) -> Option<String> {
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() || c == ';' || comment(c, &mut chars)? {
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            return Some(word(c, &mut chars).to_ascii_uppercase());
        }
        return None;
    }
    None
}

/// Whether `sql` can reset the server session. 0.6.2 answers `DISCARD ALL`
/// by dropping the session's state, the open transaction with it.
pub(super) fn resets_session(sql: &str) -> bool {
    first_word(sql).as_deref() == Some("DISCARD")
}

/// The first words of a statement that only reads.
const READS: &[&str] = &["SELECT", "SHOW", "DESCRIBE", "EXPLAIN", "WITH"];

/// Words that can write, run a statement, or change the session or the
/// transaction. One anywhere in the text, even in a string or a comment,
/// makes the statement not a read.
const WRITES: &[&str] = &[
    "INSERT",
    "UPDATE",
    "DELETE",
    "UPSERT",
    "MERGE",
    "INTO",
    "CREATE",
    "DROP",
    "ALTER",
    "TRUNCATE",
    "GRANT",
    "REVOKE",
    "COPY",
    "CALL",
    "DO",
    "EXECUTE",
    "PREPARE",
    "NEXTVAL",
    "SETVAL",
    "ANALYZE",
    "VACUUM",
    "REFRESH",
    "BEGIN",
    "START",
    "COMMIT",
    "ROLLBACK",
    "ABORT",
    "SAVEPOINT",
    "RELEASE",
    "DISCARD",
    "RESET",
    "SET",
    "LISTEN",
    "NOTIFY",
    "KILL",
    "LOCK",
];

/// Whether `sql` is one statement that only reads, so running it again
/// changes nothing. Unsure is not a read: a second statement, a first word
/// outside `READS`, or any word of `WRITES` anywhere.
pub(super) fn is_plain_read(sql: &str) -> bool {
    let one_statement = !sql.trim_end().trim_end_matches(';').contains(';');
    one_statement
        && first_word(sql).is_some_and(|w| READS.contains(&w.as_str()))
        && !sql
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|w| WRITES.iter().any(|k| k.eq_ignore_ascii_case(w)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_in_any_spelling_is_control() {
        for sql in [
            "BEGIN",
            "begin;",
            "Begin Work",
            "START TRANSACTION",
            "start transaction isolation level serializable, read only ;",
            "-- open\nBEGIN /* a /* nested */ note */",
            "BEGIN FOO",
        ] {
            assert_eq!(control_of(sql), Some(Control::Begin), "{sql}");
        }
    }

    #[test]
    fn commit_and_rollback_follow_the_server_grammar() {
        for sql in [
            "COMMIT",
            "commit work",
            "END TRANSACTION;",
            "COMMIT AND NO CHAIN",
            "/* c */ COMMIT",
            "-- c\ncommit",
        ] {
            assert_eq!(control_of(sql), Some(Control::Commit), "{sql}");
        }
        for sql in ["ROLLBACK", "abort work", "ROLLBACK -- end the block\n"] {
            assert_eq!(control_of(sql), Some(Control::Rollback), "{sql}");
        }
    }

    #[test]
    fn savepoints_follow_the_server_grammar() {
        let sp = |n: &str| Some(Control::Savepoint(n.into()));
        assert_eq!(control_of("SAVEPOINT s1"), sp("s1"));
        assert_eq!(control_of("savepoint S1;"), sp("s1"));
        assert_eq!(control_of(r#"SAVEPOINT "Mixed""#), sp("Mixed"));
        for sql in ["RELEASE SAVEPOINT s1", "release s1", "RELEASE S1;"] {
            assert_eq!(
                control_of(sql),
                Some(Control::Release("s1".into())),
                "{sql}"
            );
        }
        for sql in [
            "ROLLBACK TO SAVEPOINT s1",
            "rollback to s1",
            "ROLLBACK WORK TO SAVEPOINT s1",
            "/* undo */ ROLLBACK TRANSACTION TO s1;",
        ] {
            assert_eq!(
                control_of(sql),
                Some(Control::RollbackTo("s1".into())),
                "{sql}"
            );
        }
        assert_eq!(
            control_of(r#"ROLLBACK TO "S""#),
            Some(Control::RollbackTo("S".into()))
        );
    }

    /// The server classifies none of these as transaction control: a near
    /// miss runs as a statement, and the server fails it.
    #[test]
    fn statements_that_only_look_like_control_are_statements() {
        for sql in [
            "SELECT 1",
            "BEGINNING",
            "COMMIT AND CHAIN",
            "COMMIT OFFSETS ON s CONSUMER GROUP g",
            "COMMIT PREPARED 'x'",
            ";COMMIT",
            "; ROLLBACK",
            "ABORT TO s1",
            "ROLLBACK TO",
            "SAVEPOINT",
            "SAVEPOINT a b",
            "BEGIN; SELECT 1",
            "ROLLBACK /* open",
            "",
        ] {
            assert_eq!(control_of(sql), None, "{sql}");
        }
    }

    #[test]
    fn read_only_is_found_among_the_modes() {
        assert!(asks_read_only("BEGIN READ ONLY"));
        assert!(asks_read_only(
            "start transaction isolation level serializable, read only;"
        ));
        assert!(!asks_read_only("BEGIN READ WRITE"));
        assert!(!asks_read_only("BEGIN"));
    }

    #[test]
    fn the_first_word_skips_comments_space_and_semicolons() {
        assert_eq!(first_word("  select 1").as_deref(), Some("SELECT"));
        assert_eq!(
            first_word("/* a /* b */ */ -- c\n ;discard all").as_deref(),
            Some("DISCARD")
        );
        assert_eq!(first_word("(SELECT 1)"), None);
        assert_eq!(first_word("/* open"), None);
        assert_eq!(first_word(" ; "), None);
    }

    #[test]
    fn discard_in_any_spelling_resets_the_session() {
        for sql in [
            "DISCARD ALL",
            "discard all;",
            "/* c */ Discard All",
            ";DISCARD ALL",
        ] {
            assert!(resets_session(sql), "{sql}");
        }
        assert!(!resets_session("SELECT 'DISCARD ALL'"));
    }

    #[test]
    fn only_plain_reads_are_reads() {
        for sql in [
            "SELECT id FROM t",
            "select * from t where n > 1;",
            "-- note\nSHOW COLLECTIONS",
            "DESCRIBE t",
            "EXPLAIN SELECT 1",
            "WITH x AS (SELECT 1) SELECT * FROM x",
            "SELECT inserted_at FROM t",
        ] {
            assert!(is_plain_read(sql), "{sql}");
        }
        for sql in [
            "INSERT INTO t (id) VALUES ('a') RETURNING id",
            "WITH x AS (DELETE FROM t RETURNING id) SELECT * FROM x",
            "UPDATE t SET n = 1",
            "SELECT 1; DELETE FROM t",
            "SELECT '--'; DELETE FROM t",
            "SELECT nextval('s')",
            "SELECT * INTO u FROM t",
            "EXPLAIN ANALYZE SELECT 1",
            "/* /* */ DELETE FROM t */ SELECT 1",
            "SELECT * FROM t WHERE note = 'delete me'",
            "(SELECT 1)",
            "CALL p()",
            "",
        ] {
            assert!(!is_plain_read(sql), "{sql}");
        }
    }
}
