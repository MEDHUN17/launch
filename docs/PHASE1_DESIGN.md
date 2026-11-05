# Phase 1 design: program core (as built)

Status: as built, 2026-11-05. First written as a plan, then revised after an independent design review (every finding applied or logged), then rewritten to match the code. Where this note and the code disagree, the code wins; where the code and `SPEC.md` disagree, `SPEC.md` wins unless a decision in `docs/DECISIONS.md` says otherwise.

Scope: `initialize_config`, `update_config`, `set_pause`, `create_event`, `propose_candidate`, `co_propose`, `commit`, `close_window`, `void_event` (admin), `reject_candidate` (admin), `refund`. Decisions behind it: D-010, D-013, D-014, D-015 and Q-02, Q-04, Q-05, Q-11, Q-13, Q-14, Q-16, Q-19.

Out of scope here: resolution (Phase 2), birth, claims and the permissionless delay-void (Phases 2 to 4). Fields those phases need exist now (reserved), so no account is ever reallocated. Anchor refuses an existing account whose size differs from the declared space, so a layout change after deploy would need a migration instruction.

**Known Phase 1 limit.** The only ways out for a committer are an admin `void_event` or `reject_candidate`. The permissionless void after `max_resolution_delay` (SPEC §5 status machines) arrives in Phase 2. Until then an inactive admin would leave funds in the vaults of a closed event. This is a stage of the build, not a design choice.

## 1. Constants and caps (enforced)

```
SEEDS        config | event+u64le(id) | ext+sha256(external_id) | candidate+event+outcome(1)+canon_hash
             | vault+candidate | commit+candidate+wallet | coprop+candidate+wallet
SIZES        outcomes 2..=8 · name <= 32 bytes · ticker 2..=10 ASCII · uri 1..=128 printable ASCII, no spaces
             external_id 1..=64 bytes · CANON_VERSION 1
ATTESTATION  domain b"PREQUEL-PROPOSE-V1", message 251 bytes, signed as its sha256 digest (section 5)
```

Hard caps, checked by `validate_params` on `initialize_config` and `update_config` (D-013; the values are proposed and need your confirmation):

| Setting | Allowed range | SPEC default |
| --- | --- | --- |
| `success_fee_bps` | 0 to 500 | 100 |
| fee split (proposer + co-proposers + treasury) | must sum to 10,000 bps | 3000 / 2000 / 5000 |
| `dispute_window` | 60 s to 7 d | 2 h |
| `dispute_bond` | 0.01 to 10 SOL | 1 SOL |
| `max_resolution_delay` | 1 d to 30 d | 7 d |
| `launch_window` | 10 min to 7 d | 1 d (new, atomic launch, Q-02) |
| `proposal_fee` | 0.001 to 1 SOL | 0.02 SOL |
| `min_commit` | at least 0.001 SOL | 0.05 SOL |
| `min_raise` | at least `min_commit`, and 0.1 to 1,000 SOL | 5 SOL |
| `max_commit_per_wallet` | 0 (no cap) or at least `min_commit` | 50 SOL |
| `max_candidates_per_outcome` | 1 to 32 | 12 |
| `proposal_cutoff` | 0 to 7 d | 0 (new; 0 means proposals run until close) |

Event times, checked by `validate_event_times` at `create_event`: `window_open <= window_close`; the window closes in the future and lasts at most 60 days; `window_close <= event_start - 5 min`; `event_start <= expected_resolution <= event_start + 90 d`; and a non-zero `proposal_cutoff` must be shorter than the window.

## 2. Accounts

All program-owned, Anchor `#[account]` with `InitSpace` (space is `8 + INIT_SPACE`). Fixed-size fields come first and strings last, so a keeper can filter on status and totals with `memcmp` at fixed offsets. Every long-lived account ends its fixed part with a `reserved` tail.

