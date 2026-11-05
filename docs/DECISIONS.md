# DECISIONS

One entry per decision. A version or doc link goes in only after it was read from the official source, with the date. Nothing here changes SPEC.md until the user approves it.

- **Status:** `Decided`, `Provisional` (chosen, waiting for a named confirmation), `Pending` (phase says when), `Inconclusive`.
- **Evidence tags:** **[V-me]** I ran or read it myself on 2026-11-05. **[V-rev]** verified by an independent Opus reviewer from official docs or devnet and not re-checked by me. **[U]** unverified or from memory.
- **Section B is not decisions.** It lists gaps and risks in SPEC.md found by me and by two independent Opus reviews. Each needs the user's answer before the phase that depends on it.

## Index

| ID | Title | Phase | Status |
| --- | --- | --- | --- |
| D-001 | Toolchain pins | 0 | Decided |
| D-002 | AMM: Meteora DAMM v2 vs Raydium CPMM | 0 | **Provisional: Meteora DAMM v2, only with an atomic launch (Q-02)** |
| D-003 | Kalshi ingestion and API terms | 0 | Decided (terms review still open) |
| D-004 | DFlow on-chain settlement | 0 | Inconclusive (research only) |
| D-005 | Media storage | 0 | Provisional: IPFS via Pinata |
| D-006 | Token program | 0 | Decided: classic SPL |
| D-007 | Program test stack | 0 | Decided: LiteSVM |
| D-008 | Client SDK generator | 5 | Pending |
| D-009 | Shared canon constants | 1 | Pending |
| D-010 | Vault and payer account model | 1 | Pending |
| D-011 | Dust rule | 3 | Pending |
| D-012 | RPC and webhook provider | 5 | Pending |
| Q-01 to Q-20 | Spec questions | various | Open, section B |

## A. Decisions

### D-001 · Toolchain pins

- **Phase 0 · Decided 2026-11-05.** Everything below is installed in the sandbox and exercised by `scripts/bootstrap.sh` and `scripts/rust-checks.sh`.

