# Phase 1 design: program core

Status: draft for review, 2026-11-05. Scope: `initialize_config`, `update_config`, `set_pause`, `create_event`, `propose_candidate`, `co_propose`, `commit`, `close_window`, `void_event` (admin), `reject_candidate`, `refund`. Inputs: SPEC §2, §3, §5; decisions D-010, D-013, D-014, D-015 and Q-02, Q-11, Q-13, Q-14, Q-16, Q-19 in `docs/DECISIONS.md`.

Out of scope here: resolution (Phase 2), birth/launch and claims (3/4). Fields those phases need are reserved now so no account has to be reallocated.

## 1. Constants

```
SEEDS: config | event+u64le | ext+sha256(external_id) | candidate+event+outcome+canon_hash | vault+candidate
       | commit+candidate+wallet | coprop+candidate+wallet
MAX_OUTCOMES 8 · name <= 32 bytes · ticker 2..=10 · uri <= 200 bytes · external_id <= 64 bytes · CANON_VERSION 1
Hard caps (D-013): success_fee_bps <= 500 · dispute_window 60s..7d · max_resolution_delay 1d..30d · launch_window 10m..7d
  dispute_bond 0.01..10 SOL · proposal_fee 0.001..1 SOL · min_commit >= 0.001 SOL · min_commit <= min_raise <= 1000 SOL
  max_candidates_per_outcome 1..=32 · fee split sums to 10,000 · expected_resolution in [event_start, event_start + 90d]
```

## 2. Accounts

All program-owned, Anchor `#[account]` with `InitSpace`. `Params` is shared by Config (defaults for new events) and EventState (frozen copy).

```
Params { min_commit u64, max_commit_per_wallet u64 (0 = no cap), min_raise u64, max_candidates_per_outcome u8,
         dispute_window i64, dispute_bond u64, max_resolution_delay i64, launch_window i64, proposal_fee u64,
         success_fee_bps u16, split_proposer_bps u16, split_coprop_bps u16, split_treasury_bps u16,
         require_attestation bool }

Config        [config]            admin, resolver, canonicalizer, treasury: Pubkey; params: Params;
                                  pause_proposals, pause_commits: bool; version u8; next_event_id u64; bump
EventState    [event, id]         id u64; source u8 (0 = Kalshi); external_id_hash [u8;32]; outcome_count u8;
                                  window_open, window_close, event_start, expected_resolution: i64;
                                  status EventStatus; proposed_outcome, resolved_outcome: u8 (255 = none);
                                  evidence_hash [u8;32]; dispute_deadline i64; disputer Pubkey; resolved_at i64;
                                  launch_deadline i64; candidate_count [u8;8]; params: Params (frozen);
                                  canon_version u8; created_at i64; bump
ExternalListing [ext, hash]       event Pubkey; external_id_hash [u8;32]; bump
Candidate     [candidate, event, outcome, canon_hash]
                                  event Pubkey; outcome u8; canon_hash [u8;32]; proposer Pubkey;
                                  name String(32); ticker String(10); uri String(200); image_hash [u8;32];
                                  total_committed u64; committer_count u32; co_proposer_total u64; co_proposer_count u16;
                                  refunded_total u64; status CandidateStatus; created_at i64;
                                  reserved for Phase 3/4: mint, pool: Pubkey; committer_tokens, pool_tokens, claimed_total: u64;
                                  bump; vault_bump
Vault         [vault, candidate]  candidate Pubkey; bump        (program-owned; holds the candidate's SOL above its rent reserve)
Commitment    [commit, candidate, wallet]
                                  candidate, wallet: Pubkey; amount u64; first_commit_ts i64; bump   (closed on refund/claim)
CoProposerShare [coprop, candidate, wallet]
                                  candidate, wallet: Pubkey; weight u64 (frozen at join); claimed u64; reward_debt u128; joined_at i64; bump

EventStatus     Open | Closed | ResolutionProposed | Disputed | Resolved | Voided
CandidateStatus Pending | Born | Refunding | Rejected
```

`ExternalListing` stops the same market being listed twice; it is never closed in Phase 1 (Q-20 follow-up).

## 3. Pure logic (no Anchor types, unit-tested)

- `effective_event_status(event, now)`: stored `Open` and `now >= window_close` reads as `Closed`; every other stored status is returned as is. The stored value is a cache that cranks update; **every check uses the effective value** (Q-04).
- `accepts_commits(event, now)`: effective status `Open` and `now < window_close` and `window_open <= now`. At `now == window_close` commits are rejected.
- `validate_params(p)`: every cap in section 1.
- `validate_event_times(now, window_open, window_close, event_start, expected_resolution)`.
- `canonical_ticker_hash(ticker) -> Result<[u8;32]>`: D-015.
- `refundable(event_status, candidate_status)`: true for `Rejected`, `Refunding`, and `Pending` while the event is `Voided`. (Phase 2/3 add the losing-side and below-min-raise cases.)

## 4. Instructions

Common rule: privileged instructions name their role; permissionless ones either do their job once or fail with a named error (never apply twice). Money moves only with checked math; no `remaining_accounts`.

