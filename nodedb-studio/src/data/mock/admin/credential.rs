//! Credential aliases: the providers the server calls out to. The store
//! holds a reference to each secret and never its value.

use crate::data::mock::admin::store::{AdminStore, DATABASE, missing};
use crate::models::admin::{AdminReview, ReviewLine};
use crate::models::admin_audit::TargetType;
use crate::models::admin_credential::{
    Credential, CredentialChange, CredentialKind, CredentialList, CredentialStatus, CredentialTest,
    IndexDimension,
};
use crate::models::format::epoch;
use crate::services::error::StudioError;

const CREDENTIAL_WRITE: &str = "credential.write";
/// The vector size `support-embed-v2` returns.
const PROVIDER_DIMENSION: u32 = 768;

pub(super) fn seeded_credentials() -> Vec<Credential> {
    vec![
        Credential {
            alias: "search_embeddings".into(),
            kind: CredentialKind::Https,
            endpoint: "https://embed.internal".into(),
            models: vec!["support-embed-v2".into()],
            dimension: Some(PROVIDER_DIMENSION),
            scope: DATABASE.into(),
            status: CredentialStatus::Tested,
            last_tested: Some(epoch(2026, 6, 13, 9, 12, 40)),
            secret_ref: "secret/embeddings".into(),
            secret_version: 2,
            enabled: true,
        },
        Credential {
            alias: "backup_store".into(),
            kind: CredentialKind::ObjectStorage,
            endpoint: "s3://nodedb-backups".into(),
            models: Vec::new(),
            dimension: None,
            scope: DATABASE.into(),
            status: CredentialStatus::Unknown,
            last_tested: None,
            secret_ref: "secret/backup_store".into(),
            secret_version: 1,
            enabled: true,
        },
    ]
}

impl AdminStore {
    pub fn credential_list(&self) -> CredentialList {
        CredentialList {
            items: self.credentials.clone(),
            indexes: vec![IndexDimension {
                index: "idx_articles_embedding".into(),
                collection: "articles".into(),
                dimension: 768,
            }],
            observed_at: self.clock,
        }
    }

    /// One explicit request to the provider. The status records the result.
    pub fn test_credential(&mut self, alias: &str) -> Result<CredentialTest, StudioError> {
        let at = self.clock;
        let c = self
            .credentials
            .iter_mut()
            .find(|c| c.alias == alias)
            .ok_or_else(|| missing("credential", alias))?;
        c.status = CredentialStatus::Tested;
        c.last_tested = Some(at);
        let embeds = c.kind == CredentialKind::Https;
        Ok(CredentialTest {
            alias: alias.to_string(),
            reachable: true,
            latency_ms: Some(if embeds { 38 } else { 64 }),
            dimension: embeds.then_some(PROVIDER_DIMENSION),
            compared_with: embeds.then(|| "idx_articles_embedding (768)".to_string()),
            message: if embeds {
                "The provider answered one probe request. Nothing was stored.".to_string()
            } else {
                "The destination listed its prefix. Nothing was written.".to_string()
            },
            at,
        })
    }

