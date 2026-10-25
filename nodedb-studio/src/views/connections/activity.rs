//! Recent activity: the connection log, newest first.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::connections::{ConnectionEvent, ConnectionKind};
use crate::models::format::date_second;

fn kind_icon(kind: ConnectionKind) -> Icon {
    match kind {
        ConnectionKind::Origin => Icon::Database,
        ConnectionKind::Lite => Icon::Laptop,
    }
}

#[component]
pub fn ActivityTable(events: Vec<ConnectionEvent>, offset: i16) -> Element {
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt",
                thead {
                    tr {
                        th { "Time" }
                        th { "Connection" }
                        th { "Event" }
                        th { "Details" }
                    }
                }
                tbody {
                    for (i, e) in events.iter().enumerate() {
                        tr { key: "{e.at}-{e.connection}-{i}",
                            td { class: "mono", "{date_second(e.at, offset)}" }
                            td {
                                span { class: "cell-icon",
                                    Ico { icon: kind_icon(e.kind), size: 18 }
                                    "{e.connection}"
                                }
                            }
                            td { span { class: "status", span { class: "status-dot {e.outcome.dot()}" } "{e.outcome.label()}" } }
                            td { "{e.detail}" }
                        }
                    }
                }
            }
        }
    }
}
