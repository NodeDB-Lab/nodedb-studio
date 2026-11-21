//! A password the person types for a new user, carried from the form to the
//! one statement that sets it.
//!
//! NodeDB 0.6.2 enforces one rule on a new user's password: it is not empty
//! (`validate_password_assignment`). `min_password_length` in the server
//! config is read nowhere. Studio adds the characters the statement cannot
//! carry (see `invalid_char`).

use std::fmt;

/// A secret string. `Debug` redacts it unconditionally, there is no `Display`
/// and no `Serialize`, so a log line, an error string, or a persisted form
/// cannot carry it by accident. Equality compares the text, and an assertion
/// that fails prints `<redacted>`.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The text. Call it only where the secret leaves Studio.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// The first character a password cannot carry, if any.
pub fn invalid_char(password: &str) -> Option<char> {
    password.chars().find(|c| *c == '\0')
}

/// Why `password` cannot be used, or `None`. A password is exact: it is
/// never trimmed.
pub fn password_issue(password: &Secret) -> Option<String> {
    if password.is_empty() {
        return Some("Enter a password.".to_string());
    }
    invalid_char(password.expose())
        .map(|_| "A password cannot contain a NUL character.".to_string())
}

/// Why the confirmation does not match, or `None`.
pub fn confirm_issue(password: &Secret, confirm: &Secret) -> Option<String> {
    if confirm.is_empty() {
        Some("Enter the password again.".to_string())
    } else if password != confirm {
        Some("The two passwords differ.".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_text() {
        let s = Secret::new("hunter2-xyz");
        assert!(!format!("{s:?}").contains("hunter2"));
        assert!(!format!("{:#?}", Some(&s)).contains("hunter2"));
    }

    #[test]
    fn the_only_server_rule_is_not_empty() {
        assert!(password_issue(&Secret::new("")).is_some());
        assert!(password_issue(&Secret::new("a")).is_none());
        assert!(password_issue(&Secret::new(" ")).is_none(), "not trimmed");
        assert!(password_issue(&Secret::new("a\0b")).is_some());
    }

    #[test]
    fn the_confirmation_must_match() {
        let a = Secret::new("abc");
        assert!(confirm_issue(&a, &Secret::new("")).is_some());
        assert!(confirm_issue(&a, &Secret::new("abd")).is_some());
        assert!(confirm_issue(&a, &Secret::new("abc")).is_none());
    }
}
