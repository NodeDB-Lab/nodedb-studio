//! Sync devices: the Lite instances an Origin cluster knows, with what each
//! one reported and what the Origin measured.
//!
//! A connected transport says nothing about how far the data got. Each stage
//! (received, applied, acknowledged) is its own fact with its own time, and a
//! stage nobody reported stays `None` and shows as Unknown. A pending count
//! belongs to the device that reported it.

use serde::{Deserialize, Serialize};

use crate::models::feeds::Tone;
use crate::models::format::clock_second;

/// What the Origin knows about one device's connection. Registered is the
/// precondition of every state, so it has no variant of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceState {
    Connected,
    Connecting,
    Offline,
    Paused,
    Error,
}

impl DeviceState {
    pub const ALL: [DeviceState; 5] = [
        DeviceState::Connected,
        DeviceState::Connecting,
        DeviceState::Offline,
        DeviceState::Paused,
        DeviceState::Error,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DeviceState::Connected => "Connected",
            DeviceState::Connecting => "Connecting",
            DeviceState::Offline => "Offline",
            DeviceState::Paused => "Paused",
            DeviceState::Error => "Error",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            DeviceState::Connected => Tone::Ok,
            DeviceState::Connecting => Tone::Warn,
            DeviceState::Offline | DeviceState::Error => Tone::Err,
            DeviceState::Paused => Tone::Idle,
        }
    }

    /// The sentence that says what the state does and does not prove.
    pub fn meaning(self) -> &'static str {
        match self {
            DeviceState::Connected => {
                "The transport is open. It does not show that sync is complete."
            }
            DeviceState::Connecting => "The device is opening a transport to the Origin.",
            DeviceState::Offline => "The device has no transport. It stays registered.",
            DeviceState::Paused => "An administrator paused sync for this device.",
            DeviceState::Error => "The last transport or handshake attempt failed.",
        }
    }
}

/// A pending-upload count the device reported. The Origin does not measure
/// it, so it carries the time of the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub count: u32,
    /// When the report arrived, seconds since the epoch.
    pub reported_at: i64,
}

/// Device to Origin. Times are milliseconds since the epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UploadStages {
    pub received_ms: Option<i64>,
    pub applied_ms: Option<i64>,
    pub acknowledged_ms: Option<i64>,
}

/// Origin to device. Received and applied come from the device's own report,
/// so they are often unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DownloadStages {
    pub sent_ms: Option<i64>,
    pub device_received_ms: Option<i64>,
    pub device_applied_ms: Option<i64>,
    pub device_acknowledged_ms: Option<i64>,
    /// Changes sent and not yet acknowledged. It is not the upload queue.
    pub awaiting: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// Stable id, e.g. `d17`. A name can change, an id never does.
    pub id: String,
    pub name: String,
    pub state: DeviceState,
    /// The role the device authenticates as.
    pub principal: String,
    pub registered_at: i64,
    /// The last transport heartbeat, seconds since the epoch.
    pub last_contact: i64,
    /// The address the Origin saw. A Lite device has no inbound endpoint.
    pub transport: Option<String>,
    /// Why the state is Error.
    pub fault: Option<String>,
    pub pending: Option<Pending>,
    pub upload: UploadStages,
    pub download: DownloadStages,
    /// When these reports were collected, seconds since the epoch.
    pub observed_at: i64,
    pub revision: u64,
}

impl Device {
    /// Whether `needle` (lowercase) appears in a field the table shows.
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [&self.id, &self.name, &self.principal]
                .iter()
                .any(|f| f.to_lowercase().contains(needle))
    }
}

/// The Origin cluster the devices belong to. The name is a label. The id is
/// what stops a rename from rebinding a device to another cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginBinding {
    pub name: String,
    pub cluster_id: String,
    /// The destination devices are configured with.
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncDevices {
    pub origin: OriginBinding,
    pub devices: Vec<Device>,
    pub observed_at: i64,
}

impl SyncDevices {
    #[cfg(test)]
    pub fn device(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    #[cfg(test)]
    pub fn count(&self, state: DeviceState) -> usize {
        self.devices.iter().filter(|d| d.state == state).count()
    }
}

/// `13:40:03.100` for milliseconds since the epoch, in UTC.
pub fn clock_ms(ms: i64) -> String {
    format!(
        "{}.{:03}",
        clock_second(ms.div_euclid(1000), 0),
        ms.rem_euclid(1000)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;

    #[test]
    fn milliseconds_print_with_three_places() {
        let ms = epoch(2026, 6, 13, 13, 40, 3) * 1000 + 100;
        assert_eq!(clock_ms(ms), "13:40:03.100");
    }

    #[test]
    fn a_connected_device_is_not_called_synced() {
        assert!(DeviceState::Connected.meaning().contains("not show"));
        assert_eq!(DeviceState::ALL.len(), 5);
    }
}
