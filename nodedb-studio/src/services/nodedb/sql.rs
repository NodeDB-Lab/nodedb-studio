//! Text that goes into a statement where a bound `$n` parameter cannot.
//!
//! A value always travels as a `$n` parameter when the statement takes one:
//! `execute_sql(sql, &params)`. These functions are for the parts NodeDB
//! 0.6.2 does not bind: identifiers (collection, field, role and user names)
//! and the literals of statements that take no parameters at all, such as
//! `CREATE USER ... WITH PASSWORD '...'`. No function here touches the
//! network.

use crate::services::error::StudioError;

/// `name` as a bare identifier: one or more ASCII letters, digits or
/// underscores. `what` names what the name is for (`collection`, `role`) in
/// the error.
///
/// Use it for a name written unquoted into DDL or a `SHOW`, `DESCRIBE` or
/// `FROM` clause. A name in a slot that takes `$n` (`MATCH ... IN $1`) is
/// bound instead. The server lowercases a bare identifier and refuses a
/// reserved word or a leading digit (`nodedb-sql`, `reserved.rs`): the check
/// keeps the text safe, and the server's own answer covers the rest. The
/// server sets no length limit, so neither does this.
pub(in crate::services::nodedb) fn ident<'a>(
    name: &'a str,
    what: &str,
) -> Result<&'a str, StudioError> {
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(name)
    } else {
        Err(StudioError::Refused {
            reason: format!(
                "Studio cannot put the {what} name `{name}` in a statement. It writes only names made of letters, digits and underscores."
            ),
        })
    }
}

/// `name` inside double quotes, for a field name. `what` names what the name
/// is for (`field`) in the error.
///
/// Checked live on 0.6.2: a quoted name with a dash, a space, a `'`, `$1`,
/// `--` or non-ASCII letters works in INSERT, UPDATE SET and WHERE. The
/// server refuses an empty quoted name, a `"` or a control character in one
/// (`reserved.rs`), so this refuses them before they reach the text and a
/// name never leaves its quotes. Use it for any name a value can carry, such
/// as a document's field. A value is never quoted here: bind it as `$n`.
pub(in crate::services::nodedb) fn quoted(name: &str, what: &str) -> Result<String, StudioError> {
    let why = if name.is_empty() {
        format!("Studio cannot put an empty {what} name in a statement. Give the {what} a name.")
    } else if name.contains('"') {
        format!(
            "The {what} name `{name}` holds a double quote, and NodeDB 0.6.2 refuses such a name in a statement. Studio cannot write or filter by it."
        )
    } else if name.chars().any(char::is_control) {
        format!(
            "The {what} name {name:?} holds a control character, which Studio does not put in a statement."
        )
    } else {
        return Ok(format!("\"{name}\""));
    };
    Err(StudioError::Refused { reason: why })
}

/// `text` as a SQL string literal, for a statement that takes no `$n` at
/// all (`CREATE USER ... WITH PASSWORD`). Never use it where `$n` binds.
///
/// The server's lexer (`nodedb-sql`, `ddl_ast/parse/clause/token.rs`) reads
/// `'...'` with one escape: a doubled `''` is one quote. A backslash is an
/// ordinary character. Only an `E'...'` string decodes backslashes, and this
/// never writes the `E` prefix.
pub(in crate::services::nodedb) fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identifier_takes_letters_digits_and_underscores_only() {
        for ok in ["orders_2", "studio_probe_a_r1", "X", &"x".repeat(64)] {
            assert_eq!(ident(ok, "role").ok(), Some(ok), "{ok}");
        }
        for bad in ["", "a b", "a;b", "a-b", "a'b", "é", "a; DROP", "a\"b"] {
            assert!(
                matches!(ident(bad, "role"), Err(StudioError::Refused { .. })),
                "{bad}"
            );
        }
    }

    #[test]
    fn an_identifier_error_names_what_the_name_was_for() {
        let e = ident("a-b", "collection").expect_err("a dash");
        assert!(e.to_string().contains("collection name `a-b`"), "{e}");
    }

    #[test]
    fn a_quoted_name_stays_inside_its_quotes() {
        for ok in [
            "anything_9",
            "e-mail",
            "a b",
            "a;b",
            "o'k",
            "名前",
            "a$1",
            "x--y",
        ] {
            assert_eq!(quoted(ok, "field").ok(), Some(format!("\"{ok}\"")), "{ok}");
        }
        for bad in ["", "a\"b", "a\0b", "a\nb", "a\u{7f}", "x\" OR 1=1 --"] {
            assert!(
                matches!(quoted(bad, "field"), Err(StudioError::Refused { .. })),
                "{bad:?}"
            );
        }
        let e = quoted("i\"d", "field").expect_err("a quote");
        assert!(e.to_string().contains("i\"d"), "{e}");
    }

    #[test]
    fn a_literal_doubles_the_quote_and_leaves_the_rest() {
        assert_eq!(literal("abc"), "'abc'");
        assert_eq!(literal("a'b"), "'a''b'");
        assert_eq!(literal("''"), "''''''");
        assert_eq!(literal(r"a\b"), r"'a\b'", "a backslash is plain text");
        assert_eq!(literal("x\"y;z--w\n é"), "'x\"y;z--w\n é'");
        assert_eq!(literal(""), "''");
    }
}
