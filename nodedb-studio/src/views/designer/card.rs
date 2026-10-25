//! One collection on the canvas: header, fields, and attached indexes.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::collection::Profile;
use crate::models::designer::{DesignCollection, DesignIndex};
use crate::models::schema::IndexKind;
use crate::routes::Route;
use crate::state::designer::Target;
use crate::state::explorer::{RecordView, Selected, select_collection};
use crate::views::designer::layout::{FIELD_H, HEADER_H, INDEX_H, SECTION_H, SETTINGS_H};

/// The tone of the one-letter square. Strict is amber, as the design shows.
pub fn letter_tone(profile: Profile) -> &'static str {
    match profile {
        Profile::Document | Profile::Array => "blue",
        Profile::DocumentStrict => "amber",
        Profile::KeyValue => "orange",
        Profile::Columnar => "gray",
        Profile::Timeseries => "green",
        Profile::Spatial => "violet",
    }
}

/// The glyph before an index name.
pub fn index_icon(i: &DesignIndex) -> Icon {
    match (i.kind, i.unique) {
        (IndexKind::Vector, _) => Icon::Grid3x3,
        (IndexKind::FullText, _) => Icon::Search,
        (IndexKind::Secondary, true) => Icon::Key,
        (IndexKind::Secondary, false) | (IndexKind::Primary, _) => Icon::Hash,
        (IndexKind::Spatial, _) => Icon::MapPin,
    }
}

#[component]
pub fn Card(
    collection: DesignCollection,
    x: f64,
    y: f64,
    width: f64,
    selected: Option<Target>,
    on_select: EventHandler<Target>,
    on_drag: EventHandler<(f64, f64)>,
    on_add_index: EventHandler<String>,
    on_add_reference: EventHandler<String>,
) -> Element {
    let nav = use_navigator();
    let mut explorer = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let mut menu = use_signal(|| false);
    let name = collection.name().to_string();
    let profile = collection.profile();
    let card_selected = selected == Some(Target::Collection(name.clone()))
        || collection
            .indexes
            .iter()
            .any(|i| selected == Some(Target::Index(i.name.clone())));
    let class = format!(
        "dcard{}{}",
        if card_selected { " selected" } else { "" },
        if collection.state.is_draft() {
            " draft"
        } else {
            ""
        }
    );
    let style = format!("left:{x}px; top:{y}px; width:{width}px;");

    rsx! {
        article { class: "{class}", style: "{style}", "aria-label": "{name}",
            header {
                class: "dcard-head",
                style: "height:{HEADER_H}px;",
                onmousedown: move |e| {
                    if e.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary) {
                        e.prevent_default();
                        on_drag.call((e.client_coordinates().x, e.client_coordinates().y));
                    }
                },
                onclick: {
                    let name = name.clone();
                    move |_| on_select.call(Target::Collection(name.clone()))
                },
                span { class: "badge letter {letter_tone(profile)}", "{profile.letter()}" }
                span { class: "dcard-name", "{name}" }
                span { class: "badge", "{profile.badge()}" }
                if collection.state.is_draft() {
                    span { class: "badge outline", "Draft" }
                }
                div { class: "anchor",
                    button {
                        class: "icon-btn sm",
                        "aria-label": "{name} actions",
                        "aria-haspopup": "menu",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            menu.toggle();
                        },
                        Ico { icon: Icon::Ellipsis, size: 16 }
                    }
                    if menu() {
                        div {
                            class: "popover align-right",
                            role: "menu",
                            onmousedown: move |e| e.stop_propagation(),
                            onclick: move |e| e.stop_propagation(),
                            button {
                                class: "menu-item",
                                role: "menuitem",
                                onclick: {
                                    let name = name.clone();
                                    move |_| {
                                        menu.set(false);
                                        on_add_index.call(name.clone());
                                    }
                                },
                                Ico { icon: Icon::Plus, size: 16 }
                                "Add index"
                            }
                            button {
                                class: "menu-item",
                                role: "menuitem",
                                onclick: {
                                    let name = name.clone();
                                    move |_| {
                                        menu.set(false);
                                        on_add_reference.call(name.clone());
                                    }
                                },
                                Ico { icon: Icon::Link, size: 16 }
                                "Add reference"
                            }
                            button {
                                class: "menu-item",
                                role: "menuitem",
                                onclick: {
                                    let name = name.clone();
                                    move |_| {
                                        menu.set(false);
                                        select_collection(&mut explorer, &mut view, &name);
                                        nav.push(Route::Explorer {});
                                    }
                                },
                                Ico { icon: Icon::Table, size: 16 }
                                "Open in Explorer"
                            }
                        }
                    }
                }
            }
            if let Some(heading) = collection.heading {
                div { class: "dcard-section", style: "height:{SECTION_H}px;", "{heading.label()}" }
            }
            for f in collection.fields.iter() {
                div {
                    key: "{f.name()}",
                    class: "dcard-field",
                    style: "height:{FIELD_H}px;",
                    onclick: {
                        let name = name.clone();
                        move |_| on_select.call(Target::Collection(name.clone()))
                    },
                    span { class: "dcard-fname", "{f.name()}" }
                    span { class: "dcard-ftype", "{f.ty().keyword()}" }
                    span { class: "dcard-flags",
                        for flag in f.flags(profile) {
                            span { key: "{flag.label()}", class: "badge", "{flag.label()}" }
                        }
                    }
                }
            }
            if !collection.indexes.is_empty() {
                div { class: "dcard-section", style: "height:{SECTION_H}px;", "Indexes" }
                for i in collection.indexes.iter() {
                    {
                        let is_selected = selected == Some(Target::Index(i.name.clone()));
                        let target = Target::Index(i.name.clone());
                        rsx! {
                            button {
                                key: "{i.name}",
                                class: if is_selected { "dcard-index selected" } else { "dcard-index" },
                                style: "height:{INDEX_H}px;",
                                "aria-pressed": "{is_selected}",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    on_select.call(target.clone());
                                },
                                Ico { icon: index_icon(i), size: 16 }
                                span { class: "dcard-iname", "{i.name}" }
                                span { class: "dcard-isum", "{i.summary()}" }
                                if i.state.is_draft() {
                                    span { class: "badge outline", "Draft" }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(settings) = collection.settings.as_ref() {
                div { class: "dcard-settings", style: "height:{SETTINGS_H}px;",
                    Ico { icon: Icon::Settings, size: 16 }
                    "{settings}"
                }
            }
        }
    }
}
