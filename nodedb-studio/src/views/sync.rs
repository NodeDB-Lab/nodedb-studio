//! Sync: CRDT replication status between this connection and its peers.

use dioxus::prelude::*;

#[component]
pub fn Sync() -> Element {
    // (peer, addr, role, lag, last_sync, ops, pill-css, status)
    let peers = [
        (
            "edge-sg-1",
            "10.4.12.18",
            "follower",
            "12ms",
            "2s ago",
            "8.2M / 14.1M",
            "ok",
            "healthy",
        ),
        (
            "edge-tokyo-1",
            "",
            "follower",
            "88ms",
            "3s ago",
            "8.2M / 12.0M",
            "ok",
            "healthy",
        ),
        (
            "edge-mobile-fleet",
            "",
            "edge cluster",
            "2.4s",
            "12s ago",
            "2.1M / 5.4M",
            "warn",
            "degraded",
        ),
    ];
    rsx! {
        div { class: "view active",
            div { class: "view-padded",
                div { class: "view-header",
                    div { h1 { "Sync" } p { "CRDT replication between this connection and its peers." } }
                    div { class: "view-header-actions",
                        button { class: "btn", "Pause sync" }
                        button { class: "btn primary", "+ Peer" }
                    }
                }
                div { class: "placeholder-grid",
                    div { class: "ph-stat", div { class: "l", "PEERS" } div { class: "v", "3" } div { class: "sub", "all healthy" } }
                    div { class: "ph-stat", div { class: "l", "PENDING OPS" } div { class: "v", "142" } div { class: "sub", "drained in ~3s" } }
                    div { class: "ph-stat", div { class: "l", "CONFLICTS" } div { class: "v", style: "color:var(--text-warning)", "2" } div { class: "sub", "awaiting resolution" } }
                    div { class: "ph-stat", div { class: "l", "P50 LATENCY" } div { class: "v", "88 ms" } div { class: "sub", "peer → ack" } }
                }
                div { class: "ph-section",
                    h2 { "Peers" }
                    div { class: "card", style: "padding:0;",
                        table { class: "table",
                            thead { tr { th { "Peer" } th { "Role" } th { "Lag" } th { "Last sync" } th { "Ops sent / received" } th { "Status" } } }
                            tbody {
                                for p in peers {
                                    tr {
                                        td {
                                            strong { "{p.0}" }
                                            if !p.1.is_empty() {
                                                br {}
                                                span { style: "color:var(--text-tertiary);font-family:var(--font-mono);font-size:11px;", "{p.1}" }
                                            }
                                        }
                                        td { "{p.2}" }
                                        td { class: "mono", "{p.3}" }
                                        td { class: "mono", "{p.4}" }
                                        td { class: "mono", "{p.5}" }
                                        td { span { class: "pill {p.6}", "{p.7}" } }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "ph-section",
                    h2 { "Conflicts (2)" }
                    div { class: "card", style: "padding:14px 16px;",
                        div { style: "display:flex; justify-content:space-between; align-items:center; padding-bottom:10px; border-bottom:0.5px solid var(--border-soft);",
                            div {
                                strong { "users / u_44182 / email" }
                                div { style: "font-family:var(--font-mono); font-size:11px; color:var(--text-tertiary);", "two writes, ts within 14ms" }
                            }
                            span { class: "pill warn", "LWW pending" }
                        }
                        div { style: "display:grid; grid-template-columns:1fr 1fr; gap:14px; padding:14px 0;",
                            div {
                                div { class: "eyebrow", style: "margin-bottom:4px;", "local · 04:22:18.041" }
                                code { style: "font-family:var(--font-mono); font-size:11px;", "\"aisha@nodedb.io\"" }
                            }
                            div {
                                div { class: "eyebrow", style: "margin-bottom:4px;", "edge-sg-1 · 04:22:18.055" }
                                code { style: "font-family:var(--font-mono); font-size:11px;", "\"aisha.tan@nodedb.io\"" }
                            }
                        }
                        div { style: "display:flex; gap:6px;",
                            button { class: "btn small", "Keep local" }
                            button { class: "btn small", "Keep remote" }
                            button { class: "btn small", "Merge…" }
                            button { class: "btn small primary", "Accept LWW" }
                        }
                    }
                }
            }
        }
    }
}
