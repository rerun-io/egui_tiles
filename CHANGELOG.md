# `egui_tiles` Changelog


## [0.8.0](https://github.com/rerun-io/egui_tiles/compare/0.7.2...0.8.0) - 2024-03-26
* Re-export `Shares` [#56](https://github.com/rerun-io/egui_tiles/pull/56) (thanks [@Gohla](https://github.com/Gohla)!)
* Propagate `enabled` status for tile `Ui` [#55](https://github.com/rerun-io/egui_tiles/pull/55) (thanks [@Gohla](https://github.com/Gohla)!)
* Update to egui 0.27.0 [#58](https://github.com/rerun-io/egui_tiles/pull/58)


## [0.7.2](https://github.com/rerun-io/egui_tiles/compare/0.7.1...0.7.2) - 2024-02-07
* Fix `move_tile_to_container` behavior for grid-to-same-grid moves with reflow enabled [#53](https://github.com/rerun-io/egui_tiles/pull/53)


## [0.7.1](https://github.com/rerun-io/egui_tiles/compare/0.7.0...0.7.1) - 2024-02-06
* Make sure there is always an active tab [#50](https://github.com/rerun-io/egui_tiles/pull/50)
* Derive `Clone, Debug, PartialEq, Eq` for `EditAction` [#51](https://github.com/rerun-io/egui_tiles/pull/51)


## [0.7.0](https://github.com/rerun-io/egui_tiles/compare/0.6.0...0.7.0) - 2024-02-06
* Add an API to move an existing tile to an give container and position index [#44](https://github.com/rerun-io/egui_tiles/pull/44)
* Properly handle grid layout with `Tree::move_tile_to_container()` [#45](https://github.com/rerun-io/egui_tiles/pull/45)
* Turn some warn logging to debug logging [#47](https://github.com/rerun-io/egui_tiles/pull/47)
* Add an `EditAction` parameter to the `Behavior::on_edit()` call [#48](https://github.com/rerun-io/egui_tiles/pull/48)
* Update to `egui` 0.26 [#49](https://github.com/rerun-io/egui_tiles/pull/49)


## [0.6.0](https://github.com/rerun-io/egui_tiles/compare/0.5.0...0.6.0) - 2024-01-08
* Update to egui 0.25 [#43](https://github.com/rerun-io/egui_tiles/pull/43)


## [0.5.0](https://github.com/rerun-io/egui_tiles/compare/0.4.0...0.5.0) - 2024-01-04
* Pass `TileId` to `make_active` closure [#35](https://github.com/rerun-io/egui_tiles/pull/35)
* Add `SimplificationOptions::OFF` [#38](https://github.com/rerun-io/egui_tiles/pull/38)
* Add `Tree::simplify_children_of_tile` [#39) [#41](https://github.com/rerun-io/egui_tiles/pull/41)
* Expose the internal `u64` part of `TileId` [#40](https://github.com/rerun-io/egui_tiles/pull/40)
* Fix simplification errors that result in warnings after removing panes [#41](https://github.com/rerun-io/egui_tiles/pull/41)
* Add `Tree::active_tiles` for getting visible tiles [#42](https://github.com/rerun-io/egui_tiles/pull/42)


## [0.4.0](https://github.com/rerun-io/egui_tiles/compare/0.3.1...0.4.0) - 2023-11-23
* Fix Id clash when using multiple `Tree`s [#32](https://github.com/rerun-io/egui_tiles/pull/32)
* Scrollable tab bar [#9](https://github.com/rerun-io/egui_tiles/pull/9)
* `Behavior::on_tab_button` can now add context menus, on hover ui etc. [#23](https://github.com/rerun-io/egui_tiles/pull/23)
* `serde` is now and optional dependency [#13](https://github.com/rerun-io/egui_tiles/pull/13)
* Update to egui 0.24
* Update MSRV to Rust 1.72


## [0.3.1](https://github.com/rerun-io/egui_tiles/compare/0.3.0...0.3.1) - 2023-09-29
* Report edits to user with `Behavior::on_edit` [#29](https://github.com/rerun-io/egui_tiles/pull/29)
* Make `Tree::simplify` public [#28](https://github.com/rerun-io/egui_tiles/pull/28)
* Add `Shares::set_share` method [#25](https://github.com/rerun-io/egui_tiles/pull/25)


## [0.3.0](https://github.com/rerun-io/egui_tiles/compare/0.2.0...0.3.0) - 2023-09-28
* Update to egui 0.23
* Better grid column-count heuristic
* Make drag preview style customizable


## [0.2.0](https://github.com/rerun-io/egui_tiles/compare/0.1.0...0.2.0) - Invisible tiles - 2023-07-06
* Add support for invisible tiles
* `PartialEq` for `Tiles` now ignores internal state
* Add `Tiles::find_pane`
* Add `Tiles::remove_recursively`


## 0.1.0 - Initial Release - 2023-05-24