```
Params          min_commit, max_commit_per_wallet, min_raise: u64 · max_candidates_per_outcome: u8
                dispute_window: i64 · dispute_bond: u64 · max_resolution_delay, launch_window: i64
                proposal_fee: u64 · success_fee_bps, split_proposer_bps, split_coprop_bps, split_treasury_bps: u16
                require_attestation: bool · proposal_cutoff: i64 · reserved [u8;32]
Config          [config]  admin, resolver, canonicalizer, treasury, pending_admin: Pubkey · params: Params (defaults)
                pause_proposals, pause_commits: bool · version, bump: u8 · next_event_id: u64 · reserved [u8;128]
EventState      [event, id]  id: u64 · status · source, outcome_count, canon_version, layout_version,
                proposed_outcome, resolved_outcome, void_reason, bump: u8 · external_id_hash [u8;32]
                window_open, window_close, event_start, expected_resolution, created_at, resolution_proposed_at,
                dispute_deadline, disputed_at, resolved_at, launch_deadline, voided_at: i64 · evidence_hash [u8;32]
                disputer: Pubkey · candidate_count [u8;8] · params: Params (frozen copy) · reserved [u8;128]
                external_id: String (<= 64, last)
ExternalListing [ext, hash]  event: Pubkey · external_id_hash [u8;32] · bump · reserved [u8;16]   (never closed)
Candidate       [candidate, event, outcome, canon_hash]
                status, outcome, layout_version, bump, vault_bump: u8 · event, proposer: Pubkey
                canon_hash, image_hash [u8;32] · total_committed, refunded_total, claimed_commit_total: u64
                committer_count, co_proposer_count: u32 · co_proposer_total: u64 · created_at, born_at: i64
                reserved for Phases 3/4: mint, pool: Pubkey · committer_tokens, pool_tokens, claimed_total,
                success_fee, pool_sol, launch_cost, sol_dust: u64 · reserved [u8;128]
                name: String(32) · ticker: String(10, uppercased as typed) · uri: String(128)   (last)
Vault           [vault, candidate]  candidate: Pubkey · bump · reserved [u8;16]
Commitment      [commit, candidate, wallet]  candidate, wallet: Pubkey · amount: u64 · first_commit_ts: i64 · bump · reserved [u8;16]
CoProposerShare [coprop, candidate, wallet]  candidate, wallet: Pubkey · weight: u64 · claimed: u64 · reward_debt: u128
                joined_at: i64 · bump · reserved [u8;16]

EventStatus      Open | Closed | ResolutionProposed | Disputed | Resolved | Voided
CandidateStatus  Pending | Born | Refunding | Rejected
```

How the money sits: the Vault is a **program-owned data account** (D-010). It holds its own rent plus the candidate's committed SOL, plus any donations. The program pays refunds by debiting its lamports directly, so a stranger sending dust cannot make the last refund fall under the rent-exempt minimum (it could with a system-owned vault). Totals live in state (`total_committed`, `refunded_total`); vault lamports are never read to decide what is owed.

## 3. Pure logic (`logic.rs`, `canon.rs`, `attestation.rs`; no Anchor account types, unit-tested)

- `effective_event_status(stored, window_close, now)`: stored `Open` and `now >= window_close` reads as `Closed`; every other status as stored. The stored value is a cache that `close_window` updates. **Every handler uses the effective value** (Q-04).
- `check_accepts_commits`: effective `Open` and `window_open <= now < window_close`. At `now == window_close` the window is closed.
- `check_accepts_proposals`: the same, and when `proposal_cutoff > 0` also `now < window_close - proposal_cutoff`.
- `check_commit_amount(params, existing, add)`: `add >= min_commit`, checked add, and the per-wallet running total within `max_commit_per_wallet` (0 means no cap).
- `is_refundable(event, candidate)`: `Rejected` and `Refunding` always; `Pending` only once the event is `Voided`; `Born` never. (Phases 2 and 3 add the losing-side and below-`min_raise` cases.)
- `validate_params`, `validate_event_times`, `validate_outcome_count`, `validate_name`, `validate_uri`, `validate_external_id`: section 1.
- `canon::{display_ticker, canonical_ticker, canonical_ticker_hash}` (section 6) and `attestation::{content_hash, expected_message, signed_digest, parse_ed25519, verify}` (section 5).

## 4. Instructions

