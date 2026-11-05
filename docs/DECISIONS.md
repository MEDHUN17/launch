# DECISIONS

One entry per decision. A version or doc link goes in only after it was read from the official source, with the date and URL. Where a decision is still open, the entry says what has to be learned first. Nothing here changes SPEC.md until the user approves it.

- **Status:** `Pending` (not decided; the phase says when), `Decided`, or `Superseded by D-xxx`.
- **Entry format:** Options · Choice · Why · Sources (official link, version or commit read, date read).
- **Section B (Q-xx) is not decisions.** It lists places where SPEC.md is silent, ambiguous or looks wrong. Each needs the user's answer before the phase that depends on it.

## Index

| ID | Title | Phase | Status |
| --- | --- | --- | --- |
| D-001 | Toolchain pins | 0 | Pending (network blocked) |
| D-002 | AMM: Meteora DAMM v2 vs Raydium CPMM | 0 | Pending (network blocked) |
| D-003 | Kalshi ingestion and API terms | 0 | Pending (network blocked) |
| D-004 | DFlow on-chain settlement (research only) | 0 | Pending (network blocked) |
| D-005 | Media storage: IPFS (Pinata) vs Arweave | 0 | Pending (network blocked) |
| D-006 | Token program: classic SPL vs Token-2022 | 0 | Pending |
| D-007 | Program test stack | 0 and 1 | Pending |
| D-008 | Client SDK generator | 0 and 5 | Pending |
| D-009 | Shared canon constants (Rust and TypeScript) | 1 | Pending |
| D-010 | Vault account representation | 1 | Pending |
| D-011 | Dust rule | 3 | Pending |
| D-012 | RPC and webhook provider | 5 | Pending |
| Q-01 to Q-10 | Spec questions | various | Open, see section B |

## A. Decisions

### D-001 · Toolchain pins

- **Phase:** 0 · **Status:** Pending
- **Scope:** Rust (`rust-toolchain.toml`), Agave/Solana CLI and platform tools, Anchor (program crates and CLI), Node, pnpm, Turborepo, TypeScript, Next.js, Fastify, Drizzle, BullMQ, Playwright.
- **Options:** the registry-latest stable of each, unless a compatibility note says otherwise. The set has to be mutually compatible: Anchor, Agave and its platform-tools Rust, LiteSVM, Codama, and the AMM's CPI crate or IDL.
- **Choice:** none yet.
- **Why pending:** the docs hosts (`www.anchor-lang.com`, `release.anza.xyz`) are denied by the sandbox network policy, so compatibility notes haven't been read. The registry snapshot in section C is observation only, not a pin.
- **Sources:** to add.

### D-002 · AMM: Meteora DAMM v2 vs Raydium CPMM

