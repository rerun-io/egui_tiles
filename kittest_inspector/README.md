# kittest_inspector

Live inspector eframe app for [`egui_kittest`](https://github.com/emilk/egui/tree/main/crates/egui_kittest) tests.

When your harness is attached, the inspector shows every rendered frame, the full
accesskit tree, the test source with step highlighting, and Play / Pause / Next
controls for stepping through the run. You can also switch into **Control** mode
and drive the tested app yourself — pointer & keyboard events are captured and
forwarded to the harness on the next step.

## Install

```sh
cargo install --git https://github.com/rerun-io/kittest_inspector
```

This puts `kittest_inspector` on your `PATH`. The harness launches it as a child
process and talks to it over stdin/stdout.

## Use

In a test that uses `egui_kittest` with the `inspector` feature:

```sh
KITTEST_INSPECTOR=1 cargo test my_test -- --nocapture
```

Or call `.with_inspector()` on your `Harness` to opt in programmatically.

### Env vars

- `KITTEST_INSPECTOR=1` — auto-launch an inspector for every harness in this process.
- `KITTEST_INSPECTOR_PATH=/path/to/kittest_inspector` — override the binary lookup
  (by default the harness looks for `kittest_inspector` on `PATH`).

## Crate layout

- **Library** (`kittest_inspector`) — wire protocol types (`Frame`, `HarnessMessage`,
  `InspectorReply`) used by both the harness and the app. Small, no GUI deps.
- **Binary** (`kittest_inspector`, gated behind the default `app` feature) — the
  eframe window.

`egui_kittest` depends on this crate with `default-features = false` so it only
pulls in the protocol types.