Common rules: privileged instructions name their role in a `has_one` constraint; permissionless ones either do their job once or fail with a named error. Money moves with checked math only. There is no `remaining_accounts` anywhere. Every program-created PDA tolerates pre-funding (Anchor's `init` falls back to transfer, allocate, assign); a test pre-funds each address the program creates.

| Instruction | Signer | Accounts (besides the system program) | Checks, in order, then effects | Event |
| --- | --- | --- | --- | --- |
| `initialize_config(args)` | deployer | config (init), program, program_data | `program.programdata_address == program_data`; `program_data.upgrade_authority == deployer`; keys non-default; `validate_params`; `next_event_id = 0`, pause flags off | ConfigUpdated |
| `update_config(args)` | admin | config | `has_one = admin`; each key and `params` optional; keys non-default; `validate_params`; **future events only** | ConfigUpdated |
| `set_pause(p, c)` | admin | config | `has_one = admin`; either flag optional. Never gates `refund`, `reject_candidate`, `void_event` or `close_window` | PauseChanged |
| `create_event(args)` | admin | config, event (init, id = `next_event_id`), external_listing (init) | `has_one = admin`; source is Kalshi; `external_id` length; `sha256(external_id) == external_id_hash`; outcome count; `validate_event_times`; cutoff shorter than the window; **freezes `config.params`** into the event; `canon_version = 1`; `next_event_id += 1` | EventCreated |
| `propose_candidate(args)` | proposer | config, event, candidate / vault / commitment (init), treasury, instructions sysvar | `pause_proposals` off, `pause_commits` off → `check_accepts_proposals` → outcome in range → name and uri valid → ticker valid, `canon_hash == sha256(canonical ticker)` → slate not full (`candidate_count[outcome] < max`) → attestation if `event.params.require_attestation` → amount within bounds → proposer pays the proposal fee to the treasury and the amount to the vault | CandidateProposed, Committed |
| `commit(amount)` | wallet | config, event, candidate (`has_one = event`), vault, commitment (`init_if_needed`) | `pause_commits` off → `check_accepts_commits` → candidate `Pending` → fresh iff `commitment.wallet == default` → amount and running cap → `total_committed` and (new wallet only) `committer_count` → transfer to the vault | Committed |
| `co_propose(amount)` | wallet | as `commit`, plus co_proposer_share (init), treasury | both pauses off → wallet is not the proposer → as `commit` → pays the proposal fee → `CoProposerShare` with `weight = amount` (frozen) → `co_proposer_total += amount`, `co_proposer_count += 1` | CoProposed, Committed |
| `close_window()` | anyone | event | stored status `Open` else `EventStateInvalid`; `now >= window_close` else `WindowNotClosedYet`; stored status → `Closed` | WindowClosed |
| `void_event()` | admin | config, event | `has_one = admin`; stored status `Open` or `Closed` → `Voided`, `voided_at = now`, `void_reason = 1` (admin) | EventVoided |
| `reject_candidate()` | admin | config, event, candidate | `has_one = admin`; effective event `Open` (so before close) → candidate `Pending` → `Rejected`; `candidate_count[outcome] -= 1`. The proposal fee stays with the treasury; the ticker stays burned | CandidateRejected |
| `refund()` | wallet | event, candidate, vault, commitment (closed to wallet), co_proposer_share (optional, closed to wallet) | `is_refundable`; a `Pending` candidate becomes `Refunding` on the first refund; `refunded_total + amount <= total_committed`; vault lamports >= amount; `vault.sub_lamports(amount)`, `wallet.add_lamports(amount)`; `refunded_total += amount` | Refunded |

Notes:
- `commit` and `co_propose` re-check the effective status, so a stale stored `Open` cannot let money in after close.
- `init_if_needed` on `Commitment` is safe because its seeds contain the wallet and the candidate: only that wallet can reach it, an existing account is accepted only if it names the same candidate and wallet, and a pre-funded empty address is initialised normally. Closing the account on refund is what makes a second refund impossible; a re-created commitment would need a new commit, which a refunded candidate no longer accepts.
- The proposer pays rent for Candidate, Vault and Commitment through `init`; the proposal fee is separate and non-refundable. A refund also returns the closed accounts' rent to the wallet.
- `committer_count` counts each wallet that ever joined, once; refunds do not reduce it. `co_proposer_total` and `weight` are frozen at the co-propose amount (Q-14).
- A candidate account and its vault are never closed in Phase 1; they stay for the indexer and for the claim and fee logic of later phases.

## 5. Attestation (D-014, version 1.1)

When `event.params.require_attestation` is true (the beta default), `propose_candidate` must be **immediately preceded** in the same transaction by a native Ed25519 precompile instruction with exactly one signature, made by `config.canonicalizer`, over the 32-byte sha256 digest of this 251-byte message:

```
"PREQUEL-PROPOSE-V1" (18) | program id | config | event | outcome (1) | canon_hash | content_hash | image_hash | proposer | expiry (i64 LE)
content_hash = sha256( len(name) u16le | name | len(ticker) u16le | display ticker | len(uri) u16le | uri )
```

Signing the digest rather than the message keeps the precompile instruction small; a maximum-length proposal with a priority fee was measured at 938 bytes against the 1,232-byte packet limit (`a_maximum_length_attested_proposal_fits_in_one_packet_with_a_priority_fee`). The program checks: the Instructions sysvar address; that the previous instruction belongs to the Ed25519 program; a header with exactly one signature; all three `*_instruction_index` fields equal to `u16::MAX` (so key, signature and message all live in that same instruction); the public key equals the configured canonicalizer; the message equals the recomputed digest; and `now <= expiry`. The runtime verifies the signature itself. Binding the program id, config, event, proposer and expiry stops replay across deployments, events and wallets. Rotating the canonicalizer key invalidates old attestations at once.

## 6. Ticker canon (D-015, Q-19)

ASCII only, 2 to 10 characters; input is uppercased; table v1 maps `0 → O`, `1 → I`, `5 → S`; the candidate's address seed is the sha256 of the mapped string, so every disguised spelling lands on the same address. The table lives in `packages/shared/canon/confusables.txt`; `programs/prequel/build.rs` generates the Rust table and `packages/shared/scripts/gen-canon.ts` the TypeScript one, and a shared fixture file (35 cases) is checked on both sides. The program recomputes the hash from the ticker and rejects a mismatch, so a duplicate cannot be created by calling the program directly. The stored `ticker` is the uppercased input without the mapping, which is what users see. `EventState.canon_version` records the table version in force.

## 7. Invariants and the tests that pin them

Wording follows the Q-05 proposal where a donation or AMM rounding would break the original sentence.

| Id | Statement as tested | Tests |
| --- | --- | --- |
| 1 | open commitments + `refunded_total` = `total_committed` (before any refund: the commitments sum to the total) | `invariants.rs` `check_books` after every random step; `commits_accumulate_per_wallet_and_count_each_wallet_once`; `the_vault_always_holds_its_rent_plus_exactly_the_committed_total` |
| 2 | a refund pays exactly the commit; the vault holds its creation balance + owed + donations, and ends at the rent-exempt minimum + donations | `a_voided_event_refunds_every_commit_exactly_and_the_vault_ends_at_its_creation_balance`; `a_dust_donation_to_the_vault_cannot_block_the_last_refund`; `refunds_still_work_after_the_rent_rate_falls`; every refund in the random walk checks the vault delta, the wallet delta and the event |
| 4 | a Commitment is refunded at most once (claim joins in Phase 3) | second refund fails in the voided scene; `only_the_committer_can_refund_and_nobody_can_use_another_account`; the walk (`refunded` wallets have no commitment) |
| 5 | no proposal, co-proposal or commit at or after the close, nor on a voided event; a voided event only refunds | `proposals_follow_the_window_to_the_second`; `commits_follow_the_window_to_the_second_even_when_nobody_cranks_close`; `void_event_is_admin_only_works_from_open_and_closed_and_blocks_new_money`; the walk (every money-in attempt checked, with counters proving late and voided attempts happen) |
| 8 | at most one candidate per (event, outcome, canon hash) | `duplicates_are_rejected_even_when_disguised`; `one_candidate_per_canonical_ticker` (same address for every spelling in a family; different canon, outcome or event differ); the shared canon fixtures on both sides |
| 6 (proposed) | pause never blocks refund, reject, void or close | `neither_pause_flag_blocks_refund_reject_void_or_close` |
| extra | failed instructions change nothing; the treasury holds exactly the proposal fees; `candidate_count` equals the live slate; statuses only move forward | the random walk, every step |

The walk (`tests/invariants.rs`) is seeded and deterministic: 16 walks of 80 steps over two events, six wallets and disguised tickers, with time warps that land on `close - 1`, `close` and `close + 1`. Coverage floors keep it from passing vacuously (currently 58 proposals, 49 commits, 18 co-proposals, 19 refunds during the walks plus 92 in the closing phase, 18 rejections, 16 voids, 769 refused steps verified unchanged, 429 late and 186 voided money-in attempts all refused, 38 at the boundary seconds). Two deliberate bugs, a window that closes one second late and a refund that skips `refunded_total`, were confirmed to fail it.

## 8. Test harness

LiteSVM 0.15.0 (D-007) with the `precompiles` feature, so the Ed25519 precompile runs for real. `programs/prequel/tests/common` holds the program loader (the compiled `target/deploy/prequel.so`, so run `anchor build` first), clock control, funded wallets, PDA helpers, instruction builders, `send` returning the custom error code and logs, event decoding, a `ProgramData` patch so `initialize_config` can check the upgrade authority, and the Ed25519 instruction builder. Test files: `harness_smoke` (2), `config_events` (10), `propose_commit` (19), `review_hardening` (11), `lifecycle` (12), `invariants` (2), plus 22 unit tests inside the crate (logic, canon, attestation). Run everything with `scripts/rust-checks.sh`.

## 9. Deviations from SPEC.md (for your sign-off)

Approved by you on 2026-11-05: a frozen copy of the settings per event with hard caps (Q-11); the proposal fee paid by `co_propose` and a co-proposer weight frozen at joining (Q-14); admin `reject_candidate` and the extra `Rejected` status, the attestation covering name, ticker and URI with the beta running with it on, and the frozen ASCII-only ticker table v1 (Q-13, Q-16, Q-19); the atomic launch, built in Phases 3 and 4 (Q-02).

Made while building, not yet confirmed:

1. `proposal_cutoff` is a new setting, default 0, which keeps the SPEC behaviour (proposals run until close). It exists so the admin can have time to reject a bad candidate before commits stop.
2. `propose_candidate` and `co_propose` are stopped by either pause flag, because each is both a proposal and a commit.
3. The cap values in section 1 are mine (D-013). In particular `min_raise >= 0.1 SOL`, the 5-minute minimum gap between close and start, and the 60-day window limit.
4. `require_attestation` lives in `Params`, so each event freezes its own value; the SPEC lists it under Config.
5. The attestation signs a digest of the message, not the message itself (D-014 version 1.1).
6. URIs are capped at 128 bytes of printable ASCII (a CIDv1 URL is about 66); Metaplex allows 200.
7. `ExternalListing` is never closed, so a market whose event was voided cannot be listed again. Allowing relisting after a void is deferred (needs your decision).
8. `Commitment` carries no "claimed or refunded" flag: the account is closed instead, which is what the SPEC table describes ("closed on claim or refund").
9. `window_open == window_close` is allowed by `create_event`; the event then never takes money and can only be voided. Harmless, and the admin's own mistake to make.

## 10. What Phases 2 to 4 plug into

Phase 2 sets `proposed_outcome`, `resolved_outcome`, `evidence_hash`, `dispute_deadline`, `disputer`, `resolved_at` and `launch_deadline` on `EventState`, adds the dispute-bond account and the permissionless delay-void, and extends `is_refundable` with the losing side. Phases 3 and 4 fill `mint`, `pool`, `committer_tokens`, `pool_tokens`, `success_fee`, `pool_sol`, `launch_cost`, `sol_dust`, `claimed_total` and `claimed_commit_total` on `Candidate`, use `CoProposerShare.reward_debt` and `claimed` for creator fees, and add `FeeLedger`. The atomic launch (Q-02) replaces `birth_candidate` plus `seed_pool`; its measured cost is 153k compute units, 22 accounts and about 820 bytes before Metaplex, so it will need a v0 transaction with an address lookup table.
