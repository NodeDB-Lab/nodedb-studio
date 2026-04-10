# NodeDB Studio

**GUI client for [NodeDB](https://github.com/NodeDB-Lab/nodedb).** Query editor, data browser, and administration -- built with Dioxus for desktop and web.

## Features (Planned)

- **Query editor** -- SQL editor with syntax highlighting, auto-complete, and query history
- **Data browser** -- Browse, filter, and edit documents, vectors, graph nodes, timeseries, and KV pairs
- **Graph explorer** -- Interactive visualization of graph relationships and traversal results
- **Vector space viewer** -- 2D/3D projection of vector collections with cluster visualization
- **Collection management** -- Create, alter, drop, and inspect collections across all engines
- **Real-time monitor** -- Live metrics, active queries, CDC streams, and resource usage
- **Sync dashboard** -- CRDT sync status, pending deltas, conflict resolution, device overview
- **Multi-connection** -- Connect to multiple NodeDB instances and switch between them

## Install

```bash
cargo install nodedb-studio
```

Pre-built binaries for Linux, macOS, and Windows will be available on the [releases page](https://github.com/NodeDB-Lab/nodedb-studio/releases).

## Building from Source

```bash
git clone https://github.com/NodeDB-Lab/nodedb-studio.git
cd nodedb-studio
cargo build --release
```

The binary is at `target/release/nodedb-studio`.

For local development against the NodeDB workspace, create `.cargo/config.toml`:

```toml
[patch.crates-io]
nodedb-client = { path = "../nodedb/nodedb-client" }
nodedb-types = { path = "../nodedb/nodedb-types" }
```

## Status

Early development. Not yet usable.

## License

Apache-2.0. See [LICENSE](LICENSE) for details.
