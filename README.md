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

## Two workspaces

The root workspace holds `egui_tiles`, `egui_table` and the demo.

`crates/kittest_inspector` and `crates/egui_mcp` are a second workspace, rooted
in `crates/kittest_inspector/Cargo.toml`. `kittest_inspector` builds against a
git branch of egui while `egui_mcp` uses the released egui, and cargo allows one
source per semver range per workspace. They rejoin the root workspace once both
sit on a released egui.

```sh
./check.sh                                     # everything CI checks, both workspaces
cargo test --workspace                         # root workspace
cargo test --manifest-path crates/kittest_inspector/Cargo.toml --workspace
```

## Snapshots and git-LFS

The reference images for the snapshot tests, and the demo icons, are stored in
git-LFS. Install [git-lfs](https://git-lfs.com/) before cloning, or run
`git lfs pull` afterwards.

## Releasing

See [RELEASES.md](RELEASES.md).
