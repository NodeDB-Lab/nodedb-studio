//! Sync device fixtures: the Lite devices staging-cluster knows. Times are
//! the server's clock around 2026-06-13 13:40 UTC. Devices report on their
//! own schedule, so each row carries its own observation time and a stage
//! nobody reported stays unknown.

use crate::data::mock::cluster::members::OBSERVED;
use crate::models::format::epoch;
use crate::models::sync::{
    Device, DeviceState, DownloadStages, OriginBinding, Pending, UploadStages,
};

/// Seconds since the epoch at 13:40 UTC plus the given clock.
pub fn t(h: u32, m: u32, s: u32) -> i64 {
    epoch(2026, 6, 13, h, m, s)
}

/// The same instant in milliseconds, with `milli` past the second.
pub fn ms(h: u32, m: u32, s: u32, milli: i64) -> i64 {
    t(h, m, s) * 1000 + milli
}

pub fn origin() -> OriginBinding {
    OriginBinding {
        name: "staging-cluster".to_string(),
        cluster_id: "cl_a1".to_string(),
        endpoint: "sync.staging.example:9443".to_string(),
    }
}

struct Seed {
    id: &'static str,
    name: &'static str,
    state: DeviceState,
    principal: &'static str,
    registered: (u32, u32),
    contact: (u32, u32, u32),
    transport: Option<&'static str>,
    fault: Option<&'static str>,
    pending: Option<(u32, (u32, u32, u32))>,
    upload: UploadStages,
    download: DownloadStages,
}

fn device(seed: Seed) -> Device {
    let (rm, rd) = seed.registered;
    Device {
        id: seed.id.to_string(),
        name: seed.name.to_string(),
        state: seed.state,
        principal: seed.principal.to_string(),
        registered_at: epoch(2026, rm, rd, 9, 0, 0),
        last_contact: t(seed.contact.0, seed.contact.1, seed.contact.2),
        transport: seed.transport.map(str::to_string),
        fault: seed.fault.map(str::to_string),
        pending: seed.pending.map(|(count, (h, m, s))| Pending {
            count,
            reported_at: t(h, m, s),
        }),
        upload: seed.upload,
        download: seed.download,
        observed_at: if seed.state == DeviceState::Connected {
            OBSERVED
        } else {
            t(seed.contact.0, seed.contact.1, seed.contact.2)
        },
        revision: 1,
    }
}

