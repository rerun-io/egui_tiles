#!/usr/bin/env bash
# This scripts runs various CI-like checks in a convenient way.

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path"
set -x

export RUSTFLAGS="--deny warnings"
export RUSTDOCFLAGS="--deny warnings"

# `crates/kittest_inspector` and `crates/egui_mcp` are a workspace of their own.
# See the NOTE in Cargo.toml.
KITTEST_MANIFEST=crates/kittest_inspector/Cargo.toml

cargo fmt --all -- --check
cargo clippy --quiet --workspace --all-targets --all-features -- --deny warnings
cargo test --quiet --workspace --all-targets --all-features
cargo test --quiet --workspace --doc --all-features # checks all doc-tests

cargo doc --quiet --workspace --no-deps --all-features
cargo doc --quiet --workspace --document-private-items --no-deps --all-features

cargo deny --all-features --log-level error check
cargo shear # cargo install cargo-shear

cargo fmt --manifest-path $KITTEST_MANIFEST --all -- --check
cargo clippy --manifest-path $KITTEST_MANIFEST --quiet --workspace --all-targets --all-features -- --deny warnings
cargo test --manifest-path $KITTEST_MANIFEST --quiet --workspace --all-targets --all-features
cargo doc --manifest-path $KITTEST_MANIFEST --quiet --workspace --no-deps --all-features
cargo deny --manifest-path $KITTEST_MANIFEST --all-features --log-level error check
cargo shear crates/kittest_inspector

typos # cargo install typos-cli

echo "All checks passed!"
