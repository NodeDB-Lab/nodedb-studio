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
  Views reach the backend only through `Rc<dyn Backend>` (`services/backend.rs`),
  composed of per-domain async traits (`services/*_data.rs`). Today
  `services/mock/` serves hardcoded data; `services/nodedb/` is the client-backed
  stub that returns `StudioError::NotConnected` until wired.
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
scripts/screenshots.sh          # capture every screen, light + dark, to PNG (Linux + Xvfb)

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

For UI changes, also check the screen renders: `cargo run -p nodedb-studio`, or
`scripts/screenshots.sh` and read the PNGs. A new screen or overlay needs a scene in
`src/screenshots/scenes.rs`. Launch through `cargo run`: a bare debug or release
binary counts as a `dx` bundle and loads no stylesheet. Scenes live in
`src/screenshots/scenes.rs` and the per-area `*_scenes.rs` files.

## Project structure

```
nodedb-studio/src/
  main.rs        desktop launch + window config (Config / WindowBuilder)
  app.rs         root App: context providers + connected/disconnected state machine
  routes/        Route enum, typed tab segments (tabs.rs), sidebar destinations, StudioLayout
  components/    reusable chrome (sidebar, header, footer, panel, tabs, view menu, popovers, …)
  views/         one module per routed screen (explorer, query, events, cluster, admin, …)
  modals/        modal bodies + the ModalHost
  state/         live UI state as Dioxus signals; workspace panel layout + persistence
  models/        typed domain data
  services/      Backend seam: per-domain traits, mock/ impl, nodedb/ stub
  data/mock/     ALL hardcoded mock data, in one place
  screenshots/   scene runner + WebKitGTK capture (`screenshots` feature, Linux only)
assets/             tokens.css (design tokens, light + dark), shared sheets, one sheet per area
scripts/            screenshot capture + PR diff report (CI: .github/workflows/screenshots.yml)
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
  `use_resource` / `use_action`; event-handler tasks → `spawn_forever` (never
  scope-bound `spawn(` in views, components, or modals).
- Views, components, and modals read data only through the `Backend` seam, never
  `data::mock` or `services::mock`; connect through `apply_connect`.
  `tests/seam_discipline.rs` enforces both.
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
- Adding a new top-level module or changing the `Backend` seam's shape.

**Never**
- Commit secrets, credentials, or `.cargo/config.toml`.
- Use `.unwrap()`/`.expect()`/`panic!` in non-test code, or `serde_json` for JSON.
- Submit AI-generated code you have not read and understood.
