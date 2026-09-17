# egui ecosystem crates

A monorepo for crates built on [egui](https://www.egui.rs/), and for the tools
that test them. Each crate has its own version and its own changelog.

| crate | what it is | published |
| --- | --- | --- |
| [`crates/egui_tiles`](crates/egui_tiles) | Tiling layout engine with drag-and-drop and resizing | [`egui_tiles`](https://crates.io/crates/egui_tiles) |
| [`crates/egui_table`](crates/egui_table) | Table viewer for millions of rows | [`egui_table`](https://crates.io/crates/egui_table) |
| [`crates/egui_table_demo`](crates/egui_table_demo) | Web demo for `egui_table` | no |
| [`crates/egui_mcp`](crates/egui_mcp) | MCP server that drives live egui apps | [`egui_mcp`](https://crates.io/crates/egui_mcp) |
| [`crates/kittest_inspector`](crates/kittest_inspector) | GUI to step through [kittest](https://github.com/rerun-io/kittest) tests frame by frame | no |

`egui_mcp` and `kittest_inspector` both speak the
[egui_inspection](https://github.com/emilk/egui/blob/main/crates/egui_inspection/README.md)
protocol. Only egui implements it today, but the idea is that other Rust UI
frameworks could use it through kittest and AccessKit.

## Building

One workspace holds all five crates:

```sh
./check.sh                 # everything CI checks
cargo test --workspace
cargo run -p demo          # the egui_table demo
```

`kittest_inspector` builds against the `lucas/kittest-inspect` branch of egui
while everything else uses the released egui, so the first build pulls two egui
trees. That is fine: cargo keeps both in the lockfile, and the two crates are
native-only — the wasm build skips them.

## Snapshots and git-LFS

The reference images for the snapshot tests, and the demo icons, are stored in
git-LFS. Install [git-lfs](https://git-lfs.com/) before cloning, or run
`git lfs pull` afterwards.

## Releasing

See [RELEASES.md](RELEASES.md).
