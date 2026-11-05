#!/usr/bin/env bash
# Count result values across recent settled non-combo Kalshi markets (public, read-only endpoint).
# This produced the numbers in docs/DECISIONS.md D-003 (6,000 markets on 2026-11-05: 3,400 no, 2,538 yes, 62 scalar).
#   scripts/spikes/kalshi/scan-settled.sh [pages]      (default 6 pages of 1,000)
set -euo pipefail
B="${KALSHI_BASE:-https://external-api.kalshi.com/trade-api/v2}"; PAGES="${1:-6}"
out="$(mktemp)"; cursor=""
for page in $(seq 1 "$PAGES"); do
  url="$B/markets?status=settled&mve_filter=exclude&limit=1000${cursor:+&cursor=$cursor}"
  for try in 1 2 3 4 5; do
    code=$(curl -sS -m 60 -A prequel-phase0-spike "$url" -o "$out.page" -w "%{http_code}") && [ "$code" = "200" ] && break
    sleep $((try * 4))
  done
  [ "$code" = "200" ] || { echo "page $page failed ($code)" >&2; break; }
  jq -c '.markets[]' "$out.page" >> "$out"
  cursor=$(jq -r '.cursor // ""' "$out.page"); [ -z "$cursor" ] && break
done
echo "markets: $(wc -l < "$out")"
jq -r '[.status, (.result | if . == "" then "(empty)" else . end)] | @tsv' "$out" | sort | uniq -c | sort -rn
echo "sample non yes/no:"; jq -c 'select(.result != "yes" and .result != "no") | {ticker, result, settlement_value_dollars}' "$out" | head -5
rm -f "$out" "$out.page"
