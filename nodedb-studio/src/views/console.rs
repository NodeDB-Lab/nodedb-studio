//! Console: a REPL-style command log + input. CLI label de-arcadedb'd to
//! "nodedb-cli dev"; leader node to node-1.

use dioxus::prelude::*;

#[component]
pub fn Console() -> Element {
    rsx! {
        div { class: "view active",
            div { style: "height: 100%; display: grid; grid-template-rows: auto 1fr auto; background: var(--bg-primary);",
                div { style: "padding: 10px 16px; background: var(--bg-secondary); border-bottom: 0.5px solid var(--border-mid); display: flex; align-items: center; gap: 10px; font-family: var(--font-mono); font-size: 11px;",
                    span { class: "pill ok", span { class: "dot" } "repl ready" }
                    span { style: "color: var(--text-tertiary);", "nodedb-cli dev · analytics" }
                    div { style: "margin-left:auto; display:flex; gap:6px;",
                        button { class: "btn small", "Clear" }
                        button { class: "btn small", "Export" }
                    }
                }
                div { style: "overflow:auto; padding: 14px 16px; font-family: var(--font-mono); font-size: 12px; line-height: 1.6;",
                    div { span { style: "color:var(--text-tertiary)", "$" } " " span { style: "color:var(--text-info)", "SHOW CLUSTER STATUS;" } }
                    div { style: "color:var(--text-secondary); padding-left: 14px;", "cluster: staging-cluster · nodes: 7 · leader: node-1 · healthy: 6/7" }
                    div { style: "margin-top:10px;", span { style: "color:var(--text-tertiary)", "$" } " " span { style: "color:var(--text-info)", "SELECT count(*) FROM events WHERE ts > NOW() - '1h'::interval;" } }
                    div { style: "color:var(--text-secondary); padding-left: 14px;", "count: 14,820 · 142ms · plan: index_scan(events_ts_idx)" }
                    div { style: "margin-top:10px;", span { style: "color:var(--text-tertiary)", "$" } " " span { style: "color:var(--text-info)", "EXPLAIN MATCH (u:User)-[:POSTED]->(p:Post) WHERE p.topic = 'db' RETURN u LIMIT 10;" } }
                    div { style: "color:var(--text-secondary); padding-left: 14px; white-space: pre;", "  expand(out, POSTED, label=Post)\n  filter(p.topic = 'db')\n  limit(10)\n  estimated cost: 1,408 · est. rows: 10" }
                    div { style: "margin-top:10px;", span { style: "color:var(--text-tertiary)", "$" } " " span { style: "color:var(--text-success)", "PEER STATUS edge-sg-1;" } }
                    div { style: "color:var(--text-secondary); padding-left: 14px;", "edge-sg-1 · follower · lag: 12ms · last_ack: 2s ago · healthy" }
                    div { style: "margin-top:10px;", span { style: "color:var(--text-tertiary)", "$" } " " span { style: "color:var(--text-danger)", "SHOW SLOW QUERIES SINCE '1h';" } }
                    div { style: "color:var(--text-secondary); padding-left: 14px;", "1 result · vector_reindex took 22m · OOM at step 4" }
                    div { style: "margin-top:10px;", span { style: "color:var(--text-tertiary)", "$" } " " span { class: "tail-pulse", style: "display:inline-block;" } }
                }
                div { style: "display:flex; align-items:center; padding: 8px 16px; border-top: 0.5px solid var(--border-soft); font-family: var(--font-mono); font-size: 12px;",
                    span { style: "color:var(--text-tertiary); margin-right:8px;", "$" }
                    input { style: "flex:1; background: transparent; border: none; outline: none; font: inherit;", placeholder: "enter a SQL, Cypher, or admin command…" }
                    span { style: "color:var(--text-tertiary); font-size:10px;", "⏎ to run · ↑↓ history" }
                }
            }
        }
    }
}
