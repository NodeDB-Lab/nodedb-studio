//! Device enrollment. The token is a secret: the store keeps the command
//! that carries it and hands it out once.

use crate::data::mock::sync::devices::origin;
use crate::data::mock::sync::store::{SyncStore, missing};
use crate::models::sync_activity::SyncEventKind;
use crate::models::sync_enroll::{EnrollOptions, EnrollRequest, Enrollment, Preset};
use crate::services::error::StudioError;

/// How long a token lives, in seconds.
const TTL: u64 = 900;

fn refused(reason: impl Into<String>) -> StudioError {
    StudioError::Rejected {
        reason: reason.into(),
    }
}

impl SyncStore {
    fn next_id(&self) -> String {
        format!("d{}", self.next_device)
    }

    pub fn enroll_options(&self) -> EnrollOptions {
        EnrollOptions {
            origin: origin().name,
            endpoint: origin().endpoint,
            device_id: self.next_id(),
            databases: vec!["analytics".to_string()],
            presets: vec![Preset {
                name: "store_orders".to_string(),
                parameter: "store_id".to_string(),
                parameter_label: "Store ID".to_string(),
                values: ["nyc", "sf", "la", "chi"].map(str::to_string).to_vec(),
                shapes: ["orders_for_store", "customers_for_store"]
                    .map(str::to_string)
                    .to_vec(),
            }],
            roles: vec!["sync_checkout".to_string(), "sync_apac".to_string()],
            token_ttl: TTL,
        }
    }

    /// Issue a one-use token. The device does not exist until it redeems it.
    pub fn enroll(&mut self, req: &EnrollRequest) -> Result<Enrollment, StudioError> {
        let options = self.enroll_options();
        if req.name.trim().is_empty() {
            return Err(refused("Enter a device name."));
        }
        if self
            .devices
            .iter()
            .any(|d| d.name.eq_ignore_ascii_case(&req.name))
        {
            return Err(refused(format!(
                "A device named {} is already registered.",
                req.name
            )));
        }
        if !options.databases.contains(&req.database) {
            return Err(refused(format!(
                "The principal may not enroll devices for {}.",
                req.database
            )));
        }
        let preset = options
            .presets
            .iter()
            .find(|p| p.name == req.preset)
            .ok_or_else(|| missing("subscription preset", &req.preset))?;
        if !preset.values.contains(&req.value) {
            return Err(refused(format!(
                "{} has no value {}.",
                preset.parameter, req.value
            )));
        }
        if !options.roles.contains(&req.role) {
            return Err(missing("role", &req.role));
        }
        let id = self.next_id();
        self.next_device += 1;
        let secret = format!(
            "enr_{id}_{:08x}",
            0x7c1d_4f9au32.wrapping_mul(self.next_device)
        );
        let masked = format!("enr_{}{}", "•".repeat(8), &secret[secret.len() - 4..]);
        self.commands.push((
            id.clone(),
            format!(
                "ndb-lite enroll --origin {} --device {id} --token {secret}",
                options.endpoint
            ),
        ));
        let mut e = Self::event(
            SyncEventKind::EnrollmentIssued,
            (&id, &req.name),
            Some(&req.preset),
            Some(format!("role {}", req.role)),
        );
        e.database = req.database.clone();
        self.record(e);
        let issued_at = self.clock;
        Ok(Enrollment {
            device_id: id,
            name: req.name.clone(),
            masked,
            issued_at,
            expires_at: issued_at + TTL as i64,
        })
    }

    /// The command that carries the token. It is handed out once.
    pub fn take_command(&mut self, device_id: &str) -> Result<String, StudioError> {
        let at = self
            .commands
            .iter()
            .position(|(id, _)| id == device_id)
            .ok_or_else(|| missing("enrollment command", device_id))?;
        Ok(self.commands.remove(at).1)
    }
}
