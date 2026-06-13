# AGENTS.md

Contributor & AI-agent guide for **NodeDB Studio**. This is the single source of
truth for setup, commands, and conventions. (`CLAUDE.md` imports this file;
Cursor, Codex, Copilot, Gemini CLI, etc. read it directly.)

Keep this file accurate — if you change build/test commands or conventions,
update it in the same PR.

## What this is

NodeDB Studio is a **desktop GUI client for NodeDB**, built in Rust with
**Dioxus** (desktop). It is a separate repo from the NodeDB server.

- **Rust**: edition 2024, MSRV pinned in `Cargo.toml` (`rust-version`, currently 1.96).
- **UI**: Dioxus **0.7** with the `desktop` + `router` features.
- **Backend seam**: `nodedb-client` / `nodedb-types` (the typed Rust client).
  Today the app runs on hardcoded mock data behind a `ConnectionService` trait;
  the real client-backed impl plugs in at that seam.
- **Serialization**: `sonic_rs` for any runtime JSON (display), `serde` derive on
  models. The wire format to the server is the client's concern (MessagePack).

This is a Cargo workspace; the app is the `nodedb-studio` member crate.

## Setup — local NodeDB dependency (do this first)

`Cargo.toml` pins `nodedb-client` / `nodedb-types` to a published version, but
those crates are not on crates.io yet. Point them at a local NodeDB checkout with
a **gitignored** Cargo patch — never hardcode machine paths in `Cargo.toml`.

Create `.cargo/config.toml` at the workspace root (already gitignored):

```toml
[patch.crates-io]
nodedb-client = { path = "../nodedb/nodedb-client" }
nodedb-types  = { path = "../nodedb/nodedb-types" }
```

Adjust the paths to your checkout. The version pinned in `Cargo.toml` must match
your local NodeDB workspace version (see its root `Cargo.toml`). If Cargo says
your toolchain is too old, run `rustup update stable`.

## Commands

```bash
cargo run -p nodedb-studio      # launch the desktop app (1440x900 window)
dx serve                        # optional: run with hot-reload (needs `dioxus-cli`)
cargo build --release           # release binary at target/release/nodedb-studio

cargo fmt --all                 # format (CI gate runs `--check`; just run the plain form)
cargo clippy --workspace --all-targets --all-features -- -D warnings   # lints (warnings block merge)
cargo nextest run               # tests (use nextest, not `cargo test`)
cargo nextest run -E 'test(my_test_name)'                              # one test by name
```

Install the test runner once with `cargo install cargo-nextest --locked`.
There are no doctests today (the crate is a binary, no lib target), so
`cargo test --doc` is not part of the gate; add it if a library crate is introduced.

## Verify a change is done

Run these locally before pushing — they mirror CI and must all pass:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run
```

For UI changes, also `cargo run -p nodedb-studio` and confirm the screen renders.

## Project structure

```
nodedb-studio/src/
  main.rs        desktop launch + window config (Config / WindowBuilder)
  app.rs         root App: context providers + connected/disconnected state machine
  routes.rs      Route enum + StudioLayout (persistent chrome + Outlet)
  components/    reusable chrome (rail, topbar, statusbar, modal, popovers, …)
  views/         one module per routed screen (explorer, admin, streams, …)
  modals/        modal bodies + the ModalHost
  state/         live UI state as Dioxus signals (connection, prefs, ui, …)
  models/        typed domain data (collection, notification, …)
  services/      ConnectionService trait — the backend seam (mock impl today)
  data/mock.rs   ALL hardcoded mock data, in one place
assets/styles.css   CSS ported from the design (loaded via asset!)
```

## Code style & conventions

### Rust

- **No `.unwrap()` / `.expect()` / `panic!` in non-test code.** Use typed
  `thiserror` errors and propagate with `?`. Never `Result<T, String>`.
- **Module roots (`mod.rs`) contain only `pub mod` / `pub use`** — no logic, no
  type definitions. Put the module's code in a sibling file (e.g. `view.rs`).
- **Files stay under 500 lines** of non-test code — split by concern first.
- Use **`nodedb_types::Value`** for values that come from / go to the database.
- **`sonic_rs` for runtime JSON**, never `serde_json`. Carry data as native typed
  values; serialize to JSON only at the view boundary, for display.
- Naming: `UpperCamelCase` types, `snake_case` fns/modules. No `get_` prefix.
- Do **not** write `_ =>` catch-alls on exhaustive domain enums — let the compiler
  flag every site that needs updating.

### Dioxus 0.7

- Components are `PascalCase` `#[component]` functions returning `Element`.
- **State**: `use_signal` (signals are `Copy`). Subscribe with `.read()`; use
  **`.peek()`** in event handlers and when reading + writing the same signal.
  **Never hold a `.read()`/`.write()` guard across an `.await`.** Prefer
  `use_context`/`provide_context` over global statics.
- **Lists need stable keys** (`key: "{item.id}"`) — never the array index.
- Use inline format strings in attributes/text (`"{value}"`); avoid redundant
  closures over existing handlers.
- **Forms submit by default in 0.7** — call `e.prevent_default()` in `onsubmit`.
- **Never block the main thread.** CPU work → `std::thread::spawn`; async IO →
  `spawn` / `use_resource` / `use_action`; long-lived tasks → `spawn_forever`.
  Write results back into a signal.
- Keep business/state logic in plain Rust (`state/`, `services/`, `data/`) so it
  is testable without a renderer.

## Testing

- **Unit tests** inline: `#[cfg(test)] mod tests { … }` (can reach private items).
- **Integration tests** in `tests/`, public API only; shared helpers in
  `tests/common/mod.rs` (not `tests/common.rs`).
- Render-level checks: `dioxus_ssr::render_element(...)` to assert on output
  without a window.

## Git, commits & PRs

- **Conventional Commits**: `feat|fix|perf|refactor|test|docs|chore(scope): summary`.
  One logical change per commit.
- Branch from `main`; keep PRs small and focused (open an issue first for large
  changes). A PR must pass fmt, clippy, and nextest before review.

## Boundaries

**Always**
- Run fmt + clippy + nextest before pushing.
- Keep machine-specific paths in the gitignored `.cargo/config.toml`, not in `Cargo.toml`.

**Ask first**
- Adding a new dependency, or bumping the Dioxus / Rust version.
- Adding a new top-level module or changing the `ConnectionService` seam.

**Never**
- Commit secrets, credentials, or `.cargo/config.toml`.
- Use `.unwrap()`/`.expect()`/`panic!` in non-test code, or `serde_json` for JSON.
- Submit AI-generated code you have not read and understood.
