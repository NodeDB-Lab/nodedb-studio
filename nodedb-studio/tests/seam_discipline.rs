//! Structural gate: views/components/modals must read data through the
//! backend seam (`services::backend::Backend`), never straight from
//! `data::mock`. Reaching past the seam is exactly the bug the seam exists
//! to prevent — a screen that "works" against mock data but silently breaks
//! (or never connects) once a real `Backend` impl lands.
//!
//! This is a plain filesystem/text scan, not a `syn`-based check: the crate
//! has no lib target (bin-only, see AGENTS.md), so an integration test here
//! cannot `use` crate items at all. Scanning source text is the only option
//! available at this layer, and it is enough to catch the pattern we care
//! about (`data::mock` / `mock::` reference-by-name).

use std::fs;
use std::path::{Path, PathBuf};

/// Directories (relative to the crate root) that must stay seam-only.
const SCANNED_ROOTS: &[&str] = &["src/views", "src/components", "src/modals"];

/// No exceptions. `views/streams/notify.rs` was the last one and is now
/// seam-backed. Anything added back here needs a reason that survives review.
const ALLOWED_EXCEPTIONS: &[&str] = &[];

/// Recursively collect every `.rs` file under `dir`.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Strip a trailing `//` line comment (if any) so matches inside comments do
/// not count as violations. This is a simple substring split, not a real
/// tokenizer — sufficient here because none of the scanned files put `//`
/// inside a string literal ahead of real code on the same line.
fn code_part(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn references_mock(line: &str) -> bool {
    let code = code_part(line);
    code.contains("data::mock") || code.contains("mock::")
}

#[test]
fn views_components_and_modals_read_only_through_the_seam() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut files = Vec::new();
    for root in SCANNED_ROOTS {
        collect_rs_files(&manifest_dir.join(root), &mut files);
    }
    assert!(
        !files.is_empty(),
        "expected to find .rs files under {SCANNED_ROOTS:?} — scan roots may be wrong"
    );

    let src_dir = manifest_dir.join("src");
    let mut violations: Vec<String> = Vec::new();

    for file in &files {
        let rel = file
            .strip_prefix(&src_dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");

        if ALLOWED_EXCEPTIONS.contains(&rel.as_str()) {
            continue;
        }

        let Ok(contents) = fs::read_to_string(file) else {
            continue;
        };
        for (idx, line) in contents.lines().enumerate() {
            if references_mock(line) {
                violations.push(format!("{rel}:{} : {}", idx + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "found {} reference(s) to data::mock outside the allowed exception \
         ({:?}) — views/components/modals must read through `services::backend::Backend`, \
         not `data::mock` directly:\n{}",
        violations.len(),
        ALLOWED_EXCEPTIONS,
        violations.join("\n")
    );
}

/// Every `connect()` call site must reconcile its result through
/// `apply_connect`, which hands the error back for rendering.
///
/// This exists because the previous fix looked complete and was not: the call
/// sites were changed from `if let Ok(..)` (drop the error) to
/// `tracing::error!` (log the error), which leaves the user-facing symptom
/// identical — a Connect button that does nothing, with no message and no
/// state change. A count comparison catches a fourth call site added later
/// that forgets to surface its failure.
#[test]
fn every_connect_call_site_reconciles_through_apply_connect() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut files = Vec::new();
    for root in SCANNED_ROOTS {
        collect_rs_files(&manifest_dir.join(root), &mut files);
    }

    let src_dir = manifest_dir.join("src");
    let mut total_calls = 0usize;
    let mut violations: Vec<String> = Vec::new();

    // Compared PER FILE, not as a global total: a single tally lets a file that
    // drops its result pass because another file happens to contribute a spare
    // apply_connect line.
    for file in &files {
        let rel = file
            .strip_prefix(&src_dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(contents) = fs::read_to_string(file) else {
            continue;
        };
        let mut calls = 0usize;
        let mut reconciles = 0usize;
        for line in contents.lines() {
            let code = code_part(line);
            if code.contains(".connect(") {
                calls += 1;
            }
            if code.contains("apply_connect(") {
                reconciles += 1;
            }
        }
        total_calls += calls;
        if calls != reconciles {
            violations.push(format!(
                "{rel}: {calls} connect() call site(s), {reconciles} apply_connect() reconcile(s)"
            ));
        }
    }

    assert!(
        total_calls > 0,
        "expected at least one connect() call site under {SCANNED_ROOTS:?} — scan may be wrong"
    );
    assert!(
        violations.is_empty(),
        "a connect result is being dropped or only logged, which renders as a button \
         that silently does nothing:\n{}",
        violations.join("\n")
    );
}

/// No scope-bound `spawn` in views, components or modals.
///
/// Dioxus drops a scope's tasks when the scope is removed, and most of this
/// tree is conditionally mounted: every popover, every modal, and the
/// Connection Manager itself, which is swapped for the studio shell the moment
/// a connect succeeds. A handler that spawns a seam call and then closes its
/// own popover kills the task at its first await. Nothing renders, nothing
/// errors, and the button reads as broken.
///
/// The mock cannot catch this. `apply_one` resolves on the first poll, so the
/// task finishes before the unmount can cancel it; only a backend that really
/// yields reaches the await. That is why this is a source rule rather than a
/// runtime test.
///
/// Seam writes use `spawn_forever` (root scope). Seam reads use `use_resource`
/// or `use_future`, which are tied to the component on purpose.
#[test]
fn views_components_and_modals_never_use_scope_bound_spawn() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut files = Vec::new();
    for root in SCANNED_ROOTS {
        collect_rs_files(&manifest_dir.join(root), &mut files);
    }

    let src_dir = manifest_dir.join("src");
    let mut violations: Vec<String> = Vec::new();

    for file in &files {
        let rel = file
            .strip_prefix(&src_dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(contents) = fs::read_to_string(file) else {
            continue;
        };
        for (idx, line) in contents.lines().enumerate() {
            let code = code_part(line);
            // `spawn_forever(` contains `spawn(`-adjacent text, so match the
            // bare call specifically.
            if code.contains("spawn(") && !code.contains("spawn_forever(") {
                violations.push(format!("{rel}:{} : {}", idx + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "found {} scope-bound spawn(s). A conditionally-mounted component that \
         spawns a seam call and then unmounts loses the task at its first await, \
         so the action silently does nothing. Use spawn_forever for writes, or \
         use_resource / use_future for reads:\n{}",
        violations.len(),
        violations.join("\n")
    );
}