- **Phase:** 0 (spike A) · **Status:** Pending
- **Spec criteria (§4):** pool creation by CPI; a permanent LP lock that can still claim fees; a 1% fee tier; Jupiter routing. No in-house AMM.
- **Also to find out in the spike:**
  - Can fees be collected in SOL only? If not, the FeeLedger tracks both assets (§3).
  - How is the pool address derived (from the token pair, or also from a config)? This feeds Q-02.
  - Are the AMM's creation costs deterministic, and where are they read from? This feeds Q-01 (`pool_costs`).
  - Compute units and account count for `seed_pool`, and whether it needs an address lookup table (§5).
  - Can it be called as a CPI with a PDA as owner of the position or LP, and which crate or IDL should the CPI use (a published crate, or Anchor's IDL-based CPI generation)? It must match the Anchor version from D-001.
  - Are the program and its config accounts on devnet, so they can be dumped as hashed localnet fixtures? Per §1, dump from devnet, never mainnet.
  - Can anyone other than the pool owner change fees or pause the pool (admin powers over a locked pool)?
  - Jupiter routing can only be checked in docs: Jupiter doesn't run on devnet and mainnet is out of scope.
- **Options:** Meteora DAMM v2, Raydium CPMM.
- **Choice:** none yet. **Sources:** to add.

### D-003 · Kalshi ingestion and API terms

- **Phase:** 0 (spike B) · **Status:** Pending
- **To find out:** read the API docs and terms of use first (§6). Which endpoints give open markets and a settled market without credentials? Rate limits. What the terms say about caching, redistribution and commercial use (this affects `external_markets` and `evidence_snapshots`). How a voided or cancelled market looks. Which category fields map to the §6 blocklist. Production vs demo hosts.
- **Evidence hash:** sha256 over the raw response bytes, stored verbatim in `evidence_snapshots` so the hash can be re-checked. This proves what the resolver saw; it doesn't make resolution trustless (see D-004).
- **Choice:** none yet. **Sources:** to add.

### D-004 · DFlow on-chain settlement (research only)

- **Phase:** 0 (research C, no code) · **Status:** Pending
- **Question (§6):** can the program read the settlement of DFlow's tokenized Kalshi markets on-chain? Start from the DFlow Prediction Markets API announcement on solana.com (link in SPEC §6).
- **If yes:** draft trustless resolution for v1 here (what account is read, who writes it, lag, trust model, failure modes). It stays out of v0 scope (§13).
- **If no:** record why, and what would change that.
- **Choice:** none yet. **Sources:** to add.

### D-005 · Media storage: IPFS (Pinata) vs Arweave

- **Phase:** 0 (spike D) · **Status:** Pending
- **Criteria:** the on-chain metadata is immutable, so the URI has to stay valid; permanence vs the ability to take content down (moderation and legal, §8); cost model; upload API from Node; Metaplex JSON standard; gateway reliability for wallets and explorers; how to develop and test without paid services.
- **Needs from the user (optional):** a Pinata API key for an upload test. Without one the spike can use a local IPFS node and docs only.
- **Choice:** none yet. **Sources:** to add.

### D-006 · Token program: classic SPL vs Token-2022

- **Phase:** 0 · **Status:** Pending
- **Default (§3):** classic SPL Token plus Metaplex metadata. Token-2022 only for a strong reason found in Phase 0, most likely from the AMM spike (D-002) or wallet and Jupiter support.
- **Choice:** classic SPL unless D-002 says otherwise. **Sources:** to add.

### D-007 · Program test stack

- **Phase:** 0 and 1 · **Status:** Pending
- **Spec (§9):** LiteSVM (or the current recommended fast framework) with clock warping; `anchor test` on a local validator with the chosen AMM and Metaplex programs cloned; property tests; Trident fuzzing.
- **To decide:** whether integration tests are Rust or TypeScript; how AMM and Metaplex program binaries and config accounts enter the tests (hash-pinned fixtures fetched by a script, not committed binaries).
- **Choice:** none yet. **Sources:** to add.

### D-008 · Client SDK generator

- **Phase:** 0 and 5 · **Status:** Pending
- **Spec (§4):** the generator the current Anchor docs recommend, for example Codama.
- **Choice:** none yet. **Sources:** to add (Anchor docs).

### D-009 · Shared canon constants (Rust and TypeScript)

- **Phase:** 1 · **Status:** Pending
- **Problem:** §2 wants the confusables table in one shared constant used by both the program and the TypeScript library. A Rust crate and a TS package can't import each other.
- **Options:** one data file (for example JSON in `packages/shared`) read by the program's build script and by the TS library; code generation in one direction; or two copies guarded only by the cross-language fixture test (weakest).
- **Choice:** none yet; leaning towards the single data file plus the fixture test. **Sources:** n/a.

### D-010 · Vault account representation

- **Phase:** 1 · **Status:** Pending
- **Options:** a program-owned data account, or a system-owned PDA moved by `invoke_signed`.
- **Constraints:** invariant 2 (the vault ends at the rent-exempt minimum on refund); it holds one candidate's SOL only; accounting comes from `Candidate` state, never from vault lamports, because anyone can send lamports to it.
- **Choice:** none yet. **Sources:** to add (Anchor docs).

### D-011 · Dust rule

- **Phase:** 3 · **Status:** Pending
- **Spec (§3):** decide whether `sweep_dust` burns the rounding dust or adds it to the pool.
- **Note:** adding to the pool depends on whether the AMM accepts deposits into a permanently locked position (D-002). Burning is simpler. Decide in Phase 3 and log it here.

### D-012 · RPC and webhook provider

- **Phase:** 5 · **Status:** Pending
- **Spec (§4):** Helius or similar; keys only in env. Needs docs access and a key.

## B. Spec questions (need the user's answer)

Each is a gap or an ambiguity in SPEC.md. The proposals are suggestions only; nothing changes until the user approves.

### Q-01 · `pool_costs` has no home and must be known at birth

- **Needed by:** Phase 3 · **Severity:** high
- **Issue:** §3's birth math uses `pool_costs`, but it isn't in the defaults table or Config. `birth_candidate` fixes the allocations first, and `seed_pool` pays the real cost later. If the real cost differs from the number used at birth, the pool price drifts from the clearing price and invariant 3 breaks.
- **Proposal:** make `pool_costs` a Config parameter, or compute it at birth from the AMM's own config account plus the rent sysvar, whichever D-002 shows to be deterministic. `seed_pool` aborts if the actual cost differs.

### Q-02 · Pool-address squatting if tokens can be claimed before the pool exists

- **Needed by:** Phase 3 and 4 · **Severity:** high (touches the "users can always exit" rule)
- **Issue:** §5 allows `claim` after `birth_candidate`, while `seed_pool` is a separate step. If the AMM derives the pool address from the token pair (to confirm in D-002), anyone holding claimed tokens could create the (token, wrapped SOL) pool first at a hostile price and block or corrupt `seed_pool`.
- **Proposal:** `claim` requires the candidate to be pool-seeded, with a timed fallback (claims open after a grace period even if seeding keeps failing, so users can always exit). Alternative: make birth and seeding one atomic step.

### Q-03 · What is `max_resolution_delay` measured from?

- **Needed by:** Phase 2 · **Severity:** medium
- **Issue:** §3 says "after this, anyone can void" without a reference time.
- **Proposal:** measure from `expected_resolution` (set by the curator in `create_event`).

### Q-04 · Nothing moves a candidate to Refunding

- **Needed by:** Phase 1 · **Severity:** medium
- **Issue:** the status machine has Pending → Refunding, but no instruction performs it, and `finalize_resolution` can't walk an unbounded set of candidates.
- **Proposal:** derive refundability inside `refund`: the event is Voided, or Resolved with the candidate on the losing outcome, or the candidate's total is below `min_raise`. The first refund flips Pending → Refunding. The SDK and indexer expose an "effective status". Alternative: a permissionless `mark_refunding` crank.

### Q-05 · Invariants 6 and 9, and several §9 tests, have no phase

- **Needed by:** Phase 1 · **Severity:** medium
- **Issue:** §11 assigns invariants 1 to 5, 7 and 8, but not 6 (pause never blocks exits) or 9 (roles and idempotent cranks). Some §9 tests (ticker canon cross-language fixtures, property tests, `anchor test` with cloned programs) aren't tied to a phase either. Invariants 4 and 5 also contain clauses that can't be tested until birth and claim exist (Phase 3).
- **Proposal:** the placement under "Proposed additions" in PROGRESS.md.

### Q-06 · Fee-split edge cases

- **Needed by:** Phase 4 · **Severity:** medium
- **Issue:** (a) with no co-proposers, where does the 20% slice go? (b) where does the rounding remainder of the 30/20/50 split go?
- **Proposal:** (a) the treasury, so nobody can gain by being the only co-proposer; (b) the treasury.

### Q-07 · Who pays birth-time rent and any metadata protocol fee?

- **Needed by:** Phase 3 · **Severity:** medium
- **Issue:** `birth_candidate` creates the mint, the token vault and the metadata account. Their rent (and any fee the metadata program charges, to verify in the docs) can't come from the vault without breaking invariant 3.
- **Proposal:** pay them out of the 1% success fee: the treasury receives the fee minus birth costs, and the caller is made whole. Alternative: the caller pays and is funded by the treasury off-chain.

### Q-08 · `lp_tokens` rounding direction

- **Needed by:** Phase 3 · **Severity:** low
- **Issue:** §3 says round user payouts down, but flooring `lp_tokens` rounds `committer_tokens` up by less than one base unit.
- **Proposal:** round `lp_tokens` up, so `committer_tokens` and every allocation round down. The pool price then differs from the clearing price by at most one base unit.

### Q-09 · "Full refund" copy and the proposal fee

- **Needed by:** Phase 7 · **Severity:** low
- **Issue:** §7 requires "Full refund if [other outcome] happens" on every commit screen, but the proposal fee is non-refundable, including on refunding and voided candidates.
- **Proposal:** the proposal screen states the fee is non-refundable; commit screens for existing coins keep the §7 line.

### Q-10 · Pause scope for `co_propose`

- **Needed by:** Phase 1 · **Severity:** low
- **Issue:** §5 says `set_pause` pauses "proposals and/or commits only". `co_propose` creates a share and commits SOL, so it is both.
- **Proposal:** `co_propose` is blocked when either flag is set.

## C. Registry snapshot (observation, not a pin)

Read from the public registry APIs on 2026-11-05 (crates.io `max_stable_version`; npm `latest`). "Updated" is the registry's last-modified date. **The latest version of each package is not necessarily a compatible set.** Compatibility notes still have to be read (D-001). Anchor's crate version (1.x) is newer than what I remember, so no Anchor or AMM code gets written from memory.

| Package | Registry | Version | Updated |
| --- | --- | --- | --- |
| anchor-lang, anchor-spl, anchor-cli | crates.io | 1.2.2 | 2026-11-03 |
| litesvm | crates.io | 0.18.2 | 2026-11-04 |
| solana-verify | crates.io | 0.5.2 | 2026-09-21 |
| trident-cli, trident-fuzz | crates.io | 0.12.0 | 2026-05-14 |
| mpl-token-metadata | crates.io | 5.1.1 | 2025-11-13 |
| spl-token | crates.io | 9.0.0 | 2026-04-02 |
| solana-program | crates.io | 5.2.0 | 2026-10-29 |
| @coral-xyz/anchor | npm | 0.32.1 | 2025-10-10 (older than the 1.x crates; check the Anchor docs for the current TS client) |
| @solana/kit | npm | 8.6.0 | 2026-11-04 |
| @solana/web3.js | npm | 3.1.1 | 2026-10-19 |
| codama / @codama/cli / @codama/renderers-js | npm | 1.11.0 / 1.6.3 / 2.8.0 | 2026-10-20 |
| litesvm | npm | 1.6.2 | 2026-11-04 |
| @metaplex-foundation/mpl-token-metadata | npm | 3.4.0 | 2025-10-28 |
| @squads-protocol/multisig | npm | 2.1.2 | 2024-08-30 |
| turbo | npm | 2.11.10 | 2026-10-29 |
| next / react | npm | 16.4.5 / 19.3.1 | 2026-11-04 / 2026-10-31 |
| fastify | npm | 5.12.8 | 2026-11-02 |
| drizzle-orm / drizzle-kit | npm | 0.45.4 / 0.31.11 | 2026-10-21 |
| bullmq | npm | 6.7.0 | 2026-11-04 |
| @solana/wallet-adapter-react | npm | 0.15.40 | 2026-09-10 |
| playwright / @playwright/test | npm | 1.64.1 | 2026-11-05 |
| vitest | npm | 5.1.0 | 2026-11-02 |
| typescript | npm | 7.0.2 | 2026-11-04 |
| @sentry/node | npm | 11.10.0 | 2026-10-28 |

Installed in the sandbox (checked 2026-11-05): Node v22.23.4, pnpm 10.28.0, npm 10.9.9, rustc 1.97.0 (2026-07-07), cargo 1.97.0, rustup 1.28.2, git 2.43.0, psql 16.15 (server binaries present), redis-server 7.0.15, Docker CLI 29.9.2 (no daemon running). Not installed: Solana/Agave CLI, `solana-test-validator`, Anchor CLI, `solana-verify`.
