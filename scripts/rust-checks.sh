#!/usr/bin/env bash
# Build the program and run the Rust checks CI runs. Usage: scripts/rust-checks.sh
# Assumes the Agave toolchain and Anchor CLI are on PATH (see scripts/bootstrap.sh).
# `anchor build` comes before clippy/test because the LiteSVM tests embed target/deploy/prequel.so.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
anchor build
cargo clippy --all-targets -- -D warnings
cargo test
