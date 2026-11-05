# PROGRESS

**Current phase:** 1 — Program core · **State:** built and tested. All instructions are in, 78 tests pass in LiteSVM (22 unit, 56 integration including the invariant random walk), fmt, `anchor build` and clippy are clean. A second independent code review is being triaged. Phase 2 starts with a plan for your approval; the items under "Needs your decision" are small and none blocks it.
**Last updated:** 2026-11-05 (session 1)

Tick a box only when its criterion passes (tests green, or the artifact exists and was checked). Each phase lists SPEC §11's "Build" and "Done when" text for reference; the checkboxes are the gate. Each session ends with a log line at the bottom.

## Blockers and open items

- [x] **Network** — resolved 2026-11-05 (Network access set to Full). Remaining limits: kalshi.com and solana.com show a browser-verification checkpoint to scripted clients; GitHub outside this repo is scoped (403).
- [ ] **Kalshi API and data terms need a human reader** (D-003). The sandbox cannot read them. Until they are reviewed, no Kalshi payloads are committed.
- [ ] **Pinata API key** for an upload test (optional, D-005).
- [x] **First green remote CI run** — run 41987267156 passed both jobs (the first run failed on a Rust-version mismatch, since fixed). Action versions are still unverified and GitHub warns about Node 20 actions (D-001).

## Decisions (answered 2026-11-05)

Approved for Phase 1: freeze each event's economic settings with hard caps (Q-11, caps in D-013); co-propose pays the proposal fee and its weight is frozen when joining (Q-14); admin `reject_candidate`, attestation covering name/ticker/URI with the beta running with attestation on, and frozen ticker table v1 with ASCII-only input (Q-13, Q-16, Q-19); atomic launch (Q-02, built in Phase 3/4).

Still open, needed in later phases: Q-15 (trading delay and exit math), Q-07 (who pays birth rent), Q-17 (delay-void vs dispute), Q-18, Q-06. Kalshi's terms still need a human reader; a Pinata key is optional.

## Needs your decision

Small, none blocks starting Phase 2 (details and reasons in `docs/DECISIONS.md` Q-21 and `docs/PHASE1_DESIGN.md` section 9):

1. **Cap values (D-013):** `min_raise` at least 0.1 SOL, 5 minutes minimum between close and event start, 60-day window limit, `proposal_fee` at most 1 SOL, the rest as listed. Confirm or give me different numbers.
2. **`proposal_cutoff`:** new setting, default 0 (the spec behaviour). Set per beta event to leave the admin time to reject a bad candidate before commits stop. Do you want a non-zero default?
3. **Pause scope:** proposing and co-proposing stop under either pause flag; plain commits only under `pause_commits`.
4. **Relisting after a void:** today a voided market can never be listed again. Allow it?
5. **URI cap:** 128 bytes of printable ASCII (Metaplex allows 200).
6. **New dependencies (D-016):** two Solana SDK crates in the program and eight test-only crates, all listed with versions.

Interpretations, not spec changes (say if you disagree): the Phase 1 invariant 4 and 5 items are narrowed to what can be tested before `claim` and birth exist; the Phase 4 item says "or the atomic launch"; the Phase 9 item lists the Build text in brackets.

## Phase 0 — Setup and spikes

Build: Monorepo, pinned toolchain, CLAUDE.md, CI skeleton. Spikes: AMM choice (create a pool, lock the LP and claim fees by CPI on localnet); Kalshi API (list markets, read a settled one); DFlow on-chain settlement (research only); media storage.
Done when: Spike scripts run; four decisions logged.

- [x] Spike scripts run: `pnpm spike:amm` (9 tests: 8 LiteSVM against the real devnet binary, 1 IDL check), `pnpm spike:kalshi` (live read-only API), `pnpm spike:media` (dry run only: the upload path is untested, no Pinata key). DFlow is research only.
- [x] Four decisions logged in `docs/DECISIONS.md`: D-002 AMM (provisional), D-003 Kalshi, D-004 DFlow (inconclusive), D-005 media storage (provisional).

Task breakdown:

- [x] CLAUDE.md, PROGRESS.md, docs/DECISIONS.md created (kickoff)
- [x] 0.1 Toolchain bootstrap: `scripts/bootstrap.sh` (Agave 4.3.2, Anchor CLI 1.2.2, solana-verify 0.5.2), pins in D-001
- [x] 0.2 Monorepo skeleton per SPEC §4 and a compile-only Anchor stub (`ping`), tested through LiteSVM
- [x] 0.3 CI workflow (`.github/workflows/ci.yml`): JS job and program job. Green on GitHub in run 41987267156 after one fix; hardening is listed under "Carried forward"
- [x] 0.4 Spike A, AMM: Meteora DAMM v2 on localnet — pool created, locked and fees claimed by CPI from PDAs; squatting shown; Raydium compared from docs and devnet reads only
- [x] 0.5 Spike B, Kalshi: live listing, settled-market read, evidence hash, resolver mapping
- [x] 0.6 Research C, DFlow: inconclusive, with a v1 sketch and the questions for DFlow
- [x] 0.7 Spike D, media: Pinata chosen provisionally; upload untested
- [x] 0.8 Close-out: decisions and spec questions logged, docs updated

