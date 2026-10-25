//! Page controls: previous and next, numbered pages with gaps, and the page
//! size. Pages are zero-based in code and one-based on screen.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};

/// One slot of the page strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Page(usize),
    Gap,
}

/// The slots for `page` of `pages`: the first page, a window around the
/// current one, and the last page, with a gap where pages are skipped.
pub fn slots(page: usize, pages: usize) -> Vec<Slot> {
    if pages <= 7 {
        return (0..pages).map(Slot::Page).collect();
    }
    let last = pages - 1;
    let (from, to) = if page < 4 {
        (1, 4)
    } else if page + 4 > last {
        (last - 4, last - 1)
    } else {
        (page - 1, page + 1)
    };
    let mut out = vec![Slot::Page(0)];
    if from > 1 {
        out.push(Slot::Gap);
    }
    out.extend((from..=to).map(Slot::Page));
    if to < last - 1 {
        out.push(Slot::Gap);
    }
    out.push(Slot::Page(last));
    out
}

#[component]
pub fn Pager(page: usize, pages: usize, on_page: EventHandler<usize>) -> Element {
    let last = pages.saturating_sub(1);
    rsx! {
        nav { class: "pager", "aria-label": "Pages",
            button {
                class: "pager-btn",
                disabled: page == 0,
                "aria-label": "Previous page",
                onclick: move |_| on_page.call(page.saturating_sub(1)),
                Ico { icon: Icon::ChevronLeft, size: 16 }
            }
            for (i, slot) in slots(page, pages).into_iter().enumerate() {
                match slot {
                    Slot::Page(p) => rsx! {
                        button {
                            key: "p{p}",
                            class: if p == page { "pager-btn current" } else { "pager-btn" },
                            "aria-current": if p == page { "page" } else { "false" },
                            onclick: move |_| on_page.call(p),
                            "{p + 1}"
                        }
                    },
                    Slot::Gap => rsx! { span { key: "gap{i}", class: "pager-gap", "…" } },
                }
            }
            button {
                class: "pager-btn",
                disabled: page >= last,
                "aria-label": "Next page",
                onclick: move |_| on_page.call((page + 1).min(last)),
                Ico { icon: Icon::ChevronRight, size: 16 }
            }
        }
    }
}

/// The page size choices.
pub const PAGE_SIZES: [usize; 5] = [5, 10, 20, 50, 100];

#[component]
pub fn RowsPerPage(value: usize, on_change: EventHandler<usize>) -> Element {
    // A schema default outside the list still shows as chosen.
    let mut sizes: Vec<usize> = PAGE_SIZES.to_vec();
    if !sizes.contains(&value) {
        sizes.push(value);
        sizes.sort_unstable();
    }
    rsx! {
        label { class: "rows-per-page",
            select {
                class: "select native compact",
                "aria-label": "Rows per page",
                onchange: move |e| {
                    if let Ok(n) = e.value().parse::<usize>() {
                        on_change.call(n);
                    }
                },
                for n in sizes {
                    option { key: "{n}", value: "{n}", selected: n == value, "{n}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(page: usize, pages: usize) -> String {
        slots(page, pages)
            .iter()
            .map(|s| match s {
                Slot::Page(p) => (p + 1).to_string(),
                Slot::Gap => "…".to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn few_pages_show_every_page() {
        assert_eq!(labels(0, 3), "1 2 3");
        assert_eq!(labels(0, 0), "");
        assert_eq!(labels(2, 7), "1 2 3 4 5 6 7");
    }

    #[test]
    fn many_pages_keep_the_ends_and_a_window() {
        assert_eq!(labels(0, 257), "1 2 3 4 5 … 257");
        assert_eq!(labels(3, 257), "1 2 3 4 5 … 257");
        assert_eq!(labels(10, 257), "1 … 10 11 12 … 257");
        assert_eq!(labels(255, 257), "1 … 253 254 255 256 257");
        assert_eq!(labels(256, 257), "1 … 253 254 255 256 257");
    }

    #[test]
    fn the_current_page_is_always_in_the_strip() {
        for pages in [8, 11, 40] {
            for page in 0..pages {
                assert!(
                    slots(page, pages).contains(&Slot::Page(page)),
                    "{page}/{pages}"
                );
            }
        }
    }
}
