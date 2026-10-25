//! The transcript: every command and its reply as terminal lines, with a
//! line-number gutter and the live prompt last.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::models::console::{ConsoleOutput, Protocol};
use crate::models::sql_text::tokenize;
use crate::state::console::ConsoleEntry;

pub const PROMPT: &str = "nodedb> ";
const CONTINUE: &str = "     \u{2026}> ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// A command line: its protocol and the text after the prompt.
    Command(Protocol, String),
    /// A continuation line of a multiline command.
    Continued(Protocol, String),
    Table(String),
    Note(String),
    Error(String),
    Blank,
}

/// `(1 row, 12 ms)`.
fn footer(rows: usize, ms: u32) -> String {
    let unit = if rows == 1 { "row" } else { "rows" };
    format!("({rows} {unit}, {ms} ms)")
}

/// A table drawn with box characters, one string per line.
pub fn draw_table(columns: &[String], rows: &[Vec<String>]) -> Vec<String> {
    let width = |i: usize| {
        rows.iter()
            .filter_map(|r| r.get(i))
            .map(|c| c.chars().count())
            .chain(columns.get(i).map(|c| c.chars().count()))
            .max()
            .unwrap_or(0)
    };
    let widths: Vec<usize> = (0..columns.len()).map(width).collect();
    let rule = format!(
        "\u{253c}{}\u{253c}",
        widths
            .iter()
            .map(|w| "\u{2500}".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("\u{253c}")
    );
    let line = |cells: &[String]| {
        let padded: Vec<String> = widths
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let cell = cells.get(i).map(String::as_str).unwrap_or("");
                format!(" {cell:<w$} ")
            })
            .collect();
        format!("\u{2502}{}\u{2502}", padded.join("\u{2502}"))
    };
    let mut out = vec![rule.clone(), line(columns), rule.clone()];
    for r in rows {
        out.push(line(r));
    }
    out.push(rule);
    out
}

/// The transcript as lines, in order. A blank line separates commands.
pub fn lines(entries: &[ConsoleEntry]) -> Vec<Line> {
    let mut out = Vec::new();
    for e in entries {
        let mut parts = e.text.lines();
        out.push(Line::Command(
            e.protocol,
            parts.next().unwrap_or("").to_string(),
        ));
        out.extend(parts.map(|p| Line::Continued(e.protocol, p.to_string())));
        match &e.outcome {
            Ok(reply) => match &reply.output {
                ConsoleOutput::Table { columns, rows } => {
                    out.extend(draw_table(columns, rows).into_iter().map(Line::Table));
                    out.push(Line::Note(footer(rows.len(), reply.elapsed_ms)));
                }
                ConsoleOutput::Text(text) => {
                    out.extend(text.iter().cloned().map(Line::Note));
                }
            },
            Err(msg) => out.push(Line::Error(format!("ERROR: {msg}"))),
        }
        out.push(Line::Blank);
    }
    out
}

#[component]
fn Colored(text: String, protocol: Protocol) -> Element {
    rsx! {
        for (i, t) in tokenize(&text, protocol).into_iter().enumerate() {
            span { key: "{i}", class: "{t.kind.class()}", "{t.text}" }
        }
    }
}

#[component]
pub fn Transcript(
    entries: Vec<ConsoleEntry>,
    protocol: Protocol,
    busy: bool,
    /// Set to the last line, so the page can keep it in view.
    end: Signal<Option<Rc<MountedData>>>,
) -> Element {
    let mut end = end;
    let all = lines(&entries);
    // The live prompt takes the line after the last one.
    let live = all.len() + 1;
    rsx! {
        div { class: "transcript", role: "log", "aria-label": "Transcript", "aria-live": "polite",
            div { class: "transcript-gutter", "aria-hidden": "true",
                for n in 1..=live {
                    span { key: "{n}", "{n}" }
                }
            }
            div { class: "transcript-lines",
                for (i, line) in all.into_iter().enumerate() {
                    div { key: "{i}", class: "term-line",
                        match line {
                            Line::Command(p, text) => rsx! {
                                span { class: "term-prompt", "{PROMPT}" }
                                Colored { text, protocol: p }
                            },
                            Line::Continued(p, text) => rsx! {
                                span { class: "term-prompt", "{CONTINUE}" }
                                Colored { text, protocol: p }
                            },
                            Line::Table(text) => rsx! { span { class: "term-table", "{text}" } },
                            Line::Note(text) => rsx! { span { class: "term-note", "{text}" } },
                            Line::Error(text) => rsx! { span { class: "term-error", "{text}" } },
                            Line::Blank => rsx! { "\u{a0}" },
                        }
                    }
                }
                div { class: "term-line", onmounted: move |e| end.set(Some(e.data())),
                    span { class: "term-prompt", "{PROMPT}" }
                    if busy {
                        span { class: "term-note", "Running\u{2026}" }
                    } else {
                        span { class: "term-caret", "aria-label": "{protocol.label()} prompt" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::console::ConsoleReply;
    use crate::models::workbench::TxState;

    fn entry(text: &str, outcome: Result<ConsoleReply, String>) -> ConsoleEntry {
        ConsoleEntry {
            id: 1,
            protocol: Protocol::Sql,
            text: text.into(),
            outcome,
        }
    }

    fn table(columns: &[&str], rows: &[&[&str]], ms: u32) -> ConsoleReply {
        ConsoleReply {
            output: ConsoleOutput::Table {
                columns: columns.iter().map(|c| c.to_string()).collect(),
                rows: rows
                    .iter()
                    .map(|r| r.iter().map(|c| c.to_string()).collect())
                    .collect(),
            },
            elapsed_ms: ms,
            tx: TxState::Idle,
        }
    }

    #[test]
    fn a_table_is_drawn_to_its_widest_cell() {
        let drawn = draw_table(
            &["current_database".to_string()],
            &[vec!["analytics".to_string()]],
        );
        assert_eq!(drawn.len(), 5);
        assert_eq!(drawn[1], "\u{2502} current_database \u{2502}");
        assert_eq!(drawn[3], "\u{2502} analytics        \u{2502}");
        let widths: Vec<usize> = drawn.iter().map(|l| l.chars().count()).collect();
        assert!(widths.windows(2).all(|w| w[0] == w[1]), "{drawn:?}");
    }

    #[test]
    fn a_command_with_rows_gets_a_footer_and_a_blank_line() {
        let l = lines(&[entry("SELECT 1;", Ok(table(&["a"], &[&["1"]], 12)))]);
        assert_eq!(l[0], Line::Command(Protocol::Sql, "SELECT 1;".into()));
        assert_eq!(l[l.len() - 2], Line::Note("(1 row, 12 ms)".into()));
        assert_eq!(l[l.len() - 1], Line::Blank);
        let two = lines(&[entry("q", Ok(table(&["a"], &[&["1"], &["2"]], 3)))]);
        assert!(two.contains(&Line::Note("(2 rows, 3 ms)".into())));
    }

    #[test]
    fn an_error_shows_its_message_and_a_multiline_command_continues() {
        let l = lines(&[entry("SELECT\n  1", Err("boom".into()))]);
        assert_eq!(l[1], Line::Continued(Protocol::Sql, "  1".into()));
        assert_eq!(l[2], Line::Error("ERROR: boom".into()));
    }
}
