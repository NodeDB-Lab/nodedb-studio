//! Shared loading/empty/error renderer for any seam-backed read.
//!
//! The caller maps its `use_resource` result to `AsyncState<T>` (plain Rust),
//! renders the `Loaded(T)` case itself, and delegates the three non-loaded
//! states to this component. Keeps every wired view from re-implementing the
//! spinner / empty / error+retry markup.
//!
//! Calling convention (wired up by plan 01-04):
//! ```ignore
//! match AsyncState::from_value(resource.read().clone()) {
//!     AsyncState::Loaded(data) => rsx! { /* caller's own markup for `data` */ },
//!     other => rsx! {
//!         AsyncView {
//!             loading: matches!(other, AsyncState::Loading),
//!             empty: matches!(other, AsyncState::Empty),
//!             error: match &other { AsyncState::Error(e) => Some(e.to_string()), _ => None },
//!             retriable: matches!(&other, AsyncState::Error(e) if e.is_retriable()),
//!             on_retry: move |_| resource.restart(),
//!         }
//!     }
//! }
//! ```
//! Discrete props (rather than the generic `AsyncState<T>`) are used because
//! Dioxus props must be `Clone + PartialEq`; `StudioError` is neither, and `T`
//! is unconstrained — so the caller decodes the state and passes flags.

use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct AsyncViewProps {
    /// True while the first fetch is pending.
    pub loading: bool,
    /// True when the fetch returned an empty result.
    pub empty: bool,
    /// Some(message) when the fetch failed; None otherwise.
    pub error: Option<String>,
    /// Whether the failed fetch is retriable (gates the Retry button).
    #[props(default = false)]
    pub retriable: bool,
    /// Called when the user clicks Retry (wire to Resource::restart()).
    #[props(default)]
    pub on_retry: EventHandler<()>,
    /// Message shown in the empty state.
    #[props(default = "No records.".to_string())]
    pub empty_message: String,
}

// Consumed by the notification popover (01-04) and later wired views.
#[component]
pub fn AsyncView(props: AsyncViewProps) -> Element {
    if props.loading {
        return rsx! { div { class: "async-loading", "Loading…" } };
    }
    if let Some(msg) = props.error.clone() {
        return rsx! {
            div { class: "async-error",
                div { class: "async-error-msg", "{msg}" }
                if props.retriable {
                    button {
                        class: "btn",
                        onclick: move |_| props.on_retry.call(()),
                        "Retry"
                    }
                }
            }
        };
    }
    if props.empty {
        return rsx! { div { class: "async-empty", "{props.empty_message}" } };
    }
    // Loaded: the caller renders its own markup; AsyncView renders nothing.
    rsx! {}
}
