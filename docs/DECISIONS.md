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
| D-013 | Phase 1 parameter caps | 1 | Proposed values, confirm |
| D-014 | Attestation message v1 | 1 | Decided with Q-16 |
| D-015 | Ticker canon v1 | 1 | Decided with Q-19 |
| Q-01 to Q-20 | Spec questions | various | Q-02, Q-11, Q-13, Q-14, Q-16, Q-19 decided 2026-11-05; the rest open, section B |

## A. Decisions

### D-001 · Toolchain pins

- **Phase 0 · Decided 2026-11-05.** Agave came from the official installer (`stable` channel) and the Anchor CLI from a plain `cargo install`, both before the repo's `rust-toolchain.toml` and `scripts/bootstrap.sh` existed. `bootstrap.sh` encodes the same pins and was run here only on its already-installed path; its clean-machine install path is first exercised by CI. `scripts/rust-checks.sh` ran here end to end.

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
| GitHub Actions | `actions/checkout@v4`, `setup-node@v4`, `pnpm/action-setup@v4`, `cache@v4` | **[V-me]** GitHub runs on 2026-11-05: run 41984938273 failed in the program job (`cargo install anchor-cli` ran under the repo's Rust 1.89.0 and a locked dependency needs 1.91). After building the tools with the stable toolchain, **run 41987267156 passed both jobs** (JS: 28 turbo tasks plus the scripts lint; program: fmt, `anchor build`, clippy, the LiteSVM test). Runs in between were cancelled by newer pushes (`cancel-in-progress`), so no cache was saved until that one. GitHub warned that Node 20 actions (`checkout@v4`, `cache@v4`) are forced onto Node 24: bump the majors when verified. |

- **Sandbox quirks:** Node's built-in `fetch` ignores the proxy, so export `NODE_USE_ENV_PROXY=1`; put `~/.local/share/solana/install/active_release/bin` on `PATH`. `anchor build` warns that the keypair and `declare_id!` differ; program keypairs are never committed, so the real id is set in Phase 10.
- **Build-target caveat [V-rev, read from ELF headers]:** `cargo-build-sbf` 4.4.0 with platform-tools v1.57 emits SBPFv3 programs (`e_flags=3`) while the dumped Meteora binary is v0, and `LiteSVM::new()` enables every feature (p-token, SIMD-0312). Before relying on the spike numbers: confirm devnet accepts SBPFv3 programs (or build with `--arch`), and run the atomic launch once on `solana-test-validator` with devnet clones (Phase 1 prerequisite in PROGRESS.md).
- **Program dependencies added in Phase 1:** `solana-sha256-hasher` 3 (with its `sha2` feature for host tests) and `solana-instructions-sysvar` 3. Anchor 1.2.2's `solana_program` shim no longer re-exports hashing or the instructions sysvar; both are official Solana SDK crates already in the dependency graph, at the versions of anchor-lang's default `solana-v3` set.
- **Dev dependencies added beyond SPEC §4** (approved with the Phase 0 go-ahead): vitest, vite, eslint stack, prettier, `@types/node`; in the throwaway AMM spike only: `bytemuck`, `serde_json`, `base64`, `solana-account`.

### D-002 · AMM: Meteora DAMM v2 vs Raydium CPMM

- **Phase 0 · Status: Provisional; the atomic launch was approved on 2026-11-05 (Q-02).** Chosen: **Meteora DAMM v2, public config 11, launched atomically.** The atomic launch now works on localnet (below). Still needed: (1) the user's approval of the atomic-launch amendment (Q-02), (2) a check of Meteora's mainnet upgrade authority **[U]**, (3) Metaplex metadata inside the same transaction (sized below, not run).
- **What I verified myself [V-me], on devnet and in LiteSVM with the real devnet binary** (`scripts/spikes/amm`, `pnpm spike:amm`, 9 tests pass: 8 in LiteSVM, 1 IDL check; output in `scripts/spikes/amm/test-output.log`):
  - Program `cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG` is deployed on devnet (1,559,448 bytes, sha256 `82bb9375…1ec2`, hash-pinned in `fetch-fixtures.sh`).
  - Config 11 `AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW`, decoded: flat 1.00% fee (cliff_fee_numerator 10,000,000 of 1e9), no dynamic fee, protocol cut 20%, `collect_fee_mode = 1` (token B only), full price range, `pool_creator_authority` empty (public).
  - IDL extracted from npm `@meteora-ag/cp-amm-sdk@1.5.1` (cp_amm 0.2.5, 39 instructions, none unlocks), sha256 `52ad3266…e0ef`. Anchor 1.2.2 `declare_program!` compiles from it (needs a `bytemuck` dependency).
  - **Pool creation and permanent lock in one program instruction, signed only by PDAs:** 121,988 CU (123,484 with another mint address: the PDA bump searches move it by about 1.5k), 22 unique accounts, 811-byte legacy message, 22.6M lamports of rent (LiteSVM default rent; mainnet rent is falling under SIMD-0437 [V-rev]). A system-owned `seeder` PDA pays and owns the deposit accounts; a separate PDA holds the position NFT.
  - **Deposits at a 5 SOL raise:** all 497,487,437,185,929 pool tokens and 4,949,999,635 of 4,950,000,000 lamports (365 lamports of dust); the marginal price equals the clearing price to about 1e-15, and the SOL dust is the part of the deposit that would sit below the config's lower price bound, `pool_sol × √(Pmin/P)` (7.4e-8, 2.3e-8 and 7.4e-9 of the SOL side at 5, 50 and 500 SOL), not rounding **[V-rev, recomputed from the test constants]**. At 50 and 500 SOL the SOL dust is 1,155 and 3,650 lamports (measured).
  - After the lock: `unlocked_liquidity = 0`, `permanent_locked_liquidity` = the whole position. No IDL instruction unlocks.
  - **Fee claim by CPI as the NFT-holding PDA:** 30,181 CU. Fees arrive in wSOL only (token A fees = 0). The LP's share measured 0.8028% of volume: the 1% fee less the 20% protocol cut.
  - A 1 SOL buy into a 4.95 SOL pool averaged 1.206e-5 vs the 9.95e-6 clearing price (slippage plus 1% fee). This is the "exit math" in Q-15.
  - **Squatting works without holding the coin:** a third party with only 0.5 wSOL and an empty token account created the (coin, wSOL) pool on config 11 by pricing it at the top of the range (zero token A needed). Prequel's own creation for the same address then fails ("account already in use"). Gating claims is not enough; see Q-02.
  - **Atomic launch passes** (`scripts/spikes/amm/program/tests/probe2.rs`, built by an Opus agent in a scratch copy and re-run in this tree). One instruction, signed only by PDAs: create the coin's mint PDA (6 decimals, no freeze authority), mint 1e15 base units, revoke the mint authority, create and fund the wSOL account, create the pool, lock the position permanently. **153,359 CU, 22 unique accounts, 820-byte legacy message (885-byte transaction; limit 1,232), 28,146,240 lamports of rent** (AMM 22,606,080 + mint 1,461,600 + two token accounts 4,078,560, LiteSVM default rent). **These figures are a lower bound for the pool part only:** the probe leaves out the Config, EventState, Candidate, Vault, treasury and FeeLedger accounts, takes no fee and marks nothing Born, and Metaplex adds more. The reviewer estimates 1.14 to 1.24 KB against the 1,232-byte limit, so **plan for a v0 transaction with an address lookup table** (SPEC §5 already allows one). Final state: mint and freeze authority none, supply 1e15, a later `MintTo` fails, the pool holds 497,487,437,185,929 tokens and 4,949,999,635 lamports (365 lamports of dust stay in the seeder), the NFT is held by the owner PDA, the position is fully locked. At 50 and 500 SOL: 153,371 and 153,634 CU, SOL dust 1,155 and 3,650 lamports, token dust 0, same rent.
  - **Metaplex metadata is not yet in that budget [V-rev, from the `mpl-token-metadata` 5.1.1 crate source, not run].** `CreateV1` adds 3 unique accounts (metadata PDA, instructions sysvar, metadata program), so 25, and about 99 bytes plus the name, symbol and URI strings (up to 254 bytes). With maximum-length strings the transaction is 1,238 bytes, 6 over the limit; a URI of 194 bytes or less fits, and a v0 transaction with a lookup table fixes it either way. It must run before the mint authority is revoked, because that authority signs it. CU unknown.
  - **Pre-funded addresses (an attacker sends lamports to an address before the launch creates it):** the AMM's own accounts (pool, vaults, position, position NFT mint, NFT account) tolerate it in 18 of 18 cases. Our own PDAs do **not** with a plain `create_account` (9 of 9 fail with "account already in use"); they pass with top-up + allocate + assign or with `CreateAccountAllowPrefund` (SIMD-0312, active on devnet since epoch 1052, mainnet not checked), which the agent reports Anchor 1.2.2's `init` uses. A 1 SOL donation to the wSOL vault launches fine but sits unaccounted (the pool's `token_b_amount` is unchanged), which is why accounting must come from state.
  - **After the lock, as the owner PDA:** `remove_liquidity`, `remove_all_liquidity` fail (6023), `close_position` fails (6039), `lock_position` and `lock_inner_position` fail (6023). **`split_position` and `split_position2` succeed** and move the permanently locked liquidity, with its fee rights, into another position (an outsider can create the empty one). Nothing can be withdrawn, but the program must never sign a split, never expose a generic owner-signed call, and never transfer the position NFT (transfer would do the same; untested). The spike's `owner_cpi` forwarder exists only to show this and is marked unsafe.
  - LiteSVM runs the classic token program as p-token (105 to 235 CU per call, active on devnet since epoch 1044); on a cluster still on the legacy token program the ten token calls cost more (unmeasured, still far below the 1.4M limit).
- **Assertion coverage.** The main launch, the original seed/swap/harvest flow and the squat test assert their outcomes (the squat test fails if the attacker's creation is rejected, and checks that Prequel's own `seed` then fails with "already in use"). The pre-funded-address, post-lock and overfunding probes in `probe2.rs` print their outcomes without asserting; treat them as recorded observations (the printed output is committed) until they are converted to asserts.
- **Reviewer findings [V-rev]** (docs, npm bundles and devnet accounts; summarised here and in Q-02 and Q-12):
  - Raydium CPMM charges a 0.15 SOL pool creation fee (admin-changeable), pays LP fees in both tokens, and needs a second closed-source lock program (LP Lock) with a different IDL format. Its crate pins Anchor 1.0.2, which conflicts with 1.2.2. Program ids differ between devnet and mainnet.
  - Raydium lets the creator choose the pool address (so a PDA of ours cannot be squatted); Meteora derives it from config and mints.
  - Meteora: operator fee changes are capped at 10% per pool; protocol share fixed at 20%; operator `set_pool_status` can disable swap/liquidity paths (whether claims stay open is **[U]**).
  - Raydium's admin can change the 1% tier's rates for every pool on it, and pause pools.
- **Unverified [U]:** Meteora mainnet upgrade authority and governance; whether a paused pool still allows fee claims; Jupiter and DexScreener display (Jupiter's docs say both AMMs are eligible for routing); compute for Raydium; Metaplex metadata CU; mainnet status of `CreateAccountAllowPrefund`.
- **What would flip this to Raydium CPMM:** Meteora's upgrade authority turns out to be a single key; or Metaplex metadata cannot be fitted into the atomic transaction; or the user rejects the atomic launch (Raydium lets the creator choose the pool address, so a split birth/seed could be made safe there, at the cost of its 0.15 SOL creation fee, a second closed-source lock program and fees in both tokens).
- **Phase 0 done-when for the AMM spike** ("create a pool, lock the LP and claim fees by CPI on localnet") is met for Meteora. Raydium was compared from docs and devnet reads only, not run.

### D-003 · Kalshi ingestion and API terms

- **Phase 0 · Decided 2026-11-05, with the terms review still open.** Spike: `scripts/spikes/kalshi/spike.ts` (`pnpm spike:kalshi`, needs `NODE_USE_ENV_PROXY=1` here), run live against production read-only endpoints.
- **Sources [V-me]:** https://docs.kalshi.com/getting_started/api_environments.md, `market_lifecycle.md`, `market_settlement.md`, `quick_start_market_data.md`, `rate_limits.md` and the OpenAPI for `get-markets` and `get-market`, read 2026-11-05.
- **Decision:** v0 reads the **unauthenticated** public market-data endpoints at `https://external-api.kalshi.com/trade-api/v2` (also supported: `api.elections.kalshi.com`; demo `external-api.demo.kalshi.co`). No API key is needed to list markets or read a settled one.
- **Facts the design relies on:**
  - Market statuses: `initialized`, `active`, `inactive`, `closed`, `determined`, `disputed`, `amended`, `finalized` (terminal). While `determined`, a settlement timer runs and the result can be challenged. Observed timers: 5 s up to 1,800 s (60, 180, 300 and 1,800 s on the saved listing).
  - `result` is `yes`, `no`, `scalar` or empty. **A cancelled or voided event shows up as `scalar` with a fractional `settlement_value_dollars` [V-me].** In 6,000 recent settled non-combo markets (2026-11-05): 3,400 `no`, 2,538 `yes`, 62 `scalar` (1.0%), none empty. Examples: 0.50 on a cancelled two-way match, 0.33 on a cancelled three-way match. This matches the DFlow snippet that cancelled events are treated as scalar (D-004), and means the void path is routine, not rare. Reproduce with `scripts/spikes/kalshi/scan-settled.sh`.
  - **Resolver rule:** act only on `finalized`; `yes` → outcome 0, `no` → outcome 1; any other final result goes to a human (void). `toAction()` implements this and is shown over six cases.
  - Sports markets can have a `close_time` two weeks after the game while `expected_expiration_time` is hours after it. This confirms SPEC §6: the curator must set the real event start.
  - Category lives on the series (`GET /series/{ticker}`; the event-level field is deprecated). Observed categories: Sports, Science and Technology, Elections, Climate and Weather. The default market list is dominated by multivariate combos (`KXMVE*`); use `mve_filter=exclude`. `min_close_ts/max_close_ts` only combine with status `closed` or empty.
  - Rate limits: 429 responses carry no `Retry-After`; the spike backs off exponentially. The documented token buckets apply to authenticated tiers.
- **Evidence hash:** sha256 of the raw response bytes (stable across two fetches 1.5 s apart: `644e5316…1b03`), plus a hash of a canonical field subset. Both are stored; the raw hash is what the resolver submits.
- **Open:** (1) **API and data terms not read.** kalshi.com serves a browser-verification checkpoint ("We're verifying your browser", HTTP 429) to scripted clients, and the web-fetch tool could not resolve the host, so the legal pages cannot be read from here. A human needs to read the Developer Agreement / API terms (start from the footer of https://docs.kalshi.com). Until then no Kalshi payloads are committed (`scripts/spikes/kalshi/.out/` is gitignored). (2) Whether the terms allow a derivative product keyed to Kalshi outcomes (SPEC's "Before mainnet" section says to ask a lawyer).

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

- **Decided:** LiteSVM (Rust) for program tests, as the Anchor 1.2.2 template does. Evidence: the root `ping` test and the nine AMM spike tests (eight LiteSVM plus one IDL check; real devnet binary, clock control, account injection) run in seconds. **Deviation for your approval:** Rust integration tests live in `programs/prequel/tests/` (the Anchor 1.2.2 template) rather than SPEC §4's root `tests/`; the root `tests/` stays for cross-program tests. Rust tests live in `programs/prequel/tests/`; the root `tests/` is reserved for cross-program and local-validator tests. `anchor test` on a validator, property tests and Trident (trident-cli 0.12.0 exists, not evaluated) come later (Phase 1 and 9).

### D-008 · Client SDK generator

- **Pending (Phase 5).** Lead **[V-rev, read from source, not run]**: `anchor-cli` 1.2.2 has a built-in `anchor codama` command (Js, JsUmi, Rust and Go targets) and a `[clients]` section in `Anchor.toml`; evaluate it before adding Codama separately. Candidates on npm today: `codama` 1.11.0, `@codama/cli` 1.6.3, `@codama/renderers-js` 2.8.0. `@coral-xyz/anchor` on npm was last published 2025-10-10 (0.32.1), far behind the Anchor 1.2.2 crates, so the current TS client must be read from the Anchor docs first.

### D-009 · Shared canon constants (Rust and TypeScript)

- **Pending (Phase 1).** SPEC §2 wants one shared confusables constant. Options: one data file read by the program's build script and by the TS library; code generation in one direction; two copies guarded by the fixture test (weakest). Also decide: a frozen version 1 table and a `canon_version` field, and ASCII-only input checked before uppercasing in both languages (Q-19).

### D-010 · Vault and payer account model

- **Phase 1 · Decided 2026-11-05 (implementation decision, flagged for review).** The per-candidate **vault is a program-owned data account** (`Vault { candidate, bump }`); the **AMM payer (`seeder`) is a separate data-less, system-owned PDA** (Phase 3/4).
- **Why not a system-owned vault:** under Solana's rent rule a system account can only be left at 0 lamports or at least 890,880. A donation of even 1 lamport to a system-owned vault makes the last refund fail (it would leave 1 lamport), so an attacker could block the final committer. A program-owned vault only gains a harmless surplus, refunds just debit the committed amount, and invariant 2's wording ("vault ends at the rent-exempt minimum") stays true.
- **Why the seeder stays system-owned:** AMM rent is paid through System Program transfers, which need a data-less payer (verified in the AMM spike). At launch the program moves the pool SOL from the vault to the seeder by direct lamport arithmetic, which is allowed because the program owns the vault.
- **Rules:** accounting comes from `Candidate` state, never vault lamports; every program-created PDA must tolerate pre-funding (Anchor 1.2.2 `init` does; tested); invariants 1 and 2 get the reworded versions in Q-05 as extra tests.

### D-011 · Dust rule

- **Pending (Phase 3).** Burn or add to the pool. The reviewer reports DAMM v2 can add liquidity to the existing position and lock it again **[V-rev]**; not tested. `sweep_dust` must never touch unclaimed allocations (Q-18).

### D-012 · RPC and webhook provider

- **Pending (Phase 5).** Helius or similar; keys in env only.

### D-013 · Phase 1 parameter caps (Q-11)

- **Phase 1 · Proposed values, mine; please confirm or change.** Config and `create_event` reject values outside these constants; `create_event` copies the validated values into the event.
- `success_fee_bps` at most 500 (5%). Fee split must sum to 10,000.
- `dispute_window` 60 seconds to 7 days. `max_resolution_delay` 1 day to 30 days. `launch_window` (time allowed to launch a winner before it refunds) 10 minutes to 7 days, default 24 hours.
- `dispute_bond` 0.01 to 10 SOL. `proposal_fee` at most 1 SOL. `min_commit` at least 0.001 SOL. `min_raise` at least `min_commit` and at most 1,000 SOL. `max_candidates_per_outcome` 1 to 32.
- Event times: `now < window_close < event_start`, `expected_resolution` between `event_start` and `event_start` + 90 days.

### D-014 · Attestation message v1 (Q-16)

- **Phase 1 · Decided with Q-16.** The canonicalizer signs these bytes: `"PREQUEL-PROPOSE-V1"` (18 ASCII bytes) || program id || config || event || outcome (1) || canon hash (32) || sha256(name || 0x00 || ticker || 0x00 || uri) || image hash (32) || proposer || expiry (i64 LE). The program requires the previous instruction to be an Ed25519 precompile call with exactly one signature, all three instruction-index fields = u16::MAX, public key = the configured canonicalizer, message = the expected bytes, and expiry not yet passed. `require_attestation` is snapshotted per event.

### D-015 · Ticker canon v1 (Q-19)

- **Phase 1 · Decided with Q-19.** Input must be 2 to 10 ASCII letters or digits (anything else, including non-ASCII, is rejected before any case mapping). Uppercase, then map 0→O, 1→I, 5→S, then `sha256` of those bytes is the canon hash (the PDA seed). The displayed ticker keeps the uppercased input without the confusable mapping. The mapping lives in one plain-text file read by the program's `build.rs` and by the TypeScript library; a shared fixture test keeps both hashes equal. `canon_version = 1` is stored on each event.

## B. Spec questions

Each is a gap or risk in SPEC.md. Proposals are suggestions only. "Rev" marks findings from the independent Opus spec review.

### B1. Resolve before Phase 1 code

1. **Q-02 Atomic launch (critical).** Verified squatting plus the "Born but never seeded" trap.
**Decision (2026-11-05, you): atomic launch.** One transaction mints the coin, creates the Meteora pool, locks it and marks the candidate Born; a winner not launched by a deadline refunds. Replaces SPEC's separate `birth_candidate` and `seed_pool` (built in Phase 3/4). Phase 1 reserves the deadline fields.
2. **Q-11 Snapshot and bound parameters per event.**
**Decision (2026-11-05, you): freeze per event + hard caps.** `create_event` copies every economic setting into the event; `update_config` affects future events only; code constants bound the values (D-013).
3. **Q-14 Co-proposer economics.**
**Decision (2026-11-05, you): co-propose pays the fee.** `co_propose` pays the same non-refundable proposal fee; a co-proposer's weight is frozen at the amount committed when joining; the 20% slice stays.
4. **Q-13, Q-16, Q-19 Slate, attestation and Canon policy.**
5. **D-010 and Q-05 Vault model and rewritten invariants.**

### B2. Full list

**Q-01 `pool_costs` has no home and must be known at birth** · high · self, confirmed by Rev
Issue: §3 uses `pool_costs`, but it is not in Config, and birth fixes allocations before the pool is paid for. On Meteora the cost is rent only (measured in LiteSVM: 22,606,080 lamports for the AMM accounts, 28,146,240 for the whole launch including the mint and two token accounts; no config field), so it depends on the Rent sysvar and pinned account sizes, and SIMD-0437 is lowering rent in steps. Proposal (Rev): snapshot a launch-cost budget B per event, capped in code; the launch asserts measured cost ≤ B; any surplus goes to the treasury (write it down); fold the birth-time rent from Q-07 into B.

**Q-02 Pool squatting and the unseeded Born state** · **critical** · self, verified by the spike, upgraded by Rev
Verified: anyone can create the (coin, wSOL) pool on config 11 without holding the coin. If `birth_candidate` and `seed_pool` are separate transactions, a squatter wins the race and about 99% of the raise is stuck, because nothing else moves a Born candidate's SOL. Gating `claim` is not enough, and Rev notes a timed fallback that opens claims after a grace period leaves the SOL stranded and re-opens the squat. Proposal: **one atomic launch** (mint, supply, wrap, create pool, lock, revoke authority, fee, mark Born), plus a `birth_deadline` after which an unlaunched winner becomes Refunding. If compute or size forces a split, add a Birthing state in which no token leaves the program. Measured (a lower bound, pool part only): 153k CU, 22 accounts, 820 bytes in a legacy transaction. The real launch adds the program's own accounts and Metaplex metadata, so plan for a v0 transaction with a lookup table (D-002). Amends §5 and the candidate state machine.

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
Rename to `pool_tokens` (they are coin tokens, not AMM LP shares). Spike: Meteora takes a liquidity value and a Q64.64 price. The marginal price matches the clearing price to ~1e-15, and the SOL side leaves 365 to 3,650 lamports of dust equal to `pool_sol × √(Pmin/P)` (the config's lower price bound). Invariant 3 needs a dust term with that closed form. Computing price and liquidity on-chain needs 256-bit arithmetic.

**Q-09 "Full refund" copy and the proposal fee** · low
The proposal fee is non-refundable. Also disclose the winning-side reality (Q-15): "Full refund if NO happens" next to a YES commit suggests a safety the YES side lacks.

**Q-10 Pause scope** · low
`co_propose` is blocked by either flag. Add `co_propose` to invariant 5 (no proposal, co-proposal or commit after close).

**Q-11 Admin can rewrite a live event's economics** · high · Rev (H-1)
`update_config` can raise `success_fee_bps` to 10000 after resolution, set `max_resolution_delay` to infinity, make disputes impossible or change `min_raise` after close. Proposal: snapshot every §3 parameter and the pinned AMM config into `EventState` at `create_event` and the fee split into the candidate at birth; `update_config` affects future events only; hard bounds in code (for example fee ≤ 500 bps, delay ≤ 30 days).

**Q-12 A permissionless launch must not take economic inputs from the caller** · high · Rev (H-2)
A caller could pick a hostile AMM config (99% fee, delayed activation) or become the pool creator. Proposal: pin the AMM program, config, fee-collection mode, activation rule and price range; build every CPI argument in-program; afterwards assert that our PDA holds the position NFT. The spike's `seed` trusts its caller and must not be copied. After the lock the owner PDA could still `split_position` the locked liquidity into someone else's position (verified), so the program must never sign a split, expose a generic owner-signed call, or transfer the position NFT.

**Q-13 First-come slate grab; no way to free a slot** · high · Rev (H-3)
**Decision (2026-11-05, you): adopted.** Admin `reject_candidate` before close (100% refund, slot freed, proposal fee kept), the beta runs with `require_attestation` on.
Bots can fill all 12 slots per outcome for ~0.48 SOL of fees. Hidden candidates keep their slots and offensive metadata is permanent at birth. Proposal: an admin `reject_candidate` before close (100% refund, slot freed, fee kept), `require_attestation` on for the beta with per-wallet quotas, and a higher proposal fee.

**Q-14 `co_propose` always beats `commit`** · high · Rev (H-4)
Any committer can call `co_propose` (≥ 0.05 SOL, no fee) and share the 20% slice while plain committers get nothing. Proposal: either `co_propose` pays the proposal fee and its weight is frozen at the co-propose amount, or v0 drops money rewards for co-proposers (20% to the treasury, UI credit only), which removes `CoProposerShare` and the accumulator.

**Q-15 "Same price" holds on entry only** · high · Rev (H-5), consistent with the spike
With a 100 SOL raise the pool gets ~98.95 SOL, committers ~502.6M tokens; if all sell, they recover about half (~49.5 SOL stays locked). The spike shows a 1 SOL buy into a 4.95 SOL pool already averaging 21% above the clearing price. Whoever cranks the seed can bundle a claim and a sale. Proposal: delay trading by a fixed interval after launch (DAMM v2 activation point), show the exit math in the commit UI, and decide anti-sniper fee schedules explicitly. Economics and copy change.

**Q-16 Attestation scope** · medium · Rev (M-1)
**Decision (2026-11-05, you): adopted.** The attestation signs a domain tag, program id, config, event, outcome, canon hash, a hash of name+ticker+URI, image hash, proposer and expiry (D-014).
The signed message omits name, display ticker and metadata URI, and has no domain tag. Sign: domain tag, program id, config, event, outcome, canon hash, sha256(name + ticker + URI), image hash, proposer, expiry. When reading the Ed25519 instruction require exactly one signature and all three instruction-index fields = u16::MAX.

**Q-17 Delay void can race a dispute; bond fate undefined** · medium · Rev (M-2)
Allow the delay void of a Disputed event only after `disputed_at` + a settle window. Bond rule: returned if the final outcome differs from the proposed one (a void counts) or on a delay void, otherwise forfeited to the treasury. Let the admin settle or void a ResolutionProposed event without a bond.

**Q-18 `sweep_dust` can take unclaimed allocations** · medium · Rev (M-3)
Track `claimed_lamports`; sweepable tokens = balance − ceil(unclaimed × committer_tokens / total); fix the destination; no grace period needed.

**Q-19 Canon table gaps and versioning** · low · Rev (L-1)
**Decision (2026-11-05, you): adopted.** Frozen table v1 (the spec's 0→O, 1→I, 5→S only), ASCII-only input, `canon_version` stored on the event (D-015).
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
