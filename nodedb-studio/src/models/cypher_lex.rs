//! Tokenizer for the Cypher subset in `cypher`.

use std::fmt;

use super::cypher::CypherError;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tok {
    Ident(String),
    Str(String),
    Num(f64),
    Sym(&'static str),
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Ident(s) => write!(f, "`{s}`"),
            Tok::Str(s) => write!(f, "'{s}'"),
            Tok::Num(n) => write!(f, "{n}"),
            Tok::Sym(s) => write!(f, "`{s}`"),
        }
    }
}

pub(super) fn lex(text: &str) -> Result<Vec<Tok>, CypherError> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || (chars[i] == '.' && chars.get(i + 1) != Some(&'.')))
            {
                i += 1;
            }
            let digits: String = chars[start..i].iter().collect();
            let n = digits.parse().map_err(|_| CypherError::Unexpected(c))?;
            out.push(Tok::Num(n));
        } else if c == '"' || c == '\'' {
            let quote = c;
            let mut s = String::new();
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err(CypherError::UnterminatedString),
                    Some('\\') if i + 1 < chars.len() => {
                        s.push(chars[i + 1]);
                        i += 2;
                    }
                    Some(&q) if q == quote => {
                        i += 1;
                        break;
                    }
                    Some(&other) => {
                        s.push(other);
                        i += 1;
                    }
                }
            }
            out.push(Tok::Str(s));
        } else {
            let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let sym = match two.as_str() {
                "->" => Some("->"),
                "<-" => Some("<-"),
                ">=" => Some(">="),
                "<=" => Some("<="),
                "<>" => Some("<>"),
                "!=" => Some("<>"),
                ".." => Some(".."),
                _ => None,
            };
            if let Some(s) = sym {
                out.push(Tok::Sym(s));
                i += 2;
                continue;
            }
            let one = match c {
                '(' => "(",
                ')' => ")",
                '[' => "[",
                ']' => "]",
                '{' => "{",
                '}' => "}",
                ':' => ":",
                ',' => ",",
                '.' => ".",
                '-' => "-",
                '=' => "=",
                '>' => ">",
                '<' => "<",
                '*' => "*",
                other => return Err(CypherError::Unexpected(other)),
            };
            out.push(Tok::Sym(one));
            i += 1;
        }
    }
    Ok(out)
}
