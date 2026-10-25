//! Saved connections: the searchable, sortable table.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::connections::ConnectionKind;
use crate::models::format::date_minute;
use crate::state::connections_registry::SavedConnection;
use crate::views::connections::sort::{Column, Sort};

fn kind_icon(kind: ConnectionKind) -> Icon {
    match kind {
        ConnectionKind::Origin => Icon::Database,
        ConnectionKind::Lite => Icon::Laptop,
    }
}

/// The Status cell: a connection with an open session reads Connected,
/// whatever its last check said.
pub fn status_cell(c: &SavedConnection, connected: bool) -> (&'static str, &'static str) {
    if connected {
        ("ok", "Connected")
    } else {
        (c.status.dot(), c.status.label())
    }
}

#[component]
pub fn SavedTable(
    rows: Vec<SavedConnection>,
    selected: Option<String>,
    /// The name of the connection with an open session.
    connected: Option<String>,
    sort: Option<Sort>,
    offset: i16,
    search: String,
    on_sort: EventHandler<Column>,
    on_select: EventHandler<String>,
    on_open: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt connections-table",
                thead {
                    tr {
                        for col in Column::ALL {
                            {
                                let active = sort.filter(|s| s.column == col);
                                let aria = match active {
                                    Some(s) if s.ascending => "ascending",
                                    Some(_) => "descending",
                                    None => "none",
                                };
                                rsx! {
                                    th { key: "{col.label()}", "aria-sort": "{aria}",
                                        button {
                                            class: "th-sort",
                                            onclick: move |_| on_sort.call(col),
                                            "{col.label()}"
                                            if let Some(s) = active {
                                                Ico {
                                                    icon: if s.ascending { Icon::ArrowUp } else { Icon::ArrowDown },
                                                    size: 14,
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                tbody {
                    for c in rows.iter() {
                        {
                            let is_selected = selected.as_deref() == Some(c.name.as_str());
                            let is_connected = connected.as_deref() == Some(c.name.as_str());
                            let (dot, status) = status_cell(c, is_connected);
                            let pick = c.name.clone();
                            let open = c.name.clone();
                            let key_pick = c.name.clone();
                            rsx! {
                                tr {
                                    key: "{c.name}",
                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                    "aria-selected": "{is_selected}",
                                    tabindex: "0",
                                    onclick: move |_| on_select.call(pick.clone()),
                                    ondoubleclick: move |_| on_open.call(open.clone()),
                                    onkeydown: move |e| {
                                        if e.key() == Key::Enter {
                                            on_select.call(key_pick.clone());
                                        }
                                    },
                                    td { class: "key",
                                        span { class: "cell-icon",
                                            Ico { icon: kind_icon(c.kind()), size: 18 }
                                            "{c.name}"
                                        }
                                    }
                                    td { class: "mono", "{c.target.summary()}" }
                                    td {
                                        "{c.kind().label()}"
                                        if c.read_only() {
                                            span { class: "badge outline inline", "Read-only" }
                                        }
                                    }
                                    td { "{date_minute(c.last_used, offset)}" }
                                    td { span { class: "status", span { class: "status-dot {dot}" } "{status}" } }
                                }
                            }
                        }
                    }
                    if rows.is_empty() {
                        tr {
                            td { colspan: "5", class: "table-empty", "No connection matches “{search}”." }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn an_open_session_reads_connected() {
        let staging = find("staging-cluster").await;
        assert_eq!(status_cell(&staging, true), ("ok", "Connected"));
        assert_eq!(status_cell(&staging, false), ("ok", "Available"));
    }

    #[tokio::test]
    async fn statuses_use_the_design_dots() {
        assert_eq!(
            status_cell(&find("Production").await, false),
            ("idle", "Not connected")
        );
        assert_eq!(
            status_cell(&find("test-cluster").await, false),
            ("err", "Offline")
        );
    }
}