| Instruction | Signer | Order of checks, then effects | Event |
| --- | --- | --- | --- |
| `initialize_config(args)` | deployer = program upgrade authority | `program_data` constraint; `validate_params`; fee split sums; sets admin/resolver/canonicalizer/treasury, defaults, `next_event_id = 0` | ConfigUpdated |
| `update_config(args)` | admin | each key and `params` optional; `validate_params`; affects **future events only** | ConfigUpdated |
| `set_pause(p, c)` | admin | optional flags; never gates claim/refund/reject/void | PauseChanged |
| `create_event(args)` | admin | `validate_event_times`; outcome_count 2..=8; `sha256(external_id) == hash`; creates `EventState` (id = `next_event_id`, **freezes** `config.params`, `canon_version = 1`) and `ExternalListing`; `next_event_id += 1` | EventCreated |
| `propose_candidate(args)` | proposer | pause_proposals off → `accepts_commits` → outcome < outcome_count → name/ticker/uri/format → `canon_hash == canonical_ticker_hash(ticker)` → slate cap (`candidate_count[outcome] < max`) → attestation if `event.params.require_attestation` → `min_commit <= amount <= cap` → pays `proposal_fee` to treasury and `amount` to the vault → inits Candidate, Vault, Commitment; `candidate_count[outcome] += 1` | CandidateProposed |
| `commit(amount)` | wallet | pause_commits off → `accepts_commits` → candidate `Pending` → bounds and per-wallet cap on the running total → `init_if_needed` Commitment → transfer to vault → `total_committed`, `committer_count` (only for a new Commitment) | Committed |
| `co_propose(amount)` | wallet | both pause flags off → as `commit` → wallet is not the proposer → no existing `CoProposerShare` → pays `proposal_fee` to treasury → CoProposerShare (`weight = amount`) → `co_proposer_total += amount`, `co_proposer_count += 1` | CoProposed |
| `close_window()` | anyone | stored `Open` and `now >= window_close` else named error; stored status → `Closed` | WindowClosed |
| `void_event()` | admin | stored status `Open` or `Closed` (Phase 2 adds `Disputed`) → `Voided` | EventVoided |
| `reject_candidate()` | admin | effective event `Open` (before close) → candidate `Pending` → `Rejected`, `candidate_count[outcome] -= 1`; proposal fee stays with the treasury | CandidateRejected |
| `refund()` | wallet | `refundable(...)`; candidate `Pending` → `Refunding` on first refund when the event is `Voided`; pays `commitment.amount` from the vault to the wallet by direct lamport arithmetic (never below the vault's rent reserve); `refunded_total += amount`; closes the Commitment to the wallet; optionally closes the wallet's `CoProposerShare` | Refunded |

Notes:
- `commit` and `co_propose` re-check `accepts_commits` so a stale stored status cannot let money in after close.
- `init_if_needed` on `Commitment` is safe only because the seeds contain the wallet and the handler treats `amount == 0 && first_commit_ts == 0` as fresh; a unit test covers a pre-existing account.
- Attestation (D-014) reads the Instructions sysvar. If `require_attestation` is off, the account is ignored.
- Proposal fee and rent: the proposer pays rent for Candidate, Vault and Commitment through `init`; the proposal fee is separate and non-refundable.

## 5. Invariants as tests

| Id | Spec statement | Test |
| --- | --- | --- |
| 1 | sum of Commitment amounts = total committed | after any sequence of propose/commit/co_propose, and `vault.lamports - rent = total_committed` (plus any donations) |
| 2 | refunds return exactly each commit; vault ends at the rent minimum | all wallets refund; each receives exactly its amount; vault lamports = rent reserve + donations |
| 4 | claimed or refunded, never both, at most once | second refund fails (account closed); a re-created Commitment is impossible after close |
| 5 (subset) | no proposal or commit after close; voided only refunds | boundary seconds T-1, T, T+1; stale stored `Open` after close; commit/propose/co_propose on Voided and Rejected fail |
| 8 | one candidate per (event, outcome, canon hash) | duplicate, case variants, confusable variants (B0B/BOB, 1/I, 5/S), wrong `canon_hash` argument, direct call |
| extra | reworded invariants (Q-05) | `open + refunded_total = total_committed`; solvency; donations do not change payouts; pre-funded PDAs do not block `init` |
| extra | property-style | seeded pseudo-random sequences of commits, co-proposals, rejects, voids and refunds check 1 and 2 after every step |

## 6. Test harness

LiteSVM (D-007). `tests/common`: program load, clock control, funded wallets, PDA helpers, instruction builders, `send` returning the custom error code and logs, Ed25519 instruction builder for attestation tests, a `ProgramData` fixture so `initialize_config` can check the upgrade authority.

## 7. Questions for the reviewer

1. Is a program-owned vault with direct lamport arithmetic safe against donation, rent-state and revival problems?
2. Any hole in `effective_*` use, `init_if_needed` for Commitment, or the order of checks in `propose_candidate`?
3. Is the Ed25519 introspection plan complete (offsets, instruction index fields, signature count, expiry, replay)?
4. Are reserved fields and sizes right for Phase 2 to 4, or will something force a reallocation?
5. Missing failure tests, missing events, or DoS vectors (slate slots, ExternalListing squatting, rent griefing)?
6. Anything in the caps (section 1) that blocks legitimate devnet testing or leaves an exploit open?
