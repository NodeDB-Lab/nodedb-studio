//! The Rebalance preview panel: compose one replica move, stage it into the
//! plan, and open the review. The estimate shown here is what Studio knows
//! before the server previews the plan: the bytes a replica holds, and an
//! impact the server has not estimated, which reads Unknown.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::cluster_op::Impact;
use crate::models::cluster_place::{MoveRequest, Placement, VShard, size_text};
use crate::services::backend::Backend;
use crate::state::cluster::ClusterUi;
use crate::views::cluster::place_tasks;
use crate::views::events::common::StatusText;

#[component]
pub fn Rebalance(placement: Placement, vshard: VShard) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<ClusterUi>().place;
    let u = ui.read().clone();
    let group = vshard.group.clone();
    // Sources: the group's replicas. Destinations: members that answer and
    // do not already hold a replica.
    let sources: Vec<(String, String)> = placement
        .nodes
        .iter()
        .filter(|n| n.role_in(&group).is_some())
        .map(|n| (n.node_id.clone(), n.name.clone()))
        .collect();
    let destinations: Vec<(String, String)> = placement
        .nodes
        .iter()
        .filter(|n| n.role_in(&group).is_none() && n.state.reachable())
        .map(|n| (n.node_id.clone(), n.name.clone()))
        .collect();
    let from = u
        .from
        .clone()
        .filter(|f| sources.iter().any(|(id, _)| id == f));
    let to =
        u.to.clone()
            .filter(|t| destinations.iter().any(|(id, _)| id == t));
    let bytes = placement.group_bytes(&group);
    let shards = placement.group_vshards(&group);
    let ready = from.is_some() && to.is_some();
    let name_of = |id: &str| {
        placement
            .nodes
            .iter()
            .find(|n| n.node_id == id)
            .map(|n| n.name.clone())
            .unwrap_or_else(|| id.to_string())
    };
    let staged_rows: Vec<(MoveRequest, String, String, String, u64)> = u
        .staged
        .iter()
        .filter_map(|m| {
            let v = placement.vshard(m.vshard)?;
            let slot = placement
                .nodes
                .iter()
                .filter(|n| n.role_in(&v.group).is_some())
                .position(|n| n.node_id == m.from_id)
                .map(|i| i + 1)
                .unwrap_or(1);
            Some((
                m.clone(),
                format!("Replica {slot}"),
                name_of(&m.from_id),
                name_of(&m.to_id),
                placement.group_bytes(&v.group),
            ))
        })
        .collect();
    let staged_count = u.staged.len();
    let vshard_number = vshard.number;
    let impact = Impact::Unknown;
    rsx! {
        div { class: "toolbar nowrap cl-rebalance-bar",
            div { class: "cl-ctl",
                label { r#for: "move-from", title: "Move replica from", "From" }
                select {
                    id: "move-from",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        ui.write().from = (!v.is_empty()).then_some(v);
                    },
                    option { value: "", selected: from.is_none(), "Replica" }
                    for (id , name) in sources.iter() {
                        option { key: "{id}", value: "{id}", selected: from.as_ref() == Some(id), "{name}" }
                    }
                }
            }
            div { class: "cl-ctl",
                label { r#for: "move-to", "To" }
                select {
                    id: "move-to",
                    class: "select native",
                    disabled: destinations.is_empty(),
                    onchange: move |e| {
                        let v = e.value();
                        ui.write().to = (!v.is_empty()).then_some(v);
                    },
                    option { value: "", selected: to.is_none(), "Node" }
                    for (id , name) in destinations.iter() {
                        option { key: "{id}", value: "{id}", selected: to.as_ref() == Some(id), "{name}" }
                    }
                }
            }
            div { class: "cl-readout",
                span { class: "cl-readout-label", "Estimated bytes" }
                span { class: "cl-readout-value", "{size_text(bytes)}" }
            }
            div { class: "cl-readout",
                span { class: "cl-readout-label", "Estimated impact" }
                span { class: "cl-readout-value",
                    StatusText { tone: impact.tone(), text: impact.label().to_string() }
                }
            }
            div { class: "toolbar-spacer" }
            button {
                class: "btn",
                disabled: !ready,
                title: if ready { "Add this move to the plan" } else { "Choose the replica to move and its destination" },
                onclick: {
                    let (from, to) = (from.clone(), to.clone());
                    let placement = placement.clone();
                    move |_| {
                        let (Some(f), Some(t)) = (from.clone(), to.clone()) else { return };
                        ui.write().stage(
                            MoveRequest { vshard: vshard_number, from_id: f, to_id: t },
                            &placement,
                        );
                    }
                },
                Ico { icon: Icon::Plus, size: 16 }
                "Add"
            }
            button {
                class: "btn",
                disabled: staged_count == 0,
                title: if staged_count == 0 { "Add a move to the plan first" } else { "Preview the plan's impact. Nothing moves yet." },
                onclick: move |_| {
                    let backend = backend.clone();
                    spawn_forever(async move { place_tasks::open_review(ui, backend).await });
                },
                Ico { icon: Icon::Eye, size: 16 }
                "Review plan"
            }
        }
        if ready {
            div { class: "meta-line cl-move-note",
                "A replica move changes the membership of {group}. It applies to every vShard of the group: "
                span { class: "mono", {shards.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")} }
                "."
            }
        }
        div { class: "dt-wrap",
            table { class: "dt cl-table",
                thead {
                    tr {
                        th { "Replica" }
                        th { "Source node" }
                        th { "Destination node" }
                        th { class: "num", "Estimated bytes" }
                        th { "Estimated impact" }
                        th { class: "cl-x", "" }
                    }
                }
                tbody {
                    for (m , replica , source , dest , b) in staged_rows {
                        tr { key: "{m.vshard}-{m.from_id}",
                            td { "{replica}" }
                            td { "{source}" }
                            td { "{dest}" }
                            td { class: "num", "{size_text(b)}" }
                            td { StatusText { tone: impact.tone(), text: impact.label().to_string() } }
                            td { class: "cl-x",
                                button {
                                    class: "icon-btn sm",
                                    title: "Remove this move from the plan",
                                    "aria-label": "Remove the move of {replica} from {source}",
                                    onclick: {
                                        let m = m.clone();
                                        move |_| ui.write().staged.retain(|x| *x != m)
                                    },
                                    Ico { icon: Icon::X, size: 16 }
                                }
                            }
                        }
                    }
                }
            }
            if staged_count == 0 {
                div { class: "async-empty", "No move is staged. Choose a replica and a destination, then Add move." }
            }
        }
    }
}