    pub(super) fn preview_credential(
        &self,
        change: &CredentialChange,
    ) -> Result<AdminReview, StudioError> {
        let alias = change.alias();
        let found = self.credentials.iter().find(|c| c.alias == alias);
        if let CredentialChange::Create(d) = change {
            return Ok(self.preview_new_credential(d, found.is_some()));
        }
        let c = found.ok_or_else(|| missing("credential", alias))?;
        let mut review = AdminReview {
            title: String::new(),
            target: alias.to_string(),
            lines: Vec::new(),
            notes: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            permission: self.permission(CREDENTIAL_WRITE),
            revision: self.rev.credentials,
            apply_label: String::new(),
        };
        match change {
            CredentialChange::Create(_) => {}
            CredentialChange::Rotate { .. } => {
                review.title = "Review rotation".to_string();
                review.apply_label = "Rotate credential".to_string();
                review.lines.push(ReviewLine::new(
                    format!("Secret {}", c.secret_ref),
                    Some(&format!("Version {}", c.secret_version)),
                    format!("Version {}", c.secret_version + 1),
                ));
                review.notes = vec![
                    "The secret store creates the new version. Studio never sees either value."
                        .to_string(),
                    "The credential reads Unknown until you test it again.".to_string(),
                ];
                review.warnings = vec![
                    "The server switches to the new version at once. Requests that use the old version fail."
                        .to_string(),
                ];
            }
            CredentialChange::Update(d) => {
                review.title = "Review credential changes".to_string();
                review.apply_label = "Apply changes".to_string();
                review.errors = [
                    d.secret_issue(),
                    d.endpoint_issue(),
                    d.models_issue(),
                    d.dimension_issue(),
                ]
                .into_iter()
                .flatten()
                .collect();
                let mut line = |label: &str, before: String, after: String| {
                    if before != after {
                        review
                            .lines
                            .push(ReviewLine::new(label, Some(&before), after));
                    }
                };
                line(
                    "Provider",
                    c.kind.label().to_string(),
                    d.kind.label().to_string(),
                );
                line(
                    "Secret reference",
                    c.secret_ref.clone(),
                    d.secret_ref.trim().to_string(),
                );
                line(
                    "Endpoint",
                    c.endpoint.clone(),
                    d.endpoint.trim().to_string(),
                );
                line(
                    "Allowed models",
                    c.models.join(", "),
                    d.model_list().join(", "),
                );
                line(
                    "Dimension",
                    c.dimension.map(|n| n.to_string()).unwrap_or_default(),
                    d.dimension_value()
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                );
                if review.lines.is_empty() && review.errors.is_empty() {
                    review
                        .errors
                        .push("The credential has no change to apply.".to_string());
                }
                if let (Some(dim), Some(idx)) = (
                    d.dimension_value(),
                    self.credential_list().indexes.first().cloned(),
                ) && dim != idx.dimension
                {
                    review.warnings.push(format!(
                        "Vectors of {dim} dimensions do not fit {} ({}).",
                        idx.index, idx.dimension
                    ));
                }
                if d.endpoint.trim() != c.endpoint {
                    review.notes.push(
                        "The next request goes to the new endpoint. Test it afterwards."
                            .to_string(),
                    );
                }
                review
                    .notes
                    .push("The credential reads Unknown until you test it again.".to_string());
            }
        }
        review
            .notes
            .push("Credentials never appear in saved queries or exported logs.".to_string());
        Ok(review)
    }

    /// The review of a credential that does not exist yet.
    fn preview_new_credential(
        &self,
        d: &crate::models::admin_credential::CredentialDraft,
        exists: bool,
    ) -> AdminReview {
        let taken: Vec<String> = if exists {
            vec![d.alias.trim().to_string()]
        } else {
            Vec::new()
        };
        let mut errors: Vec<String> = [
            d.alias_issue(&taken),
            d.secret_issue(),
            d.endpoint_issue(),
            d.models_issue(),
            d.dimension_issue(),
        ]
        .into_iter()
        .flatten()
        .collect();
        errors.dedup();
        let mut lines = vec![
            ReviewLine::new("Alias", None, d.alias.trim()),
            ReviewLine::new("Provider", None, d.kind.label()),
            ReviewLine::new("Secret reference", None, d.secret_ref.trim()),
            ReviewLine::new("Endpoint", None, d.endpoint.trim()),
        ];
        if d.kind == CredentialKind::Https {
            lines.push(ReviewLine::new(
                "Allowed models",
                None,
                d.model_list().join(", "),
            ));
        }
        AdminReview {
            title: "Review new credential".to_string(),
            target: d.alias.trim().to_string(),
            lines,
            notes: vec![
                "The credential reads Unknown until you test it.".to_string(),
                "Credentials never appear in saved queries or exported logs.".to_string(),
            ],
            warnings: Vec::new(),
            errors,
            permission: self.permission(CREDENTIAL_WRITE),
            revision: self.rev.credentials,
            apply_label: "Create credential".to_string(),
        }
    }

