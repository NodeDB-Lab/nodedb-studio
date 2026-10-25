//! The number-and-unit retention input the Lifecycle sheets share.

use dioxus::prelude::*;

use crate::models::ts_lifecycle::Retention;
use crate::views::events::common::FormField;

/// Parse a retention typed as a number and a unit.
fn retention(text: &str, years: bool) -> Option<Retention> {
    let n: u32 = text.trim().parse().ok()?;
    Some(if years {
        Retention::CalendarYears(n)
    } else {
        Retention::Days(n)
    })
}

/// A retention as the number and unit boxes show it.
fn parts(r: Retention) -> (String, bool) {
    (
        r.amount().to_string(),
        matches!(r, Retention::CalendarYears(_)),
    )
}

#[component]
pub(super) fn RetentionInput(
    id: String,
    label: String,
    value: Retention,
    on_change: EventHandler<Retention>,
) -> Element {
    let (amount, years) = parts(value);
    rsx! {
        FormField { label, id: id.clone(),
            hint: if years { Some("Calendar years: three years back is a date three years earlier, not 1,095 days.".to_string()) } else { None },
            div { class: "ts-retention-input",
                input {
                    id: "{id}",
                    class: "input",
                    r#type: "number",
                    min: "1",
                    value: "{amount}",
                    oninput: move |e| {
                        if let Some(r) = retention(&e.value(), years) {
                            on_change.call(r);
                        }
                    },
                }
                select {
                    class: "select native",
                    "aria-label": "Unit",
                    onchange: move |e| {
                        let is_years = e.value() == "years";
                        on_change.call(if is_years { Retention::CalendarYears(value.amount()) } else { Retention::Days(value.amount()) });
                    },
                    option { value: "days", selected: !years, "days" }
                    option { value: "years", selected: years, "calendar years" }
                }
            }
        }
    }
}
