# Release Checklist

Each crate has its own version and its own `CHANGELOG.md`; there is no shared
version number. Release one crate at a time.

You decide the version and write the changelog. `release-plz` (see
`.github/workflows/release_plz.yml` and `release-plz.toml`) takes over once the
release commit is on `main`: it publishes to crates.io, pushes the tag and opens
the GitHub release, for every crate whose version is not on crates.io yet.

**Pick the version by hand, and treat an egui update as breaking.** These crates
put egui types in their public API, so a crate that compiles against egui 0.37
is a breaking change for everyone still on 0.36, even when nothing else changed.
For a `0.x` version that means bumping the minor: 0.17.1 → 0.18.0. No tool
catches this — `cargo-semver-checks` reads this crate's own rustdoc, where
`egui::Ui` looks the same before and after — which is why `semver_check` is off
and nothing proposes a version for you.

## Published crates

| crate | package | tag prefix |
| --- | --- | --- |
| `crates/egui_tiles` | `egui_tiles` | `egui_tiles-v` |
| `crates/egui_table` | `egui_table` | `egui_table-v` |
| `crates/egui_mcp` | `egui_mcp` | `egui_mcp-v` |

`crates/egui_table_demo` and `crates/kittest_inspector` are not published; both
set `publish = false`.

## Steps

Replace `CRATE` with the package name and `0.x.0` with the new version.

* [ ] Update `crates/CRATE/CHANGELOG.md` using `uv run scripts/generate_changelog.py --version 0.x.0`
* [ ] Bump `version` in `crates/CRATE/Cargo.toml` and run `cargo check`
* [ ] Open a PR titled `Release CRATE 0.x.0 - summary`, and merge it

Merging it is the whole release: the `Release-plz` workflow publishes the crate,
pushes the `CRATE-v0.x.0` tag and opens the GitHub release. Watch that run — if
it fails after `cargo publish` succeeded, re-running it is safe, because it skips
any version already on crates.io.

To do it by hand instead:

* [ ] `cargo publish --quiet -p CRATE`
* [ ] `git tag -a CRATE-v0.x.0 -m 'Release CRATE 0.x.0 - summary'`
* [ ] `git push --tags`
* [ ] Do a GitHub release: <https://github.com/rerun-io/egui_tiles/releases/new>

## History

The tags imported from the two merged repos keep their old, prefixed names:
`egui_table-0.1.0` … `egui_table-0.10.0` and `kittest_inspector-0.1.0` …
`kittest_inspector-0.2.0`. Tags from this repo's own history are unprefixed
(`0.1.0` … `0.17.1`). New tags use the `CRATE-vX.Y.Z` form above.
