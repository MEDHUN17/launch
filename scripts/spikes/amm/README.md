# Spike A: Meteora DAMM v2 by CPI (Phase 0, task 0.4)

Throwaway code that answers the D-002 questions in `docs/DECISIONS.md`. It is not the Phase 4 `seed_pool`.

```bash
scripts/spikes/amm/run.sh      # devnet fixtures (hash-pinned) -> cargo build-sbf -> LiteSVM tests
```

What it runs, inside LiteSVM against the real devnet DAMM v2 binary and public config 11 (1% flat fee, fees in token B only):

- `seed_lock_swap_and_harvest_by_cpi_from_pdas`: one program instruction creates the pool and locks the position
  permanently, signed only by PDAs; a trader swaps both ways; a second instruction claims the fees as the PDA that
  holds the position NFT.
- `squatting_probe_one_sided_pool_without_holding_the_coin`: a third party who holds none of the coin creates the
  pool first, so the program's own creation fails (Q-02).
- `no_instruction_can_unlock_a_permanently_locked_position`: none of the 39 IDL instructions unlocks.

Provenance of `idls/cp_amm.json`: `npm pack @meteora-ag/cp-amm-sdk@1.5.1`, then
`python3 -I extract-idl.py pkg/package/dist/index.js idls/cp_amm.json`
(sha256 `52ad32666ee3665c417e8e617fcf7de7ac8d2435dfa6316c57bfc7343185e0ef`). Fixtures come from devnet only, never mainnet.
