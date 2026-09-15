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
    let mut calls = 0usize;
    let mut reconciles = 0usize;
    let mut sites: Vec<String> = Vec::new();

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
            if code.contains(".connect(") {
                calls += 1;
                sites.push(format!("{rel}:{} : {}", idx + 1, line.trim()));
            }
            if code.contains("apply_connect(") {
                reconciles += 1;
            }
        }
    }

    assert!(
        calls > 0,
        "expected at least one connect() call site under {SCANNED_ROOTS:?} — scan may be wrong"
    );
    assert_eq!(
        calls,
        reconciles,
        "{calls} connect() call site(s) but {reconciles} apply_connect() reconcile(s) — \
         a connect result is being dropped or only logged, which renders as a button \
         that silently does nothing:\n{}",
        sites.join("\n")
    );
}
