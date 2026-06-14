//! Shared loading/empty/error renderer for any seam-backed read.
//!
//! The caller maps its `use_resource` result to `AsyncState<T>` (plain Rust),
//! renders the `Loaded(T)` case itself, and delegates the three non-loaded
//! states to this component. Keeps every wired view from re-implementing the
//! spinner / empty / error+retry markup.
//!
//! Calling convention: build one `AsyncState<T>` via `from_value` and drive both
//! this component and the loaded markup from it — no inline match arms:
//! ```ignore
//! let state = AsyncState::from_value(resource.read().clone());
//! rsx! {
//!     AsyncView {
//!         loading: state.is_loading(),
//!         empty: state.is_empty(),
//!         error: state.error_message(),
//!         retriable: state.is_retriable(),
//!         on_retry: move |_| resource.restart(),
//!     }
//!     if let Some(data) = state.loaded() { /* caller's own markup for `data` */ }
//! }
//! ```
//! Discrete props (rather than the generic `AsyncState<T>`) are used because
//! Dioxus props must be `Clone + PartialEq`; `StudioError`/`T` need not be — so
//! the caller decodes the state via the accessors and passes plain flags.

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

// Consumed by the notification popover and later wired views.
#[component]
pub fn AsyncView(props: AsyncViewProps) -> Element {
    if props.loading {
        return rsx! { div { class: "async-loading", "Loading…" } };
    }
    if let Some(msg) = props.error {
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

#[cfg(test)]
mod tests {
    use super::*;

    // SSR the given root component to HTML. The rsx is built INSIDE the component
    // (i.e. within the VirtualDom runtime) because `AsyncView`'s default
    // `EventHandler` prop can only be constructed while a runtime is in scope.
    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn loading_renders_spinner() {
        fn app() -> Element {
            rsx! { AsyncView { loading: true, empty: false, error: None } }
        }
        let html = render(app);
        assert!(html.contains("async-loading"));
        assert!(html.contains("Loading"));
    }

    #[test]
    fn empty_renders_message() {
        fn app() -> Element {
            rsx! {
                AsyncView {
                    loading: false,
                    empty: true,
                    error: None,
                    empty_message: "Nothing here".to_string(),
                }
            }
        }
        let html = render(app);
        assert!(html.contains("async-empty"));
        assert!(html.contains("Nothing here"));
    }

    #[test]
    fn retriable_error_shows_retry() {
        fn app() -> Element {
            rsx! {
                AsyncView {
                    loading: false,
                    empty: false,
                    error: Some("boom".to_string()),
                    retriable: true,
                }
            }
        }
        let html = render(app);
        assert!(html.contains("async-error"));
        assert!(html.contains("boom"));
        assert!(html.contains("Retry"));
    }

    #[test]
    fn non_retriable_error_hides_retry() {
        fn app() -> Element {
            rsx! {
                AsyncView {
                    loading: false,
                    empty: false,
                    error: Some("boom".to_string()),
                    retriable: false,
                }
            }
        }
        let html = render(app);
        assert!(html.contains("async-error"));
        assert!(html.contains("boom"));
        assert!(!html.contains("Retry"));
    }

    #[test]
    fn loaded_renders_nothing() {
        fn app() -> Element {
            rsx! { AsyncView { loading: false, empty: false, error: None } }
        }
        let html = render(app);
        assert!(!html.contains("async-loading"));
        assert!(!html.contains("async-empty"));
        assert!(!html.contains("async-error"));
    }
}
