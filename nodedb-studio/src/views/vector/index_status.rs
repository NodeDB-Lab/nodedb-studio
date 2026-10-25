//! The body of Index status: the overview, the vector counts, and the
//! builds. Every figure is the server's own. A figure it did not report
//! reads as unreported, never as zero.

use dioxus::prelude::*;

use crate::models::format::thousands;
use crate::models::vector_index::{Coverage, IndexStatus, estimate, percent};

#[component]
pub fn StatusBody(status: IndexStatus) -> Element {
    let partial = matches!(status.coverage, Coverage::Incomplete { .. });
    let live_label = if partial {
        "Live vectors (partial)"
    } else {
        "Live vectors"
    };
    let live = thousands(status.counts.live());
    let segments = status.sealed_segments.text(|n| n.to_string());
    let memory = status.memory_bytes.text(estimate);
    let disk = status.disk_bytes.text(estimate);
    let c = status.counts;
    rsx! {
        dl { class: "vec-overview",
            div { class: "vec-fig",
                dt { "{live_label}" }
                dd { class: "num", "{live}" }
            }
            div { class: "vec-fig",
                dt { "Sealed segments" }
                dd { class: "num", "{segments}" }
            }
            div { class: "vec-fig",
                dt { "Memory estimate" }
                dd { class: "num", "{memory}" }
            }
            div { class: "vec-fig",
                dt { "Disk estimate" }
                dd { class: "num", "{disk}" }
            }
        }
        section { class: "vec-block",
            h3 { class: "vec-heading", "Vector counts" }
            table { class: "vec-kv",
                tbody {
                    tr { th { scope: "row", "Sealed vectors" } td { "{thousands(c.sealed)}" } }
                    tr { th { scope: "row", "Growing vectors" } td { "{thousands(c.growing)}" } }
                    tr { th { scope: "row", "Tombstoned vectors" } td { "{thousands(c.tombstoned)}" } }
                    tr { th { scope: "row", "Tombstone ratio" } td { "{percent(c.tombstone_ratio())}" } }
                }
            }
            p { class: "vec-note", "Sealed and growing counts include tombstoned vectors." }
        }
        section { class: "vec-block",
            h3 { class: "vec-heading", "Builds" }
            table { class: "vec-builds",
                thead { tr { th { "Queued" } th { "Completed" } th { "Failed" } } }
                tbody {
                    tr {
                        td { "{status.builds.queued}" }
                        td { "{thousands(status.builds.completed)}" }
                        td { "{thousands(status.builds.failed)}" }
                    }
                }
            }
            p { class: "vec-note", "Completed and failed count builds since the core opened this index." }
        }
    }
}