Carried forward into Phase 1:
- Run the atomic launch once on `solana-test-validator` with devnet clones, and confirm devnet accepts SBPFv3 programs (or build with `--arch`); the spike used LiteSVM with every feature enabled (D-001).
- Convert the observational probes in `probe2.rs` (pre-funding, post-lock, overfunding) to asserts.
- CI hardening: save the toolchain cache right after bootstrap (`cache/restore` + `cache/save`), cache `target` keyed on `Cargo.lock`, `timeout-minutes`, cancel in-progress runs only off `main`, and avoid running both `push` and `pull_request`.
- Replace the throwaway AMM spike with the real launch; verify the Metaplex field limits and fee in Phase 3; read Kalshi's terms before committing any Kalshi data.

## Phase 1 — Program core

Build: Config, events, propose with Canon, co-propose, commit, close, void, refund.
Done when: Happy and failure tests for every instruction; invariants 1, 2, 4, 5 and 8 pass.

- [x] Happy-path test for every Phase 1 instruction: `initialize_config`, `update_config`, `set_pause`, `create_event`, `propose_candidate`, `co_propose`, `commit`, `close_window`, `void_event` (admin path; the delay-void path is Phase 2), `reject_candidate`, `refund`. Evidence: `scripts/rust-checks.sh`, 78 tests green (files `config_events`, `propose_commit`, `lifecycle`)
- [x] Failure-path test for every Phase 1 instruction: wrong signer or role, bad input, wrong window, paused, wrong status, swapped accounts; each failure also checks that nothing moved (`config_events`, `propose_commit`, `review_hardening`, `lifecycle`)
- [x] Invariant 1: the sum of Commitment amounts equals the candidate's total committed (reworded per Q-05: open commitments + refunded total = total committed, checked after every step of the random walk)
- [x] Invariant 2: a refunding candidate returns exactly each commit, and its vault ends at the rent-exempt minimum (plus any donations, which never block the last refund)
- [x] Invariant 4: a Commitment is claimed or refunded, never both, at most once (refund side only until `claim` exists in Phase 3)
- [x] Invariant 5: no proposal or commit after close; a voided event only refunds (the birth clauses are re-checked in Phase 3)
- [x] Invariant 8: at most one candidate per (event, outcome, canon hash)

Evidence for the invariants: `programs/prequel/tests/invariants.rs` (16 seeded walks of 80 steps; coverage floors; two deliberate bugs confirmed to fail it) plus the named tests in `docs/PHASE1_DESIGN.md` section 7. The invariant 6 check (pause never blocks refund) also passes: `neither_pause_flag_blocks_refund_reject_void_or_close`.

Open until the code review is triaged: any defect it finds gets a test and a fix before Phase 2 starts.

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
- 2026-11-05 — Phase 0. (CI is green on GitHub; a third Opus review fact-checked this write-up and its corrections are applied.) You opened the network. Installed and pinned the toolchain; built the monorepo skeleton, Anchor stub and CI; ran the Kalshi, AMM and media spikes; two independent Opus reviews (spec and AMM) folded into DECISIONS.md. Key results: the Meteora create+lock+claim flow works by CPI from PDAs, pool squatting works without holding the coin, and the pool part of an atomic launch (mint, supply, revoke, wrap, pool, lock) takes 153k CU and 22 accounts; the full launch will likely need a v0 transaction. After the lock the owner can still `split_position` the liquidity away, so the program must never sign a split.
- 2026-11-05 — Phase 1. You approved the four design questions (Q-02, Q-11, Q-13/16/19, Q-14). Wrote the design note, had it reviewed by an independent Opus (applied: account layouts with reserved tails, an attestation that signs a digest so a maximum proposal fits in one packet, the rent-safe program-owned vault, hard caps, many extra tests). Built the shared ticker table (Rust and TypeScript, 35 shared fixtures), the program (11 instructions), a LiteSVM harness with a real Ed25519 precompile, and the invariant random walk; checked that the walk fails when the program is deliberately broken. Docs updated: `docs/PHASE1_DESIGN.md` (as built), D-009 to D-018 and Q-21 in `docs/DECISIONS.md`.
