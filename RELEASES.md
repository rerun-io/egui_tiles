# Release Checklist

* [ ] Update `CHANGELOG.md` using `./scripts/generate_changelog.py --version 0.NEW.VERSION`
* [ ] Bump version numbers in `Cargo.toml` and run `cargo check`.
* [ ] `git commit -m 'Release 0.x.0 - summary'`
* [ ] `cargo publish --quiet -p egui_tiles`
* [ ] `git tag -a 0.x.0 -m 'Release 0.x.0 - summary'`
* [ ] `git pull --tags ; git tag -d latest && git tag -a latest -m 'Latest release' && git push --tags origin latest --force ; git push --tags`
* [ ] Do a GitHub release: https://github.com/rerun-io/egui_tiles/releases/new
