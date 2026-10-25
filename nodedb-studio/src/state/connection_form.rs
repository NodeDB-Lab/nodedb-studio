//! The New connection / Edit connection dialog's fields and their checks.
//!
//! The form lives in one shared signal, filled by whoever opens the dialog
//! (blank for New, `from_saved` for Edit). The secret field holds a password
//! or token only while the dialog is open: the dialog resets the form when it
//! closes, and no type here derives `Debug` over the secret.

use crate::models::connections::{
    AuthMethod, ConnectionDraft, ConnectionKind, ConnectionTarget, ConnectionTest,
    DEFAULT_ORIGIN_PORT, TlsConfig,
};
use crate::state::connections_registry::{Credentials, SavedConnection};

/// What the form knows about its last Test connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestState {
    Idle,
    Running,
    Passed(ConnectionTest),
    /// The server did not answer, or refused. Holds the message to show.
    Failed(String),
}

/// A form field a check can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Host,
    Port,
    LitePath,
    Username,
    Secret,
}

/// One check that failed, with the line shown under the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldError {
    pub field: Field,
    pub message: &'static str,
}

/// What a check is for. Test needs a reachable target but no name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Test,
    Save,
}

#[derive(Clone, PartialEq)]
pub struct ConnectionForm {
    /// The saved name an edit overwrites. `None` for a new connection.
    pub editing: Option<String>,
    pub name: String,
    pub kind: ConnectionKind,
    pub host: String,
    /// Kept as typed: a half-typed port is not an error until a check runs.
    pub port: String,
    pub lite_path: String,
    pub database: String,
    pub auth: AuthMethod,
    pub username: String,
    /// The password or token, by `auth`. Empty on an edit keeps the stored one.
    pub secret: String,
    pub keychain: bool,
    pub tls_required: bool,
    pub server_name: String,
    pub ca_file: String,
    pub client_cert: String,
    pub client_key: String,
    /// Whether the optional client-certificate fields show.
    pub client_cert_open: bool,
    pub test: TestState,
    /// A save is in flight. It lives here, not in a dialog signal, because
    /// the save task outlives the dialog and a closed dialog's signals are gone.
    pub busy: bool,
    /// Set by a Test or Save attempt. Errors show only after one: a form
    /// that scolds before the user typed anything is noise.
    pub attempted: bool,
}

