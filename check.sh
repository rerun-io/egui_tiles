#!/usr/bin/env bash
# This scripts runs various CI-like checks in a convenient way.

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path"
set -x

export RUSTFLAGS="--deny warnings"

# https://github.com/ericseppanen/cargo-cranky/issues/8
export RUSTDOCFLAGS="--deny warnings --deny rustdoc::missing_crate_level_docs"

cargo fmt --all -- --check
cargo cranky --all-targets --all-features -- --deny warnings
cargo test --all-targets --all-features
cargo test --doc --all-features # checks all doc-tests

cargo doc --no-deps --all-features
cargo doc --document-private-items --no-deps --all-features

cargo deny --all-features --log-level error check

typos # cargo install typos-cli

echo "All checks passed!"
