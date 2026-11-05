#!/usr/bin/env bash
# Dump the Meteora DAMM v2 program and public config 11 from DEVNET (never mainnet) into ./fixtures and verify
# them against the hashes pinned below. A hash mismatch means the devnet program or config changed:
# review the change before updating the pin (docs/DECISIONS.md D-002).
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.local/share/solana/install/active_release/bin:$HOME/.cargo/bin:$PATH"
DAMM=cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG
CFG11=AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW
PIN_DAMM_SO=82bb9375921bb8007551cb65f9ca43b191597496cc9922926468b36671081ec2   # dumped 2026-11-05
PIN_CFG11_DATA=dbe676a24602fca00b854731ade471e01419a52f87b18c671e18306df551a6a9                                                          # sha256 of the account data bytes
mkdir -p fixtures
solana program dump "$DAMM" fixtures/damm_v2.so -u devnet >/dev/null
solana account "$CFG11" -u devnet --output json-compact -o fixtures/config11.json >/dev/null
got_so=$(sha256sum fixtures/damm_v2.so | cut -d' ' -f1)
got_cfg=$(jq -r '.account.data[0]' fixtures/config11.json | base64 -d | sha256sum | cut -d' ' -f1)
[ "$got_so" = "$PIN_DAMM_SO" ] || { echo "damm_v2.so hash changed: $got_so" >&2; exit 1; }
[ "$got_cfg" = "$PIN_CFG11_DATA" ] || { echo "config11 data hash changed: $got_cfg" >&2; exit 1; }
echo "fixtures OK (damm_v2.so $got_so, config11 data $got_cfg)"