impl std::fmt::Debug for ConnectionForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionForm")
            .field("editing", &self.editing)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("secret", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl ConnectionForm {
    /// An empty New connection form.
    pub fn blank() -> Self {
        Self {
            editing: None,
            name: String::new(),
            kind: ConnectionKind::Origin,
            host: String::new(),
            port: DEFAULT_ORIGIN_PORT.to_string(),
            lite_path: String::new(),
            database: String::new(),
            auth: AuthMethod::UserPassword,
            username: String::new(),
            secret: String::new(),
            keychain: true,
            tls_required: true,
            server_name: String::new(),
            ca_file: String::new(),
            client_cert: String::new(),
            client_key: String::new(),
            client_cert_open: false,
            test: TestState::Idle,
            busy: false,
            attempted: false,
        }
    }

    /// The Edit form of a saved connection. The secret stays in the keychain.
    pub fn from_saved(saved: &SavedConnection) -> Self {
        let mut form = Self {
            editing: Some(saved.name.clone()),
            name: saved.name.clone(),
            database: saved.database.clone(),
            auth: saved.auth,
            username: saved.username.clone(),
            keychain: saved.keychain,
            tls_required: saved.tls.required,
            server_name: saved.tls.server_name.clone(),
            ca_file: saved.tls.ca_file.clone(),
            client_cert: saved.tls.client_cert.clone(),
            client_key: saved.tls.client_key.clone(),
            client_cert_open: !saved.tls.client_cert.is_empty(),
            ..Self::blank()
        };
        match &saved.target {
            ConnectionTarget::Origin { host, port } => {
                form.kind = ConnectionKind::Origin;
                form.host = host.clone();
                form.port = port.to_string();
            }
            ConnectionTarget::Lite { path } => {
                form.kind = ConnectionKind::Lite;
                form.lite_path = path.clone();
            }
        }
        form
    }

    pub fn title(&self) -> &'static str {
        if self.editing.is_some() {
            "Edit connection"
        } else {
            "New connection"
        }
    }

    fn parsed_port(&self) -> Option<u16> {
        self.port.trim().parse::<u16>().ok().filter(|p| *p > 0)
    }

    /// Every check that fails for `purpose`. `taken` lists the other saved
    /// names, which a new name must avoid.
    pub fn errors(&self, purpose: Purpose, taken: &[String]) -> Vec<FieldError> {
        let mut out = Vec::new();
        let mut fail = |field, message| out.push(FieldError { field, message });
        let name = self.name.trim();
        if purpose == Purpose::Save {
            if name.is_empty() {
                fail(Field::Name, "Enter a name.");
            } else if taken.iter().any(|t| t.trim() == name) {
                fail(Field::Name, "A connection with this name exists.");
            }
        }
        match self.kind {
            ConnectionKind::Lite => {
                if self.lite_path.trim().is_empty() {
                    fail(Field::LitePath, "Enter the path of the Lite file.");
                }
            }
            ConnectionKind::Origin => {
                let host = self.host.trim();
                if host.is_empty() {
                    fail(Field::Host, "Enter a host.");
                } else if host.contains(char::is_whitespace) {
                    fail(Field::Host, "A host has no spaces.");
                }
                if self.parsed_port().is_none() {
                    fail(Field::Port, "Enter a port from 1 to 65535.");
                }
                match self.auth {
                    AuthMethod::UserPassword => {
                        if self.username.trim().is_empty() {
                            fail(Field::Username, "Enter a user name.");
                        }
                    }
                    AuthMethod::Token => {
                        if self.secret.is_empty() && self.editing.is_none() {
                            fail(Field::Secret, "Enter a token.");
                        }
                    }
                    AuthMethod::SystemCredentials => {}
                }
            }
        }
        out
    }

    /// The checks to show: none until an attempt has run.
    pub fn visible_errors(&self, purpose: Purpose, taken: &[String]) -> Vec<FieldError> {
        if self.attempted {
            self.errors(purpose, taken)
        } else {
            Vec::new()
        }
    }

    /// The draft to test or save, or the checks that fail. A Test draft
    /// keeps whatever name is typed.
    pub fn draft(
        &self,
        purpose: Purpose,
        taken: &[String],
    ) -> Result<ConnectionDraft, Vec<FieldError>> {
        let errors = self.errors(purpose, taken);
        if !errors.is_empty() {
            return Err(errors);
        }
        let target = match self.kind {
            ConnectionKind::Origin => ConnectionTarget::Origin {
                host: self.host.trim().to_string(),
                port: self.parsed_port().unwrap_or(DEFAULT_ORIGIN_PORT),
            },
            ConnectionKind::Lite => ConnectionTarget::Lite {
                path: self.lite_path.trim().to_string(),
            },
        };
        let tls = match self.kind {
            ConnectionKind::Origin => TlsConfig {
                required: self.tls_required,
                server_name: self.server_name.trim().to_string(),
                ca_file: self.ca_file.trim().to_string(),
                client_cert: self.client_cert.trim().to_string(),
                client_key: self.client_key.trim().to_string(),
            },
            ConnectionKind::Lite => TlsConfig::default(),
        };
        let auth = match self.kind {
            ConnectionKind::Origin => self.auth,
            ConnectionKind::Lite => AuthMethod::SystemCredentials,
        };
        Ok(ConnectionDraft {
            name: self.name.trim().to_string(),
            target,
            database: self.database.trim().to_string(),
            auth,
            username: match auth {
                AuthMethod::UserPassword => self.username.trim().to_string(),
                AuthMethod::Token | AuthMethod::SystemCredentials => String::new(),
            },
            keychain: match auth {
                AuthMethod::UserPassword | AuthMethod::Token => self.keychain,
                AuthMethod::SystemCredentials => false,
            },
            tls,
        })
    }

    /// The identity to send. The password or token is `None` when blank, an
    /// absent secret and not one of length zero.
    pub fn credentials(&self) -> Credentials {
        Credentials {
            username: self.username.trim().to_string(),
            password: (!self.secret.is_empty()).then(|| self.secret.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock;

    fn filled() -> ConnectionForm {
        ConnectionForm {
            name: "Staging".into(),
            host: "db.staging.example".into(),
            username: "admin".into(),
            secret: "pw".into(),
            ..ConnectionForm::blank()
        }
    }

    fn fields(errors: Vec<FieldError>) -> Vec<Field> {
        errors.into_iter().map(|e| e.field).collect()
    }

    #[test]
    fn a_blank_form_names_every_missing_field_for_save() {
        let f = ConnectionForm::blank();
        assert_eq!(
            fields(f.errors(Purpose::Save, &[])),
            [Field::Name, Field::Host, Field::Username]
        );
    }

    /// Test needs a place to reach and an identity, not a name.
    #[test]
    fn test_does_not_need_a_name() {
        let f = ConnectionForm {
            name: String::new(),
            ..filled()
        };
        assert!(f.errors(Purpose::Test, &[]).is_empty());
        assert_eq!(fields(f.errors(Purpose::Save, &[])), [Field::Name]);
    }

    #[test]
    fn whitespace_is_not_a_name_or_a_user() {
        let f = ConnectionForm {
            name: "  ".into(),
            username: "\t".into(),
            ..filled()
        };
        assert_eq!(
            fields(f.errors(Purpose::Save, &[])),
            [Field::Name, Field::Username]
        );
    }

    #[test]
    fn a_taken_name_is_rejected_after_trimming() {
        let f = ConnectionForm {
            name: " staging-cluster ".into(),
            ..filled()
        };
        let taken = vec!["staging-cluster".to_string()];
        assert_eq!(fields(f.errors(Purpose::Save, &taken)), [Field::Name]);
    }

    #[test]
    fn port_must_be_a_number_from_1_to_65535() {
        for bad in ["", "0", "70000", "74x2", "-1"] {
            let f = ConnectionForm {
                port: bad.into(),
                ..filled()
            };
            assert_eq!(
                fields(f.errors(Purpose::Test, &[])),
                [Field::Port],
                "{bad:?}"
            );
        }
        let f = ConnectionForm {
            port: " 65535 ".into(),
            ..filled()
        };
        assert!(f.errors(Purpose::Test, &[]).is_empty());
    }

    #[test]
    fn a_host_with_a_space_is_rejected() {
        let f = ConnectionForm {
            host: "db staging".into(),
            ..filled()
        };
        assert_eq!(fields(f.errors(Purpose::Test, &[])), [Field::Host]);
    }

    #[test]
    fn token_auth_needs_a_token_unless_editing() {
        let mut f = ConnectionForm {
            auth: AuthMethod::Token,
            secret: String::new(),
            username: String::new(),
            ..filled()
        };
        assert_eq!(fields(f.errors(Purpose::Test, &[])), [Field::Secret]);
        f.editing = Some("Staging".into());
        assert!(f.errors(Purpose::Test, &[]).is_empty());
    }

    #[test]
    fn lite_needs_a_path_and_nothing_else() {
        let f = ConnectionForm {
            kind: ConnectionKind::Lite,
            host: String::new(),
            username: String::new(),
            ..filled()
        };
        assert_eq!(fields(f.errors(Purpose::Save, &[])), [Field::LitePath]);
        let ok = ConnectionForm {
            lite_path: "/home/hana/dev.nodedb".into(),
            ..f
        };
        let draft = ok.draft(Purpose::Save, &[]).expect("valid");
        assert_eq!(draft.auth, AuthMethod::SystemCredentials);
        assert!(!draft.keychain);
        assert!(!draft.tls.required);
    }

    #[test]
    fn errors_show_only_after_an_attempt() {
        let mut f = ConnectionForm::blank();
        assert!(f.visible_errors(Purpose::Save, &[]).is_empty());
        f.attempted = true;
        let shown = f.visible_errors(Purpose::Save, &[]);
        assert!(shown.contains(&FieldError {
            field: Field::Host,
            message: "Enter a host."
        }));
        assert!(shown.iter().all(|e| e.field != Field::Port));
    }

    #[test]
    fn the_draft_is_trimmed_and_holds_no_secret() {
        let f = ConnectionForm {
            name: " Staging ".into(),
            host: " db.staging.example ".into(),
            database: " analytics ".into(),
            ..filled()
        };
        let d = f.draft(Purpose::Save, &[]).expect("valid");
        assert_eq!(d.name, "Staging");
        assert_eq!(d.database, "analytics");
        assert_eq!(d.target.summary(), "db.staging.example:7432");
        let printed = format!("{d:?} {f:?}");
        assert!(!printed.contains("pw"), "secret leaked: {printed}");
    }

    #[test]
    fn credentials_carry_an_absent_secret_as_none() {
        let f = ConnectionForm {
            secret: String::new(),
            ..filled()
        };
        assert_eq!(f.credentials().password, None);
        assert_eq!(filled().credentials().password.as_deref(), Some("pw"));
    }

    #[test]
    fn an_edit_form_round_trips_a_saved_connection() {
        for saved in mock::connections() {
            let form = ConnectionForm::from_saved(&saved);
            assert_eq!(form.editing.as_deref(), Some(saved.name.as_str()));
            assert!(form.secret.is_empty(), "the secret is not read back");
            let draft = form
                .draft(Purpose::Save, &[])
                .unwrap_or_else(|e| panic!("{} does not validate: {e:?}", saved.name));
            assert_eq!(draft.name, saved.name);
            assert_eq!(draft.target, saved.target);
            assert_eq!(draft.database, saved.database);
        }
    }

    #[test]
    fn the_title_follows_the_mode() {
        assert_eq!(ConnectionForm::blank().title(), "New connection");
        let edit = ConnectionForm {
            editing: Some("x".into()),
            ..ConnectionForm::blank()
        };
        assert_eq!(edit.title(), "Edit connection");
    }
}
