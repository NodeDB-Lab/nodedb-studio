//! Credentials: named aliases for the providers the server calls out to.
//!
//! The server holds the secret. A credential row carries a reference to it
//! (`secret/embeddings`), never the value, and Studio cannot read the value
//! back. A credential never appears in a saved query or an exported log. A
//! remote provider is called only after an administrator enables it for
//! explicit use, and nothing calls it on its own.

use crate::models::admin::identifier_issue;
use crate::models::feeds::Tone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    Https,
    ObjectStorage,
}

impl CredentialKind {
    pub const ALL: [CredentialKind; 2] = [CredentialKind::Https, CredentialKind::ObjectStorage];

    pub fn label(self) -> &'static str {
        match self {
            CredentialKind::Https => "HTTPS",
            CredentialKind::ObjectStorage => "Object Storage",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStatus {
    Tested,
    /// Nobody tested it, so nothing is known about it.
    Unknown,
    // The server reports it. No fixture holds a failed credential.
    #[allow(dead_code)]
    Failed,
}

impl CredentialStatus {
    pub fn label(self) -> &'static str {
        match self {
            CredentialStatus::Tested => "Tested",
            CredentialStatus::Unknown => "Unknown",
            CredentialStatus::Failed => "Failed",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            CredentialStatus::Tested => Tone::Ok,
            CredentialStatus::Unknown => Tone::Idle,
            CredentialStatus::Failed => Tone::Err,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub alias: String,
    pub kind: CredentialKind,
    /// A host or URL. It holds no token.
    pub endpoint: String,
    /// Model identifiers the provider may serve. Empty for storage.
    pub models: Vec<String>,
    /// The vector size the provider's model returns. Embedding providers only.
    pub dimension: Option<u32>,
    /// The database the credential is scoped to.
    pub scope: String,
    pub status: CredentialStatus,
    pub last_tested: Option<i64>,
    /// Where the server keeps the secret. The value is never here.
    pub secret_ref: String,
    pub secret_version: u32,
    /// An administrator enabled the provider for explicit requests.
    pub enabled: bool,
}

impl Credential {
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [&self.alias, &self.endpoint, &self.scope]
                .iter()
                .any(|f| f.to_lowercase().contains(needle))
            || self.kind.label().to_lowercase().contains(needle)
    }

    pub fn draft(&self) -> CredentialDraft {
        CredentialDraft {
            alias: self.alias.clone(),
            kind: self.kind,
            secret_ref: self.secret_ref.clone(),
            endpoint: self.endpoint.clone(),
            models: self.models.join(", "),
            dimension: self.dimension.map(|d| d.to_string()).unwrap_or_default(),
            tried: false,
        }
    }
}

/// An index a provider's vectors must fit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDimension {
    pub index: String,
    pub collection: String,
    pub dimension: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialList {
    pub items: Vec<Credential>,
    /// The vector indexes of the scope, for the dimension check.
    pub indexes: Vec<IndexDimension>,
    pub observed_at: i64,
}

/// An edit of one credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialDraft {
    pub alias: String,
    pub kind: CredentialKind,
    pub secret_ref: String,
    pub endpoint: String,
    /// Comma-separated model identifiers.
    pub models: String,
    pub dimension: String,
    pub tried: bool,
}

impl CredentialDraft {
    /// A draft for a credential that does not exist yet.
    pub fn blank() -> Self {
        Self {
            alias: String::new(),
            kind: CredentialKind::Https,
            secret_ref: "secret/".to_string(),
            endpoint: String::new(),
            models: String::new(),
            dimension: String::new(),
            tried: false,
        }
    }

    pub fn alias_issue(&self, taken: &[String]) -> Option<String> {
        identifier_issue(&self.alias, "credential alias").or_else(|| {
            taken
                .iter()
                .any(|a| *a == self.alias.trim())
                .then(|| format!("`{}` exists. Choose another alias.", self.alias.trim()))
        })
    }

    pub fn model_list(&self) -> Vec<String> {
        self.models
            .split(',')
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(str::to_string)
            .collect()
    }

    pub fn dimension_value(&self) -> Option<u32> {
        self.dimension.trim().parse().ok().filter(|d| *d > 0)
    }

    pub fn secret_issue(&self) -> Option<String> {
        let r = self.secret_ref.trim();
        if !r.starts_with("secret/")
            || r.len() <= "secret/".len()
            || r.contains(char::is_whitespace)
        {
            Some("Enter a reference such as secret/embeddings.".to_string())
        } else {
            None
        }
    }

