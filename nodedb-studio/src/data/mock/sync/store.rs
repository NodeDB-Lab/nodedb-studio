//! The sync state the mock serves, with the writes applied to it. Devices,
//! subscriptions, rejections, and activity share one store, so pausing a
//! device shows in the device list, the log, and the Subscriptions tab.
//! Every write names the revision it read and is refused when it changed.

use crate::data::mock::cluster::members::OBSERVED;
use crate::data::mock::sync::activity::seeded_events;
use crate::data::mock::sync::devices::{origin, seeded_devices};
use crate::data::mock::sync::rejects::seeded_rejects;
use crate::data::mock::sync::subs::{seeded_subs, store_rows};
use crate::models::cluster_op::{OpResult, PermissionCheck};
use crate::models::sync::{Device, DeviceState, SyncDevices};
use crate::models::sync_activity::{SyncEvent, SyncEventKind, SyncLog, SyncQuery};
use crate::models::sync_reject::{CompensationStep, RejectedWrites, Rejection};
use crate::models::sync_sub::{
    ChangePreview, Scope, Snapshot, SubChange, SubEvent, SubEventKind, SubState, Subscription,
    Subscriptions,
};
use crate::services::error::StudioError;

/// The privilege a subscription change needs.
const SUBSCRIPTION_WRITE: &str = "sync.subscription.write";
/// What one row costs on the wire, in bytes. The server measures it.
const ROW_BYTES: u64 = 1_420;
const ADMIN: &str = "admin";

#[derive(Debug, Clone)]
pub struct SyncStore {
    pub(super) clock: i64,
    pub(super) devices: Vec<Device>,
    pub(super) subs: Vec<Subscription>,
    pub(super) rejects: Vec<Rejection>,
    pub(super) events: Vec<SyncEvent>,
    /// Steps recorded after a rejection, by mutation id.
    pub(super) repairs: Vec<(String, CompensationStep)>,
    /// The mutation id a resubmission created, by the original's id.
    pub(super) replacements: Vec<(String, String)>,
    pub(super) next_event: u32,
    pub(super) next_operation: u32,
    pub(super) next_device: u32,
    /// Enrollment commands not yet handed out, by device id.
    pub(super) commands: Vec<(String, String)>,
    granted: bool,
}

pub(super) fn missing(kind: &'static str, id: &str) -> StudioError {
    StudioError::UnknownObject {
        kind,
        id: id.to_string(),
    }
}

pub(super) fn stale(object: String, expected: u64, stored: u64) -> StudioError {
    StudioError::StaleRevision {
        object,
        expected,
        stored,
    }
}

impl SyncStore {
    pub fn seeded() -> Self {
        Self {
            clock: OBSERVED,
            devices: seeded_devices(),
            subs: seeded_subs(),
            rejects: seeded_rejects(),
            events: seeded_events(),
            repairs: Vec::new(),
            replacements: Vec::new(),
            next_event: 816,
            next_operation: 41,
            next_device: 18,
            commands: Vec::new(),
            granted: true,
        }
    }

    /// A store whose principal lacks the subscription privilege.
    #[cfg(test)]
    pub fn without_permission() -> Self {
        Self {
            granted: false,
            ..Self::seeded()
        }
    }

    pub fn devices(&self) -> SyncDevices {
        SyncDevices {
            origin: origin(),
            devices: self.devices.clone(),
            observed_at: self.clock,
        }
    }

    pub fn subscriptions(&self) -> Subscriptions {
        Subscriptions {
            items: self.subs.clone(),
            observed_at: self.clock,
        }
    }

    pub fn rejected(&self) -> RejectedWrites {
        RejectedWrites {
            rows: self.rejects.clone(),
            observed_at: self.clock,
        }
    }

    pub fn activity(&self, q: &SyncQuery) -> SyncLog {
        let floor = self.clock - q.range.seconds();
        let events = self
            .events
            .iter()
            .filter(|e| e.at >= floor)
            .filter(|e| q.device.as_deref().is_none_or(|d| e.device_id == d))
            .filter(|e| q.kind.is_none_or(|k| e.kind == k))
            .cloned()
            .collect();
        SyncLog {
            observed_at: self.clock,
            events,
        }
    }

