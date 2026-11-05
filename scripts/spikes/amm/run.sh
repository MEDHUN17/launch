#!/usr/bin/env bash
# Run spike A end to end: fixtures from devnet (hash-pinned), build the spike program, run the LiteSVM tests.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.local/share/solana/install/active_release/bin:$HOME/.cargo/bin:$PATH"
./fetch-fixtures.sh
cargo build-sbf --manifest-path program/Cargo.toml
cargo test --manifest-path program/Cargo.toml -- --nocapture --test-threads=1