pub fn seeded_devices() -> Vec<Device> {
    use DeviceState as S;
    let up = |r, a, k| UploadStages {
        received_ms: r,
        applied_ms: a,
        acknowledged_ms: k,
    };
    let down = |s, k, awaiting| DownloadStages {
        sent_ms: s,
        device_acknowledged_ms: k,
        awaiting,
        ..DownloadStages::default()
    };
    vec![
        device(Seed {
            id: "lite_007",
            name: "lite-sg-07",
            state: S::Connected,
            principal: "sync_apac",
            registered: (5, 2),
            contact: (13, 40, 4),
            transport: Some("QUIC from 203.0.113.41"),
            fault: None,
            pending: Some((28, (13, 40, 4))),
            upload: up(
                Some(ms(13, 40, 3, 100)),
                Some(ms(13, 40, 3, 200)),
                Some(ms(13, 40, 3, 400)),
            ),
            download: down(Some(ms(13, 40, 2, 0)), Some(ms(13, 40, 2, 500)), Some(3)),
        }),
        device(Seed {
            id: "lite_003",
            name: "lite-kl-03",
            state: S::Offline,
            principal: "sync_apac",
            registered: (4, 18),
            contact: (13, 18, 6),
            transport: None,
            fault: None,
            pending: None,
            upload: up(
                Some(ms(13, 17, 52, 40)),
                Some(ms(13, 17, 52, 110)),
                Some(ms(13, 17, 52, 300)),
            ),
            download: down(Some(ms(13, 17, 50, 0)), Some(ms(13, 17, 50, 420)), None),
        }),
        device(Seed {
            id: "lite_002",
            name: "lite-tokyo-02",
            state: S::Connecting,
            principal: "sync_apac",
            registered: (3, 29),
            contact: (13, 39, 57),
            transport: None,
            fault: None,
            pending: None,
            upload: up(Some(ms(13, 12, 8, 260)), Some(ms(13, 12, 8, 330)), None),
            download: down(Some(ms(13, 12, 7, 0)), None, None),
        }),
        device(Seed {
            id: "d17",
            name: "checkout-ipad",
            state: S::Connected,
            principal: "sync_checkout",
            registered: (6, 1),
            contact: (13, 40, 5),
            transport: Some("WebSocket from 198.51.100.17"),
            fault: None,
            pending: Some((1, (13, 40, 4))),
            upload: up(
                Some(ms(13, 40, 0, 120)),
                Some(ms(13, 38, 41, 300)),
                Some(ms(13, 38, 41, 480)),
            ),
            download: DownloadStages {
                sent_ms: Some(ms(13, 39, 31, 0)),
                device_received_ms: Some(ms(13, 39, 31, 300)),
                device_applied_ms: Some(ms(13, 39, 31, 450)),
                device_acknowledged_ms: Some(ms(13, 39, 31, 600)),
                awaiting: Some(0),
            },
        }),
        device(Seed {
            id: "d08",
            name: "front-desk-ipad",
            state: S::Connected,
            principal: "sync_checkout",
            registered: (6, 1),
            contact: (13, 40, 3),
            transport: Some("WebSocket from 198.51.100.8"),
            fault: None,
            pending: Some((0, (13, 40, 3))),
            upload: up(
                Some(ms(13, 31, 12, 90)),
                Some(ms(13, 31, 12, 200)),
                Some(ms(13, 31, 12, 350)),
            ),
            download: down(Some(ms(13, 39, 3, 0)), Some(ms(13, 39, 3, 410)), Some(0)),
        }),
        device(Seed {
            id: "d21",
            name: "kiosk-21",
            state: S::Connected,
            principal: "sync_checkout",
            registered: (6, 10),
            contact: (13, 40, 5),
            transport: Some("WebSocket from 198.51.100.21"),
            fault: None,
            pending: None,
            upload: up(Some(ms(13, 36, 40, 10)), None, None),
            download: down(Some(ms(13, 40, 1, 0)), None, Some(2)),
        }),
        device(Seed {
            id: "d09",
            name: "stock-ipad",
            state: S::Paused,
            principal: "sync_checkout",
            registered: (5, 20),
            contact: (13, 33, 40),
            transport: None,
            fault: None,
            pending: None,
            upload: up(
                Some(ms(13, 33, 30, 500)),
                Some(ms(13, 33, 30, 640)),
                Some(ms(13, 33, 30, 800)),
            ),
            download: down(Some(ms(13, 33, 29, 0)), Some(ms(13, 33, 29, 380)), None),
        }),
        device(Seed {
            id: "d12",
            name: "store-tablet",
            state: S::Error,
            principal: "sync_checkout",
            registered: (5, 20),
            contact: (13, 31, 20),
            transport: None,
            fault: Some("Handshake refused: the device certificate expired."),
            pending: None,
            upload: up(None, None, None),
            download: down(None, None, None),
        }),
        device(Seed {
            id: "d14",
            name: "manager-ipad",
            state: S::Offline,
            principal: "sync_checkout",
            registered: (4, 2),
            contact: (10, 42, 11),
            transport: None,
            fault: None,
            pending: None,
            upload: up(
                Some(ms(10, 42, 9, 210)),
                Some(ms(10, 42, 9, 300)),
                Some(ms(10, 42, 9, 470)),
            ),
            download: down(Some(ms(10, 41, 58, 0)), Some(ms(10, 41, 58, 330)), None),
        }),
    ]
}