    pub(super) fn operation(&mut self) -> String {
        let id = format!("op_{}", self.next_operation);
        self.next_operation += 1;
        id
    }

    /// Advance the clock and log one event at the new time.
    pub(super) fn record(&mut self, mut e: SyncEvent) {
        self.clock += 14;
        e.id = format!("evt_s{}", self.next_event);
        self.next_event += 1;
        e.at = self.clock;
        self.events.insert(0, e);
    }

    pub(super) fn event(
        kind: SyncEventKind,
        device: (&str, &str),
        shape: Option<&str>,
        detail: Option<String>,
    ) -> SyncEvent {
        SyncEvent {
            id: String::new(),
            at: 0,
            kind,
            device_id: device.0.to_string(),
            device_name: device.1.to_string(),
            database: "analytics".to_string(),
            shape: shape.map(str::to_string),
            direction: None,
            detail,
            received: None,
            applied: None,
            acknowledged: None,
            mutation: None,
            actor: kind.administrative().then(|| ADMIN.to_string()),
        }
    }

    fn device_mut(&mut self, id: &str) -> Result<&mut Device, StudioError> {
        self.devices
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| missing("device", id))
    }

    /// Pause or resume one device. Other devices keep syncing.
    pub fn set_device_paused(
        &mut self,
        id: &str,
        paused: bool,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        let d = self.device_mut(id)?;
        if d.revision != revision {
            return Err(stale(format!("device {id}"), revision, d.revision));
        }
        let name = d.name.clone();
        d.revision += 1;
        d.state = if paused {
            DeviceState::Paused
        } else {
            // A resumed device has to reconnect before it is connected.
            DeviceState::Connecting
        };
        let kind = if paused {
            SyncEventKind::DevicePaused
        } else {
            SyncEventKind::DeviceResumed
        };
        self.record(Self::event(kind, (id, &name), None, None));
        let operation = self.operation();
        Ok(OpResult {
            operation,
            summary: if paused {
                format!("Paused sync for {name}. Other devices keep syncing.")
            } else {
                format!("Resumed sync for {name}. It reconnects before it syncs.")
            },
        })
    }

    fn sub_mut(&mut self, id: &str) -> Result<&mut Subscription, StudioError> {
        self.subs
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| missing("subscription", id))
    }

    /// Pause or resume one subscription. The device's others keep syncing.
    pub fn set_subscription_paused(
        &mut self,
        id: &str,
        paused: bool,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        let s = self.sub_mut(id)?;
        if s.revision != revision {
            return Err(stale(
                format!("subscription {}", s.name),
                revision,
                s.revision,
            ));
        }
        s.revision += 1;
        s.state = if paused {
            SubState::Paused
        } else {
            SubState::Active
        };
        let (name, device) = (s.name.clone(), (s.device_id.clone(), s.device_name.clone()));
        let detail = if paused { "paused" } else { "resumed" };
        self.record(Self::event(
            SyncEventKind::SubscriptionChanged,
            (&device.0, &device.1),
            Some(&name),
            Some(detail.to_string()),
        ));
        let operation = self.operation();
        Ok(OpResult {
            operation,
            summary: format!(
                "{} subscription {name} on {}.",
                if paused { "Paused" } else { "Resumed" },
                device.1
            ),
        })
    }

    fn scope(sub: &Subscription, change: &SubChange) -> Scope {
        let rows = match (sub.collection.as_str(), change.parameters.first()) {
            ("orders", Some((name, value))) if name == "store" => store_rows(value),
            _ if *change == sub.as_change() => match sub.snapshot {
                Snapshot::Complete { rows } => rows,
                Snapshot::Running { total, .. } => total,
                Snapshot::Pending => 0,
            },
            // The server would count them. The mock keeps three quarters.
            _ => match sub.snapshot {
                Snapshot::Complete { rows } => rows * 3 / 4,
                Snapshot::Running { total, .. } => total * 3 / 4,
                Snapshot::Pending => 0,
            },
        };
        Scope {
            rows,
            bytes: rows * ROW_BYTES,
        }
    }

    /// What `change` would do to subscription `id`. Nothing changes.
    pub fn preview_change(
        &self,
        id: &str,
        change: &SubChange,
    ) -> Result<ChangePreview, StudioError> {
        let sub = self
            .subs
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| missing("subscription", id))?;
        if let Some(issue) = change.issues().into_iter().next() {
            return Err(StudioError::Rejected { reason: issue });
        }
        let current = Self::scope(sub, &sub.as_change());
        let proposed = Self::scope(sub, change);
        let changed = *change != sub.as_change();
        let device = self.devices.iter().find(|d| d.id == sub.device_id);
        let mut effects = Vec::new();
        let mut warnings = Vec::new();
        if changed {
            effects.push(format!(
                "{} removes {} rows that no longer match.",
                sub.device_name,
                crate::models::format::thousands(current.rows)
            ));
            effects.push(format!(
                "Origin sends a new snapshot of {} rows before live changes resume.",
                crate::models::format::thousands(proposed.rows)
            ));
        } else {
            effects.push("The definition is unchanged. Nothing is sent.".to_string());
        }
        effects.push(format!(
            "Other subscriptions of {} keep syncing.",
            sub.device_name
        ));
        if let (true, Some(d)) = (changed, device) {
            if matches!(
                d.state,
                DeviceState::Offline | DeviceState::Paused | DeviceState::Error
            ) {
                warnings.push(format!(
                    "{} is {}. It keeps its current rows until it reconnects and finishes the snapshot.",
                    d.name,
                    d.state.label().to_lowercase()
                ));
            }
            if d.pending.is_some_and(|p| p.count > 0) {
                warnings.push(format!(
                    "{} reported pending uploads. Writes it made against removed rows still reach the Origin.",
                    d.name
                ));
            }
        }
        Ok(ChangePreview {
            subscription: sub.id.clone(),
            name: sub.name.clone(),
            device_id: sub.device_id.clone(),
            revision: sub.revision,
            change: change.clone(),
            current,
            proposed,
            rows_removed: if changed { current.rows } else { 0 },
            rows_added: if changed { proposed.rows } else { 0 },
            resnapshot: changed,
            effects,
            warnings,
            permission: PermissionCheck {
                required: SUBSCRIPTION_WRITE.to_string(),
                granted: self.granted,
            },
        })
    }

    /// Apply a change against the revision its preview read.
    pub fn apply_change(
        &mut self,
        id: &str,
        change: &SubChange,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        let preview = self.preview_change(id, change)?;
        if !preview.permission.granted {
            return Err(StudioError::PermissionDenied {
                privilege: SUBSCRIPTION_WRITE,
                collection: "sync".to_string(),
            });
        }
        if preview.revision != revision {
            return Err(stale(
                format!("subscription {}", preview.name),
                revision,
                preview.revision,
            ));
        }
        if preview.unchanged() {
            return Err(StudioError::Rejected {
                reason: "The definition is unchanged.".to_string(),
            });
        }
        let at = self.clock + 14;
        let s = self.sub_mut(id)?;
        s.filter = change.filter.clone();
        s.parameters = change.parameters.clone();
        s.revision += 1;
        s.state = SubState::Bootstrapping;
        s.snapshot = Snapshot::Running {
            done: 0,
            total: preview.proposed.rows,
        };
        s.checkpoint = None;
        s.events.insert(
            0,
            SubEvent {
                at,
                kind: SubEventKind::StartingSnapshot,
                count: None,
            },
        );
        let device = (s.device_id.clone(), s.device_name.clone());
        self.record(Self::event(
            SyncEventKind::SubscriptionChanged,
            (&device.0, &device.1),
            Some(&preview.name),
            Some("definition changed".to_string()),
        ));
        let operation = self.operation();
        Ok(OpResult {
            operation,
            summary: format!(
                "Changed {} on {}. A new snapshot of {} rows starts.",
                preview.name,
                device.1,
                crate::models::format::thousands(preview.proposed.rows)
            ),
        })
    }
}
