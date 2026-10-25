//! Connection details: the selected connection's facts and actions.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::connections::{ConnectionKind, ConnectionTarget, TlsStatus};
use crate::models::format::{date, date_minute, days_between};
use crate::state::connections_registry::SavedConnection;
use crate::views::connections::saved::status_cell;

/// The Version row: the version the last check saw, labelled as such.
fn version_line(c: &SavedConnection) -> String {
    match &c.observation {
        Some(o) => format!("{} (from last observation)", o.version),
        None => "Not observed".to_string(),
    }
}

/// `2026-12-10 (in 180 days)`, counted from the check that saw the
/// certificate, or `(expired)` when it had already lapsed.
fn expiry_line(expires_at: i64, observed_at: i64, offset: i16) -> String {
    let days = days_between(observed_at, expires_at);
    let when = date(expires_at, offset);
    if days < 0 {
        format!("{when} (expired)")
    } else {
        format!("{when} (in {days} days)")
    }
}

fn badge_tone(dot: &str) -> &'static str {
    match dot {
        "ok" => "badge green",
        "err" => "badge red",
        _ => "badge gray",
    }
}

#[component]
pub fn ConnectionDetails(
    conn: SavedConnection,
    /// Whether this connection has the open session.
    connected: bool,
    offset: i16,
    on_connect: EventHandler<()>,
    on_open_workspace: EventHandler<()>,
    on_edit: EventHandler<()>,
    on_duplicate: EventHandler<()>,
) -> Element {
    let (dot, status) = status_cell(&conn, connected);
    let kind = conn.kind();
    let icon = match kind {
        ConnectionKind::Origin => Icon::Database,
        ConnectionKind::Lite => Icon::Laptop,
    };
    let tls_status = conn
        .observation
        .as_ref()
        .map(|o| o.tls.clone())
        .unwrap_or(TlsStatus::NotChecked);
    let tls_dot = match tls_status {
        TlsStatus::Verified(_) => "ok",
        TlsStatus::NotUsed | TlsStatus::NotChecked => "idle",
    };
    rsx! {
        div { class: "inspector connection-details",
            div { class: "inspector-head",
                span { class: "inspector-icon", Ico { icon, size: 26 } }
                h3 { class: "inspector-title", "{conn.name}" }
                span { class: "{badge_tone(dot)}", span { class: "status-dot {dot}" } "{status}" }
                if conn.read_only() {
                    span { class: "badge outline", "Read-only" }
                }
            }
            div { class: "inspector-actions top",
                if connected {
                    button { class: "btn primary", onclick: move |_| on_open_workspace.call(()),
                        Ico { icon: Icon::Play, size: 16 }
                        "Open workspace"
                    }
                } else {
                    button { class: "btn primary", onclick: move |_| on_connect.call(()),
                        Ico { icon: Icon::Play, size: 16 }
                        "Connect"
                    }
                }
                button { class: "btn", onclick: move |_| on_edit.call(()),
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit"
                }
                button { class: "btn", onclick: move |_| on_duplicate.call(()),
                    Ico { icon: Icon::Copy, size: 16 }
                    "Duplicate"
                }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Target" } span { "{conn.target.summary()}" } }
                div { class: "kv-row", span { "Mode" } span { "{kind.label()}" } }
                match &conn.target {
                    ConnectionTarget::Origin { host, port } => rsx! {
                        div { class: "kv-row", span { "Host" } span { "{host}" } }
                        div { class: "kv-row", span { "Port" } span { "{port}" } }
                    },
                    ConnectionTarget::Lite { path } => rsx! {
                        div { class: "kv-row", span { "File" } span { class: "mono", "{path}" } }
                    },
                }
                if kind == ConnectionKind::Origin {
                    div { class: "kv-row",
                        span { "TLS" }
                        span { class: "status", span { class: "status-dot {tls_dot}" } "{tls_status.label()}" }
                    }
                }
                if let Some(o) = conn.observation.as_ref() {
                    div { class: "kv-row",
                        span { "Database count" }
                        span { class: "with-info",
                            "{o.db_count}"
                            span { class: "info-tip", title: "Counted at the last check", Ico { icon: Icon::Info, size: 14 } }
                        }
                    }
                }
                div { class: "kv-row", span { "Version" } span { "{version_line(&conn)}" } }
                div { class: "kv-row", span { "Last used" } span { "{date_minute(conn.last_used, offset)}" } }
                div { class: "kv-row", span { "Created" } span { "{date_minute(conn.created, offset)}" } }
                if !conn.description.is_empty() {
                    div { class: "kv-row", span { "Description" } span { "{conn.description}" } }
                }
            }
            if let (TlsStatus::Verified(cert), Some(o)) = (&tls_status, conn.observation.as_ref()) {
                div { class: "kv cert",
                    div { class: "kv-row",
                        span { "Last checked" }
                        span { class: "status", span { class: "status-dot ok" } "Verified" }
                    }
                    div { class: "kv-row", span { "Issuer" } span { "{cert.issuer}" } }
                    div { class: "kv-row", span { "Expires" } span { "{expiry_line(cert.expires_at, o.at, offset)}" } }
                    div { class: "kv-row", span { "Subject" } span { class: "mono", "{cert.subject}" } }
                }
                button {
                    class: "link-btn cert-link",
                    disabled: true,
                    title: "The certificate viewer is not designed yet",
                    "View certificate details"
                    Ico { icon: Icon::ExternalLink, size: 14 }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::services::MockConnectionService;
    use crate::services::connection_service::ConnectionService;

    async fn find(name: &str) -> SavedConnection {
        MockConnectionService::ready()
            .list_connections()
            .await
            .expect("mock registry")
            .into_iter()
            .find(|c| c.name == name)
            .expect(name)
    }

    #[tokio::test]
    async fn version_names_its_source() {
        assert_eq!(
            version_line(&find("staging-cluster").await),
            "1.4.2 (from last observation)"
        );
        assert_eq!(version_line(&find("Production").await), "Not observed");
    }

    #[tokio::test]
    async fn expiry_counts_days_from_the_check() {
        let c = find("staging-cluster").await;
        let o = c.observation.expect("observed");
        let TlsStatus::Verified(cert) = o.tls else {
            panic!("staging has a verified certificate");
        };
        assert_eq!(
            expiry_line(cert.expires_at, o.at, 0),
            "2026-12-10 (in 180 days)"
        );
        assert_eq!(
            expiry_line(epoch(2026, 6, 1, 0, 0, 0), epoch(2026, 6, 13, 0, 0, 0), 0),
            "2026-06-01 (expired)"
        );
    }
}
