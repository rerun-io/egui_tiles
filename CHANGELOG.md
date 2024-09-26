# `egui_tiles` Changelog


## 0.10.0 - 2024-09-26
* Update to egui 0.29 [#78](https://github.com/rerun-io/egui_tiles/pull/78)
* Add `Tree::set_width` and `set_height` functions [#73](https://github.com/rerun-io/egui_tiles/pull/73) (thanks [@rafaga](https://github.com/rafaga)!)
* Fix for eagerly starting a drag when clicking tab background [#80](https://github.com/rerun-io/egui_tiles/pull/80)
* Fix `Tree` deserialization using JSON [#85](https://github.com/rerun-io/egui_tiles/pull/85) (thanks [@hastri](https://github.com/hastri)!)


## [0.9.1](https://github.com/rerun-io/egui_tiles/compare/0.9.0...0.9.1) - 2024-08-27
* Add `Tree::set_width` and `set_height` functions [#73](https://github.com/rerun-io/egui_tiles/pull/73) (thanks [@rafaga](https://github.com/rafaga)!)
* Fix for eagerly starting a drag when clicking tab background [#80](https://github.com/rerun-io/egui_tiles/pull/80)


## [0.9.0](https://github.com/rerun-io/egui_tiles/compare/0.8.0...0.9.0) - 2024-07-03 - egui 0.28 and tab close buttons
Full diff at https://github.com/rerun-io/egui_tiles/compare/0.8.0..HEAD

* Update to egui 0.28.0 [#67](https://github.com/rerun-io/egui_tiles/pull/67)
* Update to Rust 1.76 [#60](https://github.com/rerun-io/egui_tiles/pull/60) [#66](https://github.com/rerun-io/egui_tiles/pull/66)
* Optional close-buttons on tabs [#70](https://github.com/rerun-io/egui_tiles/pull/70) (thanks [@voidburn](https://github.com/voidburn)!)
* Add `Tiles::rect` to read where a tile is [#61](https://github.com/rerun-io/egui_tiles/pull/61)
* Add `Behavior::paint_on_top_of_tile` [#62](https://github.com/rerun-io/egui_tiles/pull/62)
* Fix: make sure `Tree::ui` allocates the space it uses in parent `Ui` [#71](https://github.com/rerun-io/egui_tiles/pull/71) (thanks [@rydb](https://github.com/rydb)!)
* Fix bugs when having multiple `Tree`s visible at the same time [#68](https://github.com/rerun-io/egui_tiles/pull/68) (thanks [@GuillaumeSchmid](https://github.com/GuillaumeSchmid)!)
* Fix drag-and-drop of tiles on touchscreen devices [#74](https://github.com/rerun-io/egui_tiles/pull/74) (thanks [@mcoroz](https://github.com/mcoroz)!)
* Fix container resize drag for touchscreens [#75](https://github.com/rerun-io/egui_tiles/pull/75) (thanks [@mcoroz](https://github.com/mcoroz)!)
* Update release instructions [62ecb4c](https://github.com/rerun-io/egui_tiles/commit/62ecb4ccd52bdabd11e688e4e6e29e4d1a3783ab)
* Add clippy lint `match_bool` [fadf41a](https://github.com/rerun-io/egui_tiles/commit/fadf41ab42af5527e8a17af436a5608dd7dbd7bf)
* Add a PR template [87110a9](https://github.com/rerun-io/egui_tiles/commit/87110a98a280f73c77b80507367290691f75d33b)
* Expose `egui_tiles::TabState` [6e88ea9](https://github.com/rerun-io/egui_tiles/commit/6e88ea9774d63b0a7a8a67af9a90c13a4b3efb10)
* Pass `&TabState` to all relevant functions in Behavior [ee1286a](https://github.com/rerun-io/egui_tiles/commit/ee1286a975239ffa34258313a11d2bf03ec4cea9)


## [0.8.0](https://github.com/rerun-io/egui_tiles/compare/0.7.2...0.8.0) - 2024-03-26
* Update to egui 0.27.0 [#58](https://github.com/rerun-io/egui_tiles/pull/58)
* Re-export `Shares` [#56](https://github.com/rerun-io/egui_tiles/pull/56) (thanks [@Gohla](https://github.com/Gohla)!)
* Propagate `enabled` status for tile `Ui` [#55](https://github.com/rerun-io/egui_tiles/pull/55) (thanks [@Gohla](https://github.com/Gohla)!)


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
* `Behavior::on_tab_button` can now add context menus, on hover ui etc [#23](https://github.com/rerun-io/egui_tiles/pull/23)
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
