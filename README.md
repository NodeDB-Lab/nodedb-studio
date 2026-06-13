# NodeDB Studio

**Desktop GUI client for [NodeDB](https://github.com/NodeDB-Lab/nodedb)** — a single-engine,
multi-modal database. Built with Dioxus + Rust. Studio is the visual surface for one NodeDB
instance, which exposes eight internal **storage modes** (Document, Strict, Vector, Graph,
Timeseries, KV, Spatial, FTS) — not separate engines. Studio renders a purpose-built UI per mode
within one connection.

## Status: UI skeleton

This repository is currently a **production-quality UI skeleton**: real Dioxus components,
routing, signal/context state, a faithful CSS port of the design, typed models, and hardcoded
mock data wired into the full interactive flow. There is **no network/database I/O yet** — every
backend touchpoint is a typed stub behind the `ConnectionService` seam (see below).

Implemented:

- Connection Manager → studio shell two-state flow
- Capability-driven rail and Admin sub-tabs (hide per the active connection's `Capabilities`)
- Engine-aware Explorer (collection click swaps the per-storage-mode viewer)
- Connection / database / notification / avatar popovers; per-connection identity
- Command palette (⌘K), Preferences (⌘,), Disconnect (⌘D), Esc — pure Rust, no JS interop
- Streams lateral nav, Admin sub-tabs with cluster gating, and all Phase-6 standalone views
- New-connection and Preferences modals
- Light/dark theme follows the OS

Heavy widgets (graph/chart/map renderers, code editor) are intentionally static SVG / `<pre>` /
`<textarea>` placeholders — the production library for each is a `// TODO` decision, not made here.

## Building & running

This is a Cargo workspace; the app is the `nodedb-studio` member crate.

```bash
cargo run -p nodedb-studio      # opens the desktop window (1440x900)
cargo build --release           # binary at target/release/nodedb-studio
```

### Local NodeDB dependency

The crate depends on `nodedb-client` / `nodedb-types`, pinned to a published version in the root
`Cargo.toml`. Until those crates are published, point them at a **local NodeDB checkout** with a
Cargo patch.

Create `.cargo/config.toml` at the workspace root (it is gitignored, so it stays machine-local and
never lands in the published manifest):

```toml
[patch.crates-io]
nodedb-client = { path = "../nodedb/nodedb-client" }
nodedb-types  = { path = "../nodedb/nodedb-types" }
```

Adjust the paths if your NodeDB checkout lives elsewhere, and make sure the version pinned in
`Cargo.toml` matches your local NodeDB workspace version (see its root `Cargo.toml`). (The skeleton
does not yet import these crates — they compile as dependencies only.)

## Project structure

```
nodedb-studio/                  workspace root
└── nodedb-studio/              the app crate
    ├── assets/styles.css       CSS ported verbatim from the design (tokens + classes)
    └── src/
        ├── main.rs             desktop launch + window config
        ├── app.rs              root: context providers + two-state machine
        ├── routes.rs           studio Route enum + StudioLayout (chrome + Outlet)
        ├── state/              live UI state (signals): connection, registry,
        │                       notifications, preferences, ui (popover/modal kinds)
        ├── models/             typed data: StorageMode + Collection, Notification
        ├── data/mock.rs        ALL hardcoded mock data, in one place
        ├── services/           ConnectionService trait + MockConnectionService
        ├── components/         shell chrome: rail, topbar, statusbar, command_palette,
        │                       modal, snav, subnav, popovers/
        ├── modals/             new_connection + preferences bodies + ModalHost
        └── views/              one screen per rail destination
            ├── connection_manager.rs, studio_shell.rs
            ├── explorer/       sidebar + viewers/ (one per storage mode)
            ├── streams/        landing + cdc/mv/topics/notify/cron
            ├── admin/          cluster/shards/nodes/raft/rbac/rls/audit
            └── query, designer, graph_explorer/, vector_space, sync,
                console, timeseries_dashboard, spatial_view, fts_inspector
```

## Where the backend plugs in

Everything the UI needs from the outside world goes through one trait:

`src/services/connection_service.rs` — `ConnectionService` (`list_connections`, `notifications`,
`connect`). Today the only implementor is `MockConnectionService`, reading `src/data/mock.rs`. It
is provided as `Rc<dyn ConnectionService>` via context in `app.rs`, so a real NodeDB-client-backed
implementor is a one-line swap. **Async** (`use_resource`) is introduced at this boundary when the
real client lands — not sprinkled through the views.

When richer per-database data is needed (e.g. an Explorer that lists a database's collections), add
the model under `src/models/` and surface it through `ConnectionService`.

## License

Apache-2.0. See [LICENSE](LICENSE) for details.
