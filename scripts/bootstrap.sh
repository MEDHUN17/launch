#!/usr/bin/env bash
# Install the pinned Solana/Anchor toolchain. Idempotent: skips anything already at the pinned version.
# Pins are recorded in docs/DECISIONS.md (D-001). Localnet/devnet tooling only; nothing here touches mainnet.
#
#   scripts/bootstrap.sh                       # Agave + Anchor CLI + solana-verify
#   BOOTSTRAP_SKIP_VERIFY=1 scripts/bootstrap.sh   # CI: skip solana-verify (needed from Phase 9)
set -euo pipefail

AGAVE_VERSION="v4.3.2"          # `stable` channel resolved to solana-cli 4.3.2 (commit 0f131d3) on 2026-11-05
ANCHOR_VERSION="1.2.2"          # anchor-cli from crates.io; docs recommend AVM, which installs from GitHub
SOLANA_VERIFY_VERSION="0.5.2"   # from crates.io

export PATH="$HOME/.local/share/solana/install/active_release/bin:$HOME/.cargo/bin:$PATH"

if ! solana --version 2>/dev/null | grep -q "solana-cli ${AGAVE_VERSION#v} "; then
  echo ">> installing Agave ${AGAVE_VERSION}"
  sh -c "$(curl --proto '=https' --tlsv1.2 -sSfL "https://release.anza.xyz/${AGAVE_VERSION}/install")"
fi

if ! anchor --version 2>/dev/null | grep -q "anchor-cli ${ANCHOR_VERSION}\$"; then
  echo ">> installing anchor-cli ${ANCHOR_VERSION}"
  cargo install anchor-cli --version "${ANCHOR_VERSION}" --locked
fi

if [ -z "${BOOTSTRAP_SKIP_VERIFY:-}" ] && ! solana-verify --version 2>/dev/null | grep -q "${SOLANA_VERIFY_VERSION}"; then
  echo ">> installing solana-verify ${SOLANA_VERIFY_VERSION}"
  cargo install solana-verify --version "${SOLANA_VERIFY_VERSION}" --locked
fi

echo "--- toolchain ---"
solana --version
anchor --version
rustc --version
node --version 2>/dev/null || true
pnpm --version 2>/dev/null || true
[ -z "${BOOTSTRAP_SKIP_VERIFY:-}" ] && solana-verify --version || true
echo "Add to PATH:  export PATH=\"\$HOME/.local/share/solana/install/active_release/bin:\$HOME/.cargo/bin:\$PATH\""
