use super::docs::{FieldValue, MockDoc, doc};
use FieldValue::Str;

/// A LISTEN/NOTIFY channel in the sidebar.
pub struct NotifyChannel {
    pub name: &'static str,
    pub listeners: &'static str,
    pub active: bool,
}

/// One row in the LISTEN/NOTIFY live tail.
pub struct NotifyMessage {
    pub time: &'static str,
    pub source: &'static str,
    pub payload: MockDoc,
}

/// The notify channel list.
pub fn notify_channels() -> Vec<NotifyChannel> {
    let ch = |name, listeners, active| NotifyChannel {
        name,
        listeners,
        active,
    };
    vec![
        ch("user_events", "12", true),
        ch("deploy_hooks", "3", false),
        ch("cache_invalidate", "5", false),
        ch("alerts", "8", false),
        ch("jobs_done", "14", false),
        ch("presence_room_1", "22", false),
    ]
}

/// The pub/sub message tail for the active channel.
pub fn notify_messages() -> Vec<NotifyMessage> {
    vec![
        NotifyMessage {
            time: "04:23:18.041",
            source: "api-server-2",
            payload: doc(vec![("event", Str("login")), ("user", Str("u_44182"))]),
        },
        NotifyMessage {
            time: "04:23:17.812",
            source: "webhook-relay",
            payload: doc(vec![
                ("event", Str("signup")),
                ("user", Str("u_99001")),
                ("plan", Str("pro")),
            ]),
        },
        NotifyMessage {
            time: "04:23:17.501",
            source: "api-server-1",
            payload: doc(vec![
                ("event", Str("profile_update")),
                ("user", Str("u_77103")),
            ]),
        },
        NotifyMessage {
            time: "04:23:16.998",
            source: "analytics",
            payload: doc(vec![
                ("event", Str("page_view")),
                ("user", Str("u_44182")),
                ("path", Str("/pricing")),
            ]),
        },
        NotifyMessage {
            time: "04:23:16.422",
            source: "api-server-2",
            payload: doc(vec![("event", Str("logout")), ("user", Str("u_31001"))]),
        },
    ]
}