    pub(super) fn apply_credential(
        &mut self,
        change: &CredentialChange,
    ) -> Result<String, StudioError> {
        let alias = change.alias().to_string();
        if let CredentialChange::Create(d) = change {
            self.credentials.push(Credential {
                alias: alias.clone(),
                kind: d.kind,
                endpoint: d.endpoint.trim().to_string(),
                models: d.model_list(),
                dimension: d.dimension_value(),
                scope: DATABASE.into(),
                status: CredentialStatus::Unknown,
                last_tested: None,
                secret_ref: d.secret_ref.trim().to_string(),
                secret_version: 1,
                enabled: true,
            });
            self.rev.credentials += 1;
            let audit = self.record(
                "Create credential",
                TargetType::Credential,
                &format!("credential:{alias}"),
                &alias,
                vec!["Credential: none".to_string()],
                vec![format!("Credential {alias}: created")],
            );
            return Ok(format!("Created credential {alias}. Recorded as {audit}."));
        }
        let c = self
            .credentials
            .iter_mut()
            .find(|c| c.alias == alias)
            .ok_or_else(|| missing("credential", &alias))?;
        let (action, before, after) = match change {
            CredentialChange::Create(_) => return Ok(String::new()),
            CredentialChange::Rotate { .. } => {
                let before = format!("Secret version: {}", c.secret_version);
                c.secret_version += 1;
                (
                    "Rotate credential",
                    before,
                    format!("Secret version: {}", c.secret_version),
                )
            }
            CredentialChange::Update(d) => {
                let before = format!("Endpoint: {} · models: {}", c.endpoint, c.models.join(", "));
                c.kind = d.kind;
                c.secret_ref = d.secret_ref.trim().to_string();
                c.endpoint = d.endpoint.trim().to_string();
                c.models = d.model_list();
                c.dimension = d.dimension_value();
                (
                    "Update credential",
                    before,
                    format!("Endpoint: {} · models: {}", c.endpoint, c.models.join(", ")),
                )
            }
        };
        c.status = CredentialStatus::Unknown;
        self.rev.credentials += 1;
        let audit = self.record(
            action,
            TargetType::Credential,
            &format!("credential:{alias}"),
            &alias,
            vec![before],
            vec![after],
        );
        Ok(format!("{action} {alias}. Recorded as {audit}."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_credential_row_carries_a_secret_value() {
        for c in seeded_credentials() {
            assert!(c.secret_ref.starts_with("secret/"), "{}", c.alias);
            assert!(!c.endpoint.contains('@') && !c.endpoint.contains('?'));
        }
    }

    #[test]
    fn a_test_marks_the_credential_tested() {
        let mut s = AdminStore::seeded();
        let t = s.test_credential("backup_store").expect("test");
        assert!(t.reachable);
        let list = s.credential_list();
        let c = list
            .items
            .iter()
            .find(|c| c.alias == "backup_store")
            .expect("c");
        assert_eq!(c.status, CredentialStatus::Tested);
    }

    #[test]
    fn rotation_resets_the_status_to_unknown() {
        let mut s = AdminStore::seeded();
        let change = crate::models::admin::AdminChange::Credential(CredentialChange::Rotate {
            alias: "search_embeddings".into(),
        });
        let review = s.preview(&change).expect("preview");
        s.apply(&change, review.revision).expect("apply");
        let list = s.credential_list();
        let c = &list.items[0];
        assert_eq!((c.secret_version, c.status), (3, CredentialStatus::Unknown));
        assert!(
            s.apply(&change, review.revision).is_err(),
            "the review is stale"
        );
    }
}
