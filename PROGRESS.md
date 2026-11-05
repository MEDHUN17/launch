# PROGRESS

**Current phase:** 0 — Setup and spikes
**State:** plan shown, awaiting the user's go-ahead. No code written yet.
**Last updated:** 2026-11-05

Tick a box only when its criterion passes (tests green, or the artifact exists and was checked). Each phase lists SPEC §11's "Build" and "Done when" text for reference; the checkboxes are the gate. Each session ends with a log line at the bottom.

## Blockers

- [ ] **Network policy denies hosts Phase 0 needs** (checked 2026-11-05). Package registries work (crates.io, npm, PyPI). Denied: `release.anza.xyz`, `api.devnet.solana.com`, `www.anchor-lang.com`, `solana.com`, `docs.meteora.ag`, `docs.raydium.io`, `developers.metaplex.com`, `docs.kalshi.com`, `api.elections.kalshi.com`, `demo-api.kalshi.co`, `docs.pinata.cloud`, `api.pinata.cloud`, `docs.squads.so`, `pubapi.dflow.net`. The user must widen the environment's Network access (cloud environment menu in the session title bar, then Edit, then Network access). Blocks tasks 0.1 and 0.4–0.7. Does not block 0.2 and 0.3.
- [ ] **Toolchain missing in the sandbox:** Solana/Agave CLI, Anchor CLI, `solana-verify`. Present: Rust 1.97.0, Node 22.23.4, pnpm 10.28.0. No Docker daemon. Covered by task 0.1.

## Phase 0 — Setup and spikes

Build: Monorepo, pinned toolchain, CLAUDE.md, CI skeleton. Spikes: AMM choice (create a pool, lock the LP and claim fees by CPI on localnet); Kalshi API (list markets, read a settled one); DFlow on-chain settlement (research only); media storage.
Done when: Spike scripts run; four decisions logged.

- [ ] Spike scripts run (AMM, Kalshi, media storage; DFlow is research only)
- [ ] Four decisions logged in `docs/DECISIONS.md`: D-002 AMM, D-003 Kalshi, D-004 DFlow, D-005 media storage

Proposed task breakdown (awaiting approval):

- [x] CLAUDE.md, PROGRESS.md, docs/DECISIONS.md created (kickoff)
- [ ] 0.1 Bootstrap the toolchain with an idempotent `scripts/bootstrap.sh`: Agave/Solana CLI and platform tools, Anchor CLI, `solana-verify`. Read compatibility notes, then pin versions (D-001). *Needs network.*
- [ ] 0.2 Monorepo skeleton per SPEC §4: pnpm workspaces and Turborepo, package stubs, `.gitignore` (keypairs, `.env`, build output), `.env.example`, `rust-toolchain.toml`, `.nvmrc`, plus a compile-only Anchor program stub to prove the toolchain. Replace the "planned" commands in CLAUDE.md with verified ones.
- [ ] 0.3 CI skeleton (GitHub Actions): clippy, eslint, typecheck, unit tests, web build on every push.
- [ ] 0.4 Spike A, AMM: Meteora DAMM v2 and Raydium CPMM on localnet. Create the pool at the clearing price by CPI from a PDA, lock the LP permanently, claim fees by CPI. Log D-002 and the inputs to Q-01 and Q-02. *Needs network.*
- [ ] 0.5 Spike B, Kalshi: read docs and API terms, list open markets, read a settled market, hash the response as evidence, save fixtures. Log D-003. *Needs network.*
- [ ] 0.6 Research C, DFlow: can the program read tokenized Kalshi settlement on-chain? Research only, no code. Log D-004. *Needs network.*
- [ ] 0.7 Spike D, media storage: IPFS (Pinata) vs Arweave. Log D-005. *Needs network; a Pinata key is optional.*
- [ ] 0.8 Close-out: log D-006 to D-008, update PROGRESS.md and DECISIONS.md.

## Phase 1 — Program core

Build: Config, events, propose with Canon, co-propose, commit, close, void, refund.
Done when: Happy and failure tests for every instruction; invariants 1, 2, 4, 5 and 8 pass.

