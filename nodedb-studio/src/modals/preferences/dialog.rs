//! The Application preferences dialog.
//!
//! The panes edit a draft. Save Preferences writes it to the shared
//! `Preferences` signal, which applies it and saves it to disk. Cancel and
//! Esc drop it. These are preferences of this app on this machine, not
//! config of any database server.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::modal::Modal;
use crate::modals::preferences::appearance::AppearancePane;
use crate::modals::preferences::panes::{
    AboutPane, EditorPane, KeyboardPane, SecurityPane, TelemetryPane,
};
use crate::state::preferences::Preferences;
use crate::state::ui::{ModalKind, PrefsPane};

fn pane_icon(p: PrefsPane) -> Icon {
    match p {
        PrefsPane::Appearance => Icon::Palette,
        PrefsPane::Editor => Icon::Code,
        PrefsPane::Keyboard => Icon::Keyboard,
        PrefsPane::Security => Icon::Shield,
        PrefsPane::Telemetry => Icon::ChartColumn,
        PrefsPane::About => Icon::Info,
    }
}

/// `pane` is the open pane, owned by the modal signal: a click in the list
/// sets `ModalKind::Preferences` again and the draft survives it.
#[component]
pub fn PreferencesDialog(pane: PrefsPane) -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let mut prefs = use_context::<Signal<Preferences>>();
    let mut draft = use_signal(|| prefs.peek().clone());

    rsx! {
        Modal {
            title: "Application preferences",
            width: 1040,
            dismiss_on_scrim: false,
            class: "prefs-dialog",
            div { class: "prefs-layout",
                nav { class: "prefs-nav", "aria-label": "Preference categories",
                    for p in PrefsPane::ALL {
                        button {
                            key: "{p.label()}",
                            class: if p == pane { "prefs-cat active" } else { "prefs-cat" },
                            "aria-current": if p == pane { "page" } else { "false" },
                            onclick: move |_| modal.set(Some(ModalKind::Preferences(p))),
                            Ico { icon: pane_icon(p), size: 18 }
                            "{p.label()}"
                        }
                    }
                }
                div { class: "prefs-content",
                    match pane {
                        PrefsPane::Appearance => rsx! { AppearancePane { draft } },
                        PrefsPane::Editor => rsx! { EditorPane { draft } },
                        PrefsPane::Keyboard => rsx! { KeyboardPane {} },
                        PrefsPane::Security => rsx! { SecurityPane { draft } },
                        PrefsPane::Telemetry => rsx! { TelemetryPane { draft } },
                        PrefsPane::About => rsx! { AboutPane {} },
                    }
                }
            }
            footer { class: "dialog-footer spread",
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: move |_| draft.set(Preferences::default()),
                    Ico { icon: Icon::RotateCcw, size: 16 }
                    "Restore defaults"
                }
                span { class: "footer-spacer" }
                button { class: "btn", r#type: "button", onclick: move |_| modal.set(None), "Cancel" }
                button {
                    class: "btn primary",
                    r#type: "button",
                    onclick: move |_| {
                        prefs.set(draft.peek().clone());
                        modal.set(None);
                    },
                    "Save preferences"
                }
            }
        }
    }
}