| Tool | Pin | Source and notes |
| --- | --- | --- |
| Agave (Solana CLI) | v4.3.2 (commit 0f131d3, the `stable` channel today) | Official installer per https://docs.anza.xyz/cli/install. That page's example shows v4.4.0; `stable` resolved to 4.3.2. `cargo-build-sbf` pulled platform-tools v1.57 on first build. |
| Anchor CLI and crates | 1.2.2 | `cargo install anchor-cli --version 1.2.2 --locked` from crates.io. The docs (https://www.anchor-lang.com/docs/installation) recommend AVM installed from GitHub (`otter-sec/anchor`), which the sandbox cannot reach. crates.io lists `otter-sec/anchor` as the repository. |
| Rust (host) | 1.89.0 via `rust-toolchain.toml` | From the Anchor 1.2.2 template; its lockfile pins a Rust 1.89 compatible LiteSVM graph. The sandbox default is 1.97.0. |
| LiteSVM | crate 0.15.0 | Template pin. 0.18.2 exists; bump when the Rust pin moves. |
| solana-verify | 0.5.2 | crates.io; needs Docker (none in the sandbox), first used in Phase 9/10. |
| Node, pnpm | 22.23.4 (`.nvmrc`), pnpm 10.28.0 | pnpm 12.16.1 is available and not adopted. |
| Turborepo | 2.11.10 | |
| TypeScript | **6.0.3** | 7.0.2 is the latest, but typescript-eslint 8.72.1 declares `typescript >=4.8.4 <6.1.0`. Revisit when it supports 7. |
| ESLint stack | eslint 10.14.0, @eslint/js 10.0.1, typescript-eslint 8.72.1, globals 17.14.0 | |
| Prettier, Vitest, Vite | 3.9.10, 5.1.0, 8.3.6 | Vite is Vitest's peer. |
| Next.js, React | 16.4.5, 19.3.1 | Minimal App Router stub builds. |
| GitHub Actions | `actions/checkout@v4`, `setup-node@v4`, `pnpm/action-setup@v4`, `cache@v4` | **[U]** Newer majors may exist; the sandbox cannot read GitHub APIs outside this repo. The first remote run is the check. |

- **Sandbox quirks:** Node's built-in `fetch` ignores the proxy, so export `NODE_USE_ENV_PROXY=1`; put `~/.local/share/solana/install/active_release/bin` on `PATH`. `anchor build` warns that the keypair and `declare_id!` differ; program keypairs are never committed, so the real id is set in Phase 10.
- **Dev dependencies added beyond SPEC §4** (approved with the Phase 0 go-ahead): vitest, vite, eslint stack, prettier, `@types/node`; in the throwaway AMM spike only: `bytemuck`, `serde_json`, `base64`, `solana-account`.

### D-002 · AMM: Meteora DAMM v2 vs Raydium CPMM

- **Phase 0 · Status: Provisional.** Chosen: **Meteora DAMM v2, public config 11, launched atomically.** It needs (1) the user's approval of the atomic-launch amendment (Q-02), (2) the pending atomic-launch probe, (3) a check of Meteora's mainnet upgrade authority **[U]**.
- **What I verified myself [V-me], on devnet and in LiteSVM with the real devnet binary** (`scripts/spikes/amm`, `scripts/spikes/amm/run.sh`, 3 tests pass):
  - Program `cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG` is deployed on devnet (1,559,448 bytes, sha256 `82bb9375…1ec2`, hash-pinned in `fetch-fixtures.sh`).
  - Config 11 `AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW`, decoded: flat 1.00% fee (cliff_fee_numerator 10,000,000 of 1e9), no dynamic fee, protocol cut 20%, `collect_fee_mode = 1` (token B only), full price range, `pool_creator_authority` empty (public).
  - IDL extracted from npm `@meteora-ag/cp-amm-sdk@1.5.1` (cp_amm 0.2.5, 39 instructions, none unlocks), sha256 `52ad3266…e0ef`. Anchor 1.2.2 `declare_program!` compiles from it (needs a `bytemuck` dependency).
  - **Pool creation and permanent lock in one program instruction, signed only by PDAs:** 121,988 CU, 22 unique accounts, 811-byte legacy message, 22.6M lamports of rent (LiteSVM default rent; mainnet rent is falling under SIMD-0437 [V-rev]). A system-owned `seeder` PDA pays and owns the deposit accounts; a separate PDA holds the position NFT.
  - **Deposits at a 5 SOL raise:** all 497,487,437,185,929 pool tokens and 4,949,999,635 of 4,950,000,000 lamports (365 lamports of dust); pool price within 7.4e-8 of the clearing price. At 50 and 500 SOL the SOL dust is 1,155 and 3,650 lamports (offline calculation; the agent probe re-runs these).
  - After the lock: `unlocked_liquidity = 0`, `permanent_locked_liquidity` = the whole position. No IDL instruction unlocks.
  - **Fee claim by CPI as the NFT-holding PDA:** 30,181 CU. Fees arrive in wSOL only (token A fees = 0). The LP's share measured 0.8028% of volume: the 1% fee less the 20% protocol cut.
  - A 1 SOL buy into a 4.95 SOL pool averaged 1.206e-5 vs the 9.95e-6 clearing price (slippage plus 1% fee). This is the "exit math" in Q-15.
  - **Squatting works without holding the coin:** a third party with only 0.5 wSOL and an empty token account created the (coin, wSOL) pool on config 11 by pricing it at the top of the range (zero token A needed). Prequel's own creation for the same address then fails ("account already in use"). Gating claims is not enough; see Q-02.
- **Reviewer findings [V-rev]** (docs, npm bundles and devnet accounts; full table in the review notes under Q-02/Q-12):
  - Raydium CPMM charges a 0.15 SOL pool creation fee (admin-changeable), pays LP fees in both tokens, and needs a second closed-source lock program (LP Lock) with a different IDL format. Its crate pins Anchor 1.0.2, which conflicts with 1.2.2. Program ids differ between devnet and mainnet.
  - Raydium lets the creator choose the pool address (so a PDA of ours cannot be squatted); Meteora derives it from config and mints.
  - Meteora: operator fee changes are capped at 10% per pool; protocol share fixed at 20%; operator `set_pool_status` can disable swap/liquidity paths (whether claims stay open is **[U]**).
  - Raydium's admin can change the 1% tier's rates for every pool on it, and pause pools.
- **Unverified [U]:** Meteora mainnet upgrade authority and governance; whether a paused pool still allows fee claims; Jupiter and DexScreener display (Jupiter's docs say both AMMs are eligible for routing); compute for Raydium; same-instruction mint creation and pre-funded addresses (agent probe running).
- **What would flip this to Raydium CPMM:** the atomic-launch probe fails; or Meteora's upgrade authority is a single key; or birth and seeding cannot be made atomic.
- **Phase 0 done-when for the AMM spike** ("create a pool, lock the LP and claim fees by CPI on localnet") is met for Meteora. Raydium was compared from docs and devnet reads only, not run.

### D-003 · Kalshi ingestion and API terms

- **Phase 0 · Decided 2026-11-05, with the terms review still open.** Spike: `scripts/spikes/kalshi/spike.ts` (`pnpm spike:kalshi`, needs `NODE_USE_ENV_PROXY=1` here), run live against production read-only endpoints.
- **Sources [V-me]:** https://docs.kalshi.com/getting_started/api_environments.md, `market_lifecycle.md`, `market_settlement.md`, `quick_start_market_data.md`, `rate_limits.md` and the OpenAPI for `get-markets` and `get-market`, read 2026-11-05.
- **Decision:** v0 reads the **unauthenticated** public market-data endpoints at `https://external-api.kalshi.com/trade-api/v2` (also supported: `api.elections.kalshi.com`; demo `external-api.demo.kalshi.co`). No API key is needed to list markets or read a settled one.
- **Facts the design relies on:**
  - Market statuses: `initialized`, `active`, `inactive`, `closed`, `determined`, `disputed`, `amended`, `finalized` (terminal). While `determined`, a settlement timer runs and the result can be challenged. Observed timers: 5 s and 180 s.
  - `result` is `yes`, `no`, `scalar` or empty. There is **no documented void value** in the pages read.
  - **Resolver rule:** act only on `finalized`; `yes` → outcome 0, `no` → outcome 1; any other final result goes to a human (void). `toAction()` implements this and is shown over six cases.
  - Sports markets can have a `close_time` two weeks after the game while `expected_expiration_time` is hours after it. This confirms SPEC §6: the curator must set the real event start.
  - Category lives on the series (`GET /series/{ticker}`; the event-level field is deprecated). Observed categories: Sports, Science and Technology, Elections, Climate and Weather. The default market list is dominated by multivariate combos (`KXMVE*`); use `mve_filter=exclude`. `min_close_ts/max_close_ts` only combine with status `closed` or empty.
  - Rate limits: 429 responses carry no `Retry-After`; the spike backs off exponentially. The documented token buckets apply to authenticated tiers.
- **Evidence hash:** sha256 of the raw response bytes (stable across two fetches 1.5 s apart: `644e5316…1b03`), plus a hash of a canonical field subset. Both are stored; the raw hash is what the resolver submits.
- **Open:** (1) **API and data terms not read.** kalshi.com serves a browser-verification checkpoint ("We're verifying your browser", HTTP 429) to scripted clients, and the web-fetch tool could not resolve the host, so the legal pages cannot be read from here. A human needs to read the Developer Agreement / API terms (start from the footer of https://docs.kalshi.com). Until then no Kalshi payloads are committed (`scripts/spikes/kalshi/.out/` is gitignored). (2) How a voided market is represented. (3) Whether terms allow a derivative product keyed to Kalshi outcomes (SPEC §14 says to ask a lawyer).

### D-004 · DFlow on-chain settlement (research only)

- **Phase 0 · Status: Inconclusive.** No code written, per the spec.
- **Sources:** the Solana announcement (https://solana.com/news/dflow-prediction-markets-api, dated 2025-12-01; direct fetch hit a Vercel browser check, so I read search summaries of it), the QuickNode guide for DFlow's Kalshi markets (read in full), and the DFlow docs at https://pond.dflow.net.
- **Finding:** the DFlow docs I can reach today (`llms.txt`) no longer list prediction-market pages; the old paths return 404. A QuickNode guide shows each tokenized market has, per settlement mint, a `marketLedger` account address, `yesMint`, `noMint`, `isInitialized`, `redemptionStatus` (`pending` / `open`) and `scalarOutcomePercent`. Search snippets of earlier DFlow docs say a settlement authority writes the determined outcome on-chain and that cancelled events are treated as scalar **[U, secondary]**.
- **Answer to "can the program read it?":** plausibly yes, because a ledger account is written on-chain. It is **not confirmed**: I found no program id, account layout, IDL, devnet deployment, or the identity of the settlement authority.
- **If readable, a v1 sketch:** `propose_resolution` could take the ledger account instead of a resolver signature. The program checks the account owner against a pinned DFlow program id, the discriminator, that the YES/NO mints match the event's recorded pair, and a determined/finalized flag with redemption open; it maps the result to outcome 0/1 and sends scalar/cancelled to void. The dispute window stays, because DFlow's authority and Kalshi remain trusted. Risks: a centralized writer, layout changes, and a lag between Kalshi `finalized` and DFlow redemption.
- **To unblock:** ask DFlow for the program id and IDL, and whether a devnet deployment exists. v0 stays off-chain (SPEC §13).

### D-005 · Media storage

- **Phase 0 · Status: Provisional.** IPFS via Pinata for v0.
- **Why:** metadata is immutable on-chain, so the URI must be stable; a CID is content-addressed. Moderation and legal takedown (SPEC §8) are possible on IPFS by unpinning and by a dedicated gateway, whereas Arweave is permanent by design.
- **Read [V-me]** https://docs.pinata.cloud/files/uploading-files.md and `account-management/limits.md`: upload is `POST https://uploads.pinata.cloud/v3/files` (multipart, `network=public`, Bearer JWT) and returns a CIDv1 (`bafybei…`). Free plan: 60 requests/minute. SDK has `files.public.delete`.
- **Spike:** `scripts/spikes/media/spike.ts` (`pnpm spike:media`) builds Metaplex-style JSON and validates sizes. **Upload is untested: no Pinata key was available.** The Metaplex limits it checks (name 32, symbol 10, URI 200) and the creation fee are **[U]**; the Metaplex docs render client-side. Verify in Phase 3.
- **Pinata plans [V-me]** (https://pinata.cloud/pricing, read 2026-11-05): Free $0 with 1 GB storage, 10 GB bandwidth, 500 files and 1 gateway; Picnic $20/month (500 GB bandwidth, extra storage $0.07/GB); Fiesta $100/month (2.5 TB bandwidth). The free plan covers a beta (about 250 coins at two files each); production needs a paid plan and a dedicated gateway. Pinata's takedown policy was not read **[U]**.
- **Open:** Irys/Arweave pricing, SOL payment and removal wording **[U]** (docs.irys.xyz did not render for scripted fetches); a Pinata key for an upload test.

### D-006 · Token program

- **Decided (provisional on D-002): classic SPL Token plus Metaplex metadata.** The AMM spike used a classic SPL coin mint with no mint or freeze authority and DAMM v2 accepted it. The position NFT is Token-2022 inside the AMM and does not affect the coin.

### D-007 · Program test stack

- **Decided:** LiteSVM (Rust) for program tests, as the Anchor 1.2.2 template does. Evidence: the root `ping` test and the three AMM spike tests (real devnet binary, clock control, account injection) run in under a second. Rust tests live in `programs/prequel/tests/`; the root `tests/` is reserved for cross-program and local-validator tests. `anchor test` on a validator, property tests and Trident (trident-cli 0.12.0 exists, not evaluated) come later (Phase 1 and 9).

### D-008 · Client SDK generator

- **Pending (Phase 5).** Candidates on npm today: `codama` 1.11.0, `@codama/cli` 1.6.3, `@codama/renderers-js` 2.8.0. `@coral-xyz/anchor` on npm was last published 2025-10-10 (0.32.1), far behind the Anchor 1.2.2 crates, so the current TS client must be read from the Anchor docs first.

### D-009 · Shared canon constants (Rust and TypeScript)

- **Pending (Phase 1).** SPEC §2 wants one shared confusables constant. Options: one data file read by the program's build script and by the TS library; code generation in one direction; two copies guarded by the fixture test (weakest). Also decide: a frozen version 1 table and a `canon_version` field, and ASCII-only input checked before uppercasing in both languages (Q-19).

### D-010 · Vault and payer account model

- **Pending (Phase 1).** The AMM spike shows the pattern that works: system-owned PDAs with no data (`seeder`) pay AMM rent and own the deposit accounts, because System Program transfers need a data-less payer. Lean towards the per-candidate vault being a system-owned PDA for the same reason. Accounting must come from `Candidate` state, never vault lamports. Invariants 1 and 2 need rewording (Q-05).

### D-011 · Dust rule

- **Pending (Phase 3).** Burn or add to the pool. The reviewer reports DAMM v2 can add liquidity to the existing position and lock it again **[V-rev]**; not tested. `sweep_dust` must never touch unclaimed allocations (Q-18).

### D-012 · RPC and webhook provider

- **Pending (Phase 5).** Helius or similar; keys in env only.

## B. Spec questions

Each is a gap or risk in SPEC.md. Proposals are suggestions only. "Rev" marks findings from the independent Opus spec review.

### B1. Resolve before Phase 1 code

1. **Q-02 Atomic launch (critical).** Verified squatting plus the "Born but never seeded" trap.
2. **Q-11 Snapshot and bound parameters per event.**
3. **Q-14 Co-proposer economics.**
4. **Q-13, Q-16, Q-19 Slate, attestation and Canon policy.**
5. **D-010 and Q-05 Vault model and rewritten invariants.**

### B2. Full list

**Q-01 `pool_costs` has no home and must be known at birth** · high · self, confirmed by Rev
Issue: §3 uses `pool_costs`, but it is not in Config, and birth fixes allocations before the pool is paid for. On Meteora the cost is rent only (observed 0.0226 SOL in LiteSVM; no config field), so it depends on the Rent sysvar and pinned account sizes, and SIMD-0437 is lowering rent in steps. Proposal (Rev): snapshot a launch-cost budget B per event, capped in code; the launch asserts measured cost ≤ B; any surplus goes to the treasury (write it down); fold the birth-time rent from Q-07 into B.

**Q-02 Pool squatting and the unseeded Born state** · **critical** · self, verified by the spike, upgraded by Rev
Verified: anyone can create the (coin, wSOL) pool on config 11 without holding the coin. If `birth_candidate` and `seed_pool` are separate transactions, a squatter wins the race and about 99% of the raise is stuck, because nothing else moves a Born candidate's SOL. Gating `claim` is not enough, and Rev notes a timed fallback that opens claims after a grace period leaves the SOL stranded and re-opens the squat. Proposal: **one atomic launch** (mint, supply, wrap, create pool, lock, revoke authority, fee, mark Born), plus a `birth_deadline` after which an unlaunched winner becomes Refunding. If compute or size forces a split, add a Birthing state in which no token leaves the program. Spike numbers: 122k CU and 22 accounts for create+lock; adding mint and metadata is being measured. Amends §5 and the candidate state machine.

**Q-03 What is `max_resolution_delay` measured from?** · medium
Proposal: from `expected_resolution`, bounded in `create_event`, snapshotted per event (Q-11). Rev adds a settle window for Disputed events (Q-17).

**Q-04 Nothing moves a candidate to Refunding; stored status can lag the clock** · medium
Proposal (merges Rev M-4): one pure `effective_status(event, candidate, snapshotted params, now)` used by the program, the SDK and the indexer; the stored status is only a cache and becomes sticky once written; `refund` flips Pending → Refunding on first use; add the birth-deadline branch from Q-02; use the snapshotted `min_raise`.

**Q-05 Invariants: missing phases and wording** · medium
Phase placement is in PROGRESS.md. Rev: invariant 1 is false once Commitments close, 2 is false after any donation, 3 ignores AMM rounding and leftovers, 6 misses indirect blocking, 9 never defines idempotent. Proposed replacements: solvency (vault ≥ rent + open commits; token vault ≥ unclaimed allocations; fee accounts ≥ unclaimed shares), a frozen total with `open + refunded + claimed = total`, "fee + launch costs + SOL deposited + SOL dust = total" and the token equivalent, liveness (with admin and resolver silent every candidate reaches Born-with-pool or Refunding within a bounded time), "no admin action can stop an exit", fee conservation per harvest and asset, forward-only statuses, "Born implies a pool exists", and idempotent defined as "a repeat call fails with a named error or does nothing".

**Q-06 Fee-split edge cases** · low
Both the no-co-proposer slice and the rounding remainder go to the treasury. Put the formula in the spec: `p = floor(h*3000/1e4)`, `c = floor(h*2000/1e4)`, `t = h - p - c`; u128 accumulator with a fixed scale, remainder carried forward; one ledger per asset.

**Q-07 Who pays birth-time rent and metadata fees?** · medium · self; Rev disagrees with my resolution
At `min_raise` the 1% fee is only 0.05 SOL, which rent for mint, token vault and metadata (about 0.009 SOL) plus a possible Metaplex fee **[U]** can approach. Paying from the success fee makes the treasury's take shrink and shifts costs that §3 charges to committers. Proposal (Rev): fold birth costs into the Q-01 budget B. This changes the economics, so it needs the user's explicit approval.

**Q-08 `lp_tokens` rounding** · low
Rename to `pool_tokens` (they are coin tokens, not AMM LP shares). Spike: with Meteora the deposits come from a liquidity value and a Q64.64 price, so the pool price differs from the clearing price by ~1e-8 to 7e-8 and SOL dust is 365 to 3,650 lamports. Count dust in invariant 3. Computing price and liquidity on-chain needs 256-bit arithmetic.

**Q-09 "Full refund" copy and the proposal fee** · low
The proposal fee is non-refundable. Also disclose the winning-side reality (Q-15): "Full refund if NO happens" next to a YES commit suggests a safety the YES side lacks.

**Q-10 Pause scope** · low
`co_propose` is blocked by either flag. Add `co_propose` to invariant 5 (no proposal, co-proposal or commit after close).

**Q-11 Admin can rewrite a live event's economics** · high · Rev (H-1)
`update_config` can raise `success_fee_bps` to 10000 after resolution, set `max_resolution_delay` to infinity, make disputes impossible or change `min_raise` after close. Proposal: snapshot every §3 parameter and the pinned AMM config into `EventState` at `create_event` and the fee split into the candidate at birth; `update_config` affects future events only; hard bounds in code (for example fee ≤ 500 bps, delay ≤ 30 days).

**Q-12 A permissionless launch must not take economic inputs from the caller** · high · Rev (H-2)
A caller could pick a hostile AMM config (99% fee, delayed activation) or become the pool creator. Proposal: pin the AMM program, config, fee-collection mode, activation rule and price range; build every CPI argument in-program; afterwards assert that our PDA holds the position NFT. The spike's `seed` trusts its caller and must not be copied.

**Q-13 First-come slate grab; no way to free a slot** · high · Rev (H-3)
Bots can fill all 12 slots per outcome for ~0.48 SOL of fees. Hidden candidates keep their slots and offensive metadata is permanent at birth. Proposal: an admin `reject_candidate` before close (100% refund, slot freed, fee kept), `require_attestation` on for the beta with per-wallet quotas, and a higher proposal fee.

**Q-14 `co_propose` always beats `commit`** · high · Rev (H-4)
Any committer can call `co_propose` (≥ 0.05 SOL, no fee) and share the 20% slice while plain committers get nothing. Proposal: either `co_propose` pays the proposal fee and its weight is frozen at the co-propose amount, or v0 drops money rewards for co-proposers (20% to the treasury, UI credit only), which removes `CoProposerShare` and the accumulator.

**Q-15 "Same price" holds on entry only** · high · Rev (H-5), consistent with the spike
With a 100 SOL raise the pool gets ~98.95 SOL, committers ~502.6M tokens; if all sell, they recover about half (~49.5 SOL stays locked). The spike shows a 1 SOL buy into a 4.95 SOL pool already averaging 21% above the clearing price. Whoever cranks the seed can bundle a claim and a sale. Proposal: delay trading by a fixed interval after launch (DAMM v2 activation point), show the exit math in the commit UI, and decide anti-sniper fee schedules explicitly. Economics and copy change.

**Q-16 Attestation scope** · medium · Rev (M-1)
The signed message omits name, display ticker and metadata URI, and has no domain tag. Sign: domain tag, program id, config, event, outcome, canon hash, sha256(name + ticker + URI), image hash, proposer, expiry. When reading the Ed25519 instruction require exactly one signature and all three instruction-index fields = u16::MAX.

**Q-17 Delay void can race a dispute; bond fate undefined** · medium · Rev (M-2)
Allow the delay void of a Disputed event only after `disputed_at` + a settle window. Bond rule: returned if the final outcome differs from the proposed one (a void counts) or on a delay void, otherwise forfeited to the treasury. Let the admin settle or void a ResolutionProposed event without a bond.

**Q-18 `sweep_dust` can take unclaimed allocations** · medium · Rev (M-3)
Track `claimed_lamports`; sweepable tokens = balance − ceil(unclaimed × committer_tokens / total); fix the destination; no grace period needed.

**Q-19 Canon table gaps and versioning** · low · Rev (L-1)
0/1/5 are mapped but 2/Z, 8/B, 6/G are not; changing the table later remaps candidate addresses. TypeScript `toUpperCase` is Unicode-aware ("ß" → "SS"). Freeze version 1, store `canon_version`, reject non-ASCII before uppercasing in both languages.

**Q-20 Rent lifetimes and wallet caps** · low · Rev (L-2, L-3)
Vault rent stays forever, `CoProposerShare` cannot close on a refunding candidate, `ExternalListing` blocks relisting. Add close cranks that return rent to its payer. Document `max_commit_per_wallet` as a UX guard only (N wallets bypass it).

## C. Registry snapshot (observation on 2026-11-05, not a pin)

crates.io `max_stable_version`, npm `latest`. The latest of each package is not necessarily a compatible set; the pins are in D-001.

| Package | Version | Package | Version |
| --- | --- | --- | --- |
| anchor-lang / -spl / -cli | 1.2.2 | @solana/kit | 8.6.0 |
| litesvm (crate / npm) | 0.18.2 / 1.6.2 | codama / @codama/cli / renderers-js | 1.11.0 / 1.6.3 / 2.8.0 |
| solana-verify | 0.5.2 | @metaplex-foundation/mpl-token-metadata | 3.4.0 |
| trident-cli / -fuzz | 0.12.0 | @squads-protocol/multisig | 2.1.2 (2024-08-30) |
| mpl-token-metadata (crate) | 5.1.1 | next / react | 16.4.5 / 19.3.1 |
| spl-token / solana-program | 9.0.0 / 5.2.0 | fastify / drizzle-orm / bullmq | 5.12.8 / 0.45.4 / 6.7.0 |
| @coral-xyz/anchor | 0.32.1 (stale) | playwright / vitest / typescript | 1.64.1 / 5.1.0 / 7.0.2 |
