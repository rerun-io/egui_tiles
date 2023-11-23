# `egui_tiles` Changelog


## [Unreleased](https://github.com/rerun-io/egui_tiles/compare/latest...HEAD)


## [0.4.0]](https://github.com/rerun-io/egui_tiles/compare/0.3.1...0.4.0) - 2023-11-23
* Fix Id clash when using multiple `Tree`s (#32)
* Scrollable tab bar (#9)
* `Behavior::on_tab_button` can now add context menus, on hover ui etc. (#23)
* `serde` is now and optional dependency (#13)
* Update to egui 0.24
* Update MSRV to Rust 1.72


## [0.3.1]](https://github.com/rerun-io/egui_tiles/compare/0.3.0...0.3.1) - 2023-09-29
* Report edits to user with `Behavior::on_edit` (#29)
* Make `Tree::simplify` public (#28)
* Add `Shares::set_share` method (#25)


## [0.3.0]](https://github.com/rerun-io/egui_tiles/compare/0.2.0...0.3.0) - 2023-09-28
* Update to egui 0.23
* Better grid column-count heuristic
* Make drag preview style customizable


## [0.2.0]](https://github.com/rerun-io/egui_tiles/compare/0.1.0...0.2.0) - Invisible tiles - 2023-07-06
* Add support for invisible tiles
* `PartialEq` for `Tiles` now ignores internal state
* Add `Tiles::find_pane`
* Add `Tiles::remove_recursively`


## 0.1.0 - Initial Release - 2023-05-24