    pub fn endpoint_issue(&self) -> Option<String> {
        let e = self.endpoint.trim();
        if e.is_empty() {
            return Some("Enter the provider endpoint.".to_string());
        }
        // A token in the URL would reach logs and saved queries.
        if e.contains('@') || e.contains('?') {
            return Some(
                "Remove credentials and query text from the endpoint. Use the secret reference."
                    .to_string(),
            );
        }
        match self.kind {
            CredentialKind::Https if !e.starts_with("https://") && !e.contains('.') => {
                Some("Enter a host or an https:// URL.".to_string())
            }
            _ => None,
        }
    }

    pub fn models_issue(&self) -> Option<String> {
        (self.kind == CredentialKind::Https && self.model_list().is_empty())
            .then(|| "Allow at least one model.".to_string())
    }

    pub fn dimension_issue(&self) -> Option<String> {
        (self.kind == CredentialKind::Https
            && !self.dimension.trim().is_empty()
            && self.dimension_value().is_none())
        .then(|| "Enter a whole number above zero.".to_string())
    }

    pub fn is_valid(&self) -> bool {
        self.secret_issue().is_none()
            && self.endpoint_issue().is_none()
            && self.models_issue().is_none()
            && self.dimension_issue().is_none()
    }

    pub fn differs_from(&self, c: &Credential) -> bool {
        self.kind != c.kind
            || self.secret_ref.trim() != c.secret_ref
            || self.endpoint.trim() != c.endpoint
            || self.model_list() != c.models
            || self.dimension_value() != c.dimension
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialChange {
    Create(CredentialDraft),
    Update(CredentialDraft),
    Rotate { alias: String },
}

impl CredentialChange {
    pub fn alias(&self) -> &str {
        match self {
            CredentialChange::Create(d) | CredentialChange::Update(d) => &d.alias,
            CredentialChange::Rotate { alias } => alias,
        }
    }
}

/// What a connection test found. It is one explicit request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialTest {
    pub alias: String,
    pub reachable: bool,
    pub latency_ms: Option<u32>,
    /// The vector size the provider returned for a probe, if it served one.
    pub dimension: Option<u32>,
    /// What the dimension was compared with: `idx_articles_embedding (768)`.
    pub compared_with: Option<String>,
    pub message: String,
    pub at: i64,
}

impl CredentialTest {
    /// The dimension matches the index it feeds.
    pub fn compatible(&self, expected: Option<u32>) -> Option<bool> {
        self.dimension.zip(expected).map(|(got, want)| got == want)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> CredentialDraft {
        CredentialDraft {
            alias: "search_embeddings".into(),
            kind: CredentialKind::Https,
            secret_ref: "secret/embeddings".into(),
            endpoint: "https://embed.internal".into(),
            models: "support-embed-v2, embed-large".into(),
            dimension: "768".into(),
            tried: false,
        }
    }

    #[test]
    fn a_new_credential_needs_a_free_alias() {
        let d = CredentialDraft::blank();
        assert!(d.alias_issue(&[]).is_some());
        let mut d = draft();
        d.alias = "search_embeddings".into();
        assert!(d.alias_issue(&["search_embeddings".to_string()]).is_some());
        assert!(d.alias_issue(&[]).is_none());
    }

    #[test]
    fn models_split_on_commas() {
        assert_eq!(draft().model_list(), ["support-embed-v2", "embed-large"]);
    }

    #[test]
    fn a_secret_never_rides_in_the_endpoint() {
        let mut d = draft();
        assert!(d.is_valid());
        d.endpoint = "https://user:pw@embed.internal".into();
        assert!(d.endpoint_issue().is_some());
        d.endpoint = "https://embed.internal/v1?key=abc".into();
        assert!(d.endpoint_issue().is_some());
    }

    #[test]
    fn a_secret_reference_is_a_path_not_a_value() {
        let mut d = draft();
        d.secret_ref = "sk-live-123".into();
        assert!(d.secret_issue().is_some());
        d.secret_ref = "secret/".into();
        assert!(d.secret_issue().is_some());
    }

    #[test]
    fn dimension_compatibility_compares_numbers() {
        let t = CredentialTest {
            alias: "a".into(),
            reachable: true,
            latency_ms: Some(40),
            dimension: Some(768),
            compared_with: None,
            message: String::new(),
            at: 0,
        };
        assert_eq!(t.compatible(Some(768)), Some(true));
        assert_eq!(t.compatible(Some(1024)), Some(false));
        assert_eq!(t.compatible(None), None);
    }
}
