# Release Checklist

* [ ] Update `CHANGELOG.md` using `./generate_changelog.py --commit-range 0.x..HEAD`
* [ ] Bump version numbers
* [ ] `git commit -m 'Release 0.x.0 - summary'`
* [ ] `cargo publish`
* [ ] `git tag -a 0.x.0 -m 'Release 0.x.0 - summary'`
* [ ] `git pull --tags && git tag -d latest && git tag -a latest -m 'Latest release' && git push --tags origin latest --force`
* [ ] `git push && git push --tags`
* [ ] Do a GitHub release: https://github.com/rerun-io/egui_tiles/releases/new
* [ ] Wait for documentation to build: https://docs.rs/releases/queue
* [ ] Post on Twitter
