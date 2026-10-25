//! Checks for text a person types as one JSON document.

/// Why `text` is not one JSON document, if it is not.
pub fn json_issue(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return Some("Enter a JSON value.".into());
    }
    sonic_rs::from_str::<sonic_rs::Value>(text)
        .err()
        .map(|e| format!("Not valid JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_documents_pass() {
        assert_eq!(json_issue("{\"event\": \"login\", \"n\": 1}"), None);
        assert_eq!(json_issue("[1, 2]"), None);
        assert_eq!(json_issue("\"text\""), None);
    }

    #[test]
    fn invalid_or_empty_text_is_named() {
        assert_eq!(json_issue("").as_deref(), Some("Enter a JSON value."));
        assert!(json_issue("{ nope").is_some_and(|m| m.starts_with("Not valid JSON")));
        assert!(json_issue("{\"a\": 1} trailing").is_some());
    }
}