- [ ] Happy-path test for every Phase 1 instruction: `initialize_config`, `update_config`, `set_pause`, `create_event`, `propose_candidate`, `co_propose`, `commit`, `close_window`, `void_event` (admin path; the delay-void path is Phase 2), `refund`
- [ ] Failure-path test for every Phase 1 instruction
- [ ] Invariant 1: the sum of Commitment amounts equals the candidate's total committed
- [ ] Invariant 2: a refunding candidate returns exactly each commit, and its vault ends at the rent-exempt minimum
- [ ] Invariant 4: a Commitment is claimed or refunded, never both, at most once (refund side only until `claim` exists in Phase 3)
- [ ] Invariant 5: no proposal or commit after close; a voided event only refunds (the birth clauses are re-checked in Phase 3)
- [ ] Invariant 8: at most one candidate per (event, outcome, canon hash)

## Phase 2 — Resolution

Build: Propose, dispute, settle, finalize, delay void, evidence hash.
Done when: Clock-warp tests cover every status path.

- [ ] Clock-warp tests cover every EventState path (Open → Closed → ResolutionProposed → Resolved; ResolutionProposed → Disputed → Resolved or Voided; every void path) and reject every other transition, including the exact boundary seconds

## Phase 3 — Birth and claims

Build: Mint, metadata, fee, allocations, claim, dust rule.
Done when: Invariants 3 and 7 pass.

- [ ] Invariant 3: fee + pool costs + pool SOL = total, committer tokens + LP tokens = supply, and no mint authority remains
- [ ] Invariant 7: every committer gets the same tokens per lamport, up to floor rounding

## Phase 4 — Pool and fees

Build: `seed_pool` by CPI, LP lock, harvest, creator fee claims.
Done when: Works on localnet with the cloned AMM; fee split exact to the lamport.

- [ ] `seed_pool` creates the pool by CPI on localnet with the cloned AMM, and the LP is locked permanently
- [ ] `harvest_fees` and `claim_creator_fees` work on localnet
- [ ] The fee split (30% proposer, 20% co-proposers, 50% treasury) is exact to the lamport

## Phase 5 — SDK, indexer, DB, API

Build: Generated client, schema, log decoding, endpoints.
Done when: API shows the correct live state for a scripted event.

- [ ] API shows the correct live state for a scripted event

## Phase 6 — Keeper and resolver

Build: Ingestion, cranks, resolution, reconciliation, alerts.
Done when: A scripted event runs end to end, unattended, on localnet.

- [ ] A scripted event runs end to end, unattended, on localnet

## Phase 7 — Frontend

Build: Every page in SPEC §7.
Done when: Playwright happy and refund paths pass.

- [ ] Playwright happy path: propose → commit → resolve → birth → claim
- [ ] Playwright refund paths (every one)

## Phase 8 — Admin and compliance

Build: Console, geoblock, terms, filters.
Done when: Admin can list, hide, resolve and pause.

- [ ] Admin can list an event
- [ ] Admin can hide a candidate
- [ ] Admin can resolve (resolution panel and dispute queue)
- [ ] Admin can pause

## Phase 9 — Hardening

Build: Fuzzing, self-review against the SPEC §5 rules, load test, threat model.
Done when: No open high-severity findings.

- [ ] No open high-severity findings (Trident fuzzing, self-review, threat model reviewed; load test of 1,000 commits across 50 candidates, then every claim and refund succeeds)

## Phase 10 — Devnet beta

Build: Deploy everything; run three events on real schedules.
Done when: Full lifecycle seen on devnet; RUNBOOK.md complete; mainnet checklist drafted, not run.

- [ ] Full lifecycle seen on devnet (three events on real schedules)
- [ ] `docs/RUNBOOK.md` complete (dispute, void, pause, key rotation)
- [ ] Mainnet checklist drafted, not run

## Proposed additions (not criteria until the user approves)

SPEC §11 doesn't assign these to a phase's "Done when". See `docs/DECISIONS.md` Q-05.

- Invariant 6 (pause never blocks claim, refund or `claim_creator_fees`): test `refund` in Phase 1, `claim` in Phase 3, `claim_creator_fees` in Phase 4.
- Invariant 9 (only the resolver proposes outcomes, only the admin settles disputes, cranks are permissionless and idempotent): roles in Phase 2; idempotency of each crank in the phase that adds it, and again in Phase 6.
- Ticker canon: Rust unit tests and the TypeScript/Rust shared-fixture hash test in Phase 1, since `propose_candidate` computes the canon hash on-chain.
- Birth-math unit tests in Phase 3. Property tests start in Phase 1; Trident fuzzing in Phase 9.

## Session log

- 2026-11-05 — Kickoff. Read SPEC.md in full; saved it unchanged as SPEC.md; created CLAUDE.md, PROGRESS.md and docs/DECISIONS.md; surveyed the sandbox (see Blockers); drafted the Phase 0 plan; stopped for the user's go-ahead.
