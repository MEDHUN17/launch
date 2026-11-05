# PROGRESS

**Current phase:** 0 — Setup and spikes · **State:** Phase 0 work is done and pushed; the spec questions in `docs/DECISIONS.md` section B need your answers before Phase 1 code.
**Last updated:** 2026-11-05 (session 1)

Tick a box only when its criterion passes (tests green, or the artifact exists and was checked). Each phase lists SPEC §11's "Build" and "Done when" text for reference; the checkboxes are the gate. Each session ends with a log line at the bottom.

## Blockers and open items

- [x] **Network** — resolved 2026-11-05 (Network access set to Full). Remaining limits: kalshi.com and solana.com show a browser-verification checkpoint to scripted clients; GitHub outside this repo is scoped (403).
- [ ] **Kalshi API and data terms need a human reader** (D-003). The sandbox cannot read them. Until they are reviewed, no Kalshi payloads are committed.
- [ ] **Pinata API key** for an upload test (optional, D-005).
- [ ] **First remote CI run** — the workflow is written and every step passes locally; confirm on GitHub (action versions are unverified, D-001).

## Needs your decision before Phase 1 code

These come from my own review and from two independent Opus reviews (details in `docs/DECISIONS.md` section B). Phase 1 fixes account layouts, so they are cheap now and expensive later.

1. **Q-02 Atomic launch (critical).** Verified on localnet: someone holding none of the coin can create the (coin, wSOL) pool on Meteora's 1% config first, which would strand about 99% of a raise. Proposal: mint, wrap, create pool, lock, revoke authority and mark Born in one instruction, plus a birth deadline that turns an unlaunched winner into a refund. This amends §5. Measured: that whole launch fits one legacy transaction (153k CU, 22 accounts, 820 bytes), before Metaplex metadata, which adds about 3 accounts and needs a short URI or a v0 transaction.
2. **Q-11 Snapshot and bound parameters per event** so the admin cannot change fees, delays or bonds on a live event.
3. **Q-14 Co-proposer economics** (`co_propose` currently beats `commit` for free).
4. **Q-13, Q-16, Q-19 Slate and Canon policy** (slot grabbing, attestation scope, confusables table versioning).
5. **D-010 / Q-05 Vault model and rewritten invariants.**
6. Also answer: D-002 (Meteora, provisional), Q-15 (delay trading after launch; exit math), Q-07 (who pays birth rent).

## Phase 0 — Setup and spikes

Build: Monorepo, pinned toolchain, CLAUDE.md, CI skeleton. Spikes: AMM choice (create a pool, lock the LP and claim fees by CPI on localnet); Kalshi API (list markets, read a settled one); DFlow on-chain settlement (research only); media storage.
Done when: Spike scripts run; four decisions logged.

- [x] Spike scripts run: `pnpm spike:amm` (3 LiteSVM tests against the real devnet binary), `pnpm spike:kalshi` (live read-only API), `pnpm spike:media` (dry run only: the upload path is untested, no Pinata key). DFlow is research only.
- [x] Four decisions logged in `docs/DECISIONS.md`: D-002 AMM (provisional), D-003 Kalshi, D-004 DFlow (inconclusive), D-005 media storage (provisional).

Task breakdown:

- [x] CLAUDE.md, PROGRESS.md, docs/DECISIONS.md created (kickoff)
- [x] 0.1 Toolchain bootstrap: `scripts/bootstrap.sh` (Agave 4.3.2, Anchor CLI 1.2.2, solana-verify 0.5.2), pins in D-001
- [x] 0.2 Monorepo skeleton per SPEC §4 and a compile-only Anchor stub (`ping`), tested through LiteSVM
- [x] 0.3 CI workflow (`.github/workflows/ci.yml`): JS job and program job. Passes locally; not yet observed on GitHub
- [x] 0.4 Spike A, AMM: Meteora DAMM v2 on localnet — pool created, locked and fees claimed by CPI from PDAs; squatting shown; Raydium compared from docs and devnet reads only
- [x] 0.5 Spike B, Kalshi: live listing, settled-market read, evidence hash, resolver mapping
- [x] 0.6 Research C, DFlow: inconclusive, with a v1 sketch and the questions for DFlow
- [x] 0.7 Spike D, media: Pinata chosen provisionally; upload untested
- [x] 0.8 Close-out: decisions and spec questions logged, docs updated

Carried forward: pin the first remote CI run; replace the throwaway AMM spike with the Phase 4 `seed_pool`/atomic launch; verify the Metaplex field limits and fee in Phase 3; read Kalshi's terms before committing any Kalshi data.

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

Invariants 1, 2 and 3 are worded in a way that donations and AMM rounding break; see Q-05 for the proposed replacements.

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

- [ ] `seed_pool` (or the atomic launch) creates the pool by CPI on localnet with the cloned AMM, and the LP is locked permanently
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

## Proposed additions (not criteria until you approve)

SPEC §11 doesn't assign these to a phase's "Done when". See `docs/DECISIONS.md` Q-05.

- Invariant 6 (pause never blocks claim, refund or `claim_creator_fees`): test `refund` in Phase 1, `claim` in Phase 3, `claim_creator_fees` in Phase 4. Reworded per Q-05: no admin action can stop a committer reaching claim or refund.
- Invariant 9 (only the resolver proposes outcomes, only the admin settles disputes, cranks are permissionless and idempotent): roles in Phase 2; idempotency of each crank in the phase that adds it, and again in Phase 6.
- New invariants from the review: solvency, liveness (with admin and resolver silent every candidate reaches Born-with-pool or Refunding), fee conservation per asset, forward-only statuses, "Born implies a pool exists".
- Ticker canon: Rust unit tests and the TypeScript/Rust shared-fixture hash test in Phase 1, since `propose_candidate` computes the canon hash on-chain.
- Birth-math unit tests in Phase 3. Property tests start in Phase 1; Trident fuzzing in Phase 9.

## Session log

- 2026-11-05 — Kickoff. Read SPEC.md; saved it unchanged; created CLAUDE.md, PROGRESS.md and docs/DECISIONS.md; drafted the Phase 0 plan.
- 2026-11-05 — Phase 0. You opened the network. Installed and pinned the toolchain; built the monorepo skeleton, Anchor stub and CI; ran the Kalshi, AMM and media spikes; two independent Opus reviews (spec and AMM) folded into DECISIONS.md. Key results: the Meteora create+lock+claim flow works by CPI from PDAs, pool squatting works without holding the coin, and the fully atomic launch (mint, supply, revoke, wrap, pool, lock) fits one transaction at 153k CU. After the lock the owner can still `split_position` the liquidity away, so the program must never sign a split.
