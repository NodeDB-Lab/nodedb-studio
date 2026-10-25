//! The Retention & rollups body: the tiers of the collection as a table,
//! and what the selected one groups by, stores, and keeps. A saved
//! definition is shown as saved. Nothing here says a tier is fresh or
//! complete: that is runtime telemetry the server does not report yet.

use dioxus::prelude::*;

use crate::models::ts_lifecycle::{Lifecycle, Tier};

/// The selected tier, or the raw collection when none is chosen.
pub fn selected_tier<'a>(tiers: &'a [Tier], selected: Option<&str>) -> &'a Tier {
    selected
        .and_then(|s| tiers.iter().find(|t| t.name == s))
        .unwrap_or(&tiers[0])
}

#[component]
pub fn RetentionBody(
    l: Lifecycle,
    selected: Option<String>,
    on_select: EventHandler<String>,
) -> Element {
    let tiers = l.tiers();
    let current = selected_tier(&tiers, selected.as_deref()).clone();
    let rollup = l.rollup(&current.name).cloned();
    let (dimensions, values): (Vec<String>, Vec<String>) = match &rollup {
        Some(r) => (
            r.group_by.clone(),
            r.outputs.iter().map(|o| o.line()).collect(),
        ),
        None => (l.group_fields.clone(), Vec::new()),
    };
    let archive = l
        .archive
        .clone()
        .unwrap_or_else(|| "Not configured".to_string());

    rsx! {
        div { class: "dt-wrap ts-tiers",
            table { class: "dt",
                thead {
                    tr {
                        th { "Dataset" }
                        th { "Input" }
                        th { "Bucket" }
                        th { "Keep" }
                        th { "Refresh" }
                    }
                }
                tbody {
                    for t in tiers.iter() {
                        {
                            let name = t.name.clone();
                            let is_sel = t.name == current.name;
                            let input = t.input.clone().unwrap_or_else(|| "—".to_string());
                            let bucket = t.bucket.map_or("Raw", |b| b.label());
                            let keep = t.keep.label();
                            let refresh = t.refresh.as_ref().map_or_else(|| "—".to_string(), |r| r.label());
                            rsx! {
                                tr {
                                    key: "{t.name}",
                                    class: if is_sel { "clickable selected" } else { "clickable" },
                                    "aria-selected": "{is_sel}",
                                    tabindex: "0",
                                    onclick: {
                                        let name = name.clone();
                                        move |_| on_select.call(name.clone())
                                    },
                                    onkeydown: {
                                        let name = name.clone();
                                        move |e| {
                                            if matches!(e.key(), Key::Enter) || matches!(&e.key(), Key::Character(c) if c == " ") {
                                                e.prevent_default();
                                                on_select.call(name.clone());
                                            }
                                        }
                                    },
                                    td { class: "key", "{t.name}" }
                                    td { "{input}" }
                                    td { "{bucket}" }
                                    td { "{keep}" }
                                    td { "{refresh}" }
                                }
                            }
                        }
                    }
                }
            }
        }
        div { class: "ts-tier-facts",
            div { class: "ts-fact",
                h3 { "Dimensions" }
                div { class: "ts-chips",
                    for d in dimensions.iter() {
                        span { key: "{d}", class: "chip static", "{d}" }
                    }
                }
            }
            div { class: "ts-fact",
                h3 { "Rollup values" }
                if values.is_empty() {
                    div { class: "ts-muted", "Raw rows. {current.name} stores every sale as written." }
                } else {
                    pre { class: "code-block ts-values",
                        for v in values.iter() {
                            span { key: "{v}", "{v}\n" }
                        }
                    }
                }
            }
            div { class: "ts-fact",
                h3 { "Source metadata" }
                dl { class: "ts-props",
                    dt { "Raw retention" }
                    dd { "{l.raw_keep.label()}" }
                    dt { "Partition" }
                    dd { "{l.partition}" }
                    dt { "Archive" }
                    dd { class: if l.archive.is_none() { "ts-muted" } else { "" }, "{archive}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;

    #[tokio::test]
    async fn the_selected_tier_falls_back_to_the_raw_collection() {
        let l = MockConnectionService::ready()
            .lifecycle("sales")
            .await
            .expect("l");
        let tiers = l.tiers();
        assert_eq!(selected_tier(&tiers, None).name, "sales");
        assert_eq!(
            selected_tier(&tiers, Some("sales_daily")).name,
            "sales_daily"
        );
        assert_eq!(selected_tier(&tiers, Some("gone")).name, "sales");
    }
}
