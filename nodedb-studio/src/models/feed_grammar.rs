//! Change feed form grammar: the filter, the projection list, and the webhook
//! endpoint rule.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
