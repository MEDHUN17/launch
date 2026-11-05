//! Accounts of Phase 1 (docs/PHASE1_DESIGN.md section 2). Fields that Phases 2 to 4 need are reserved now so no
//! account has to be reallocated.

use crate::constants::*;
use anchor_lang::prelude::*;

/// Economic settings. `Config.params` holds the defaults for new events; `EventState.params` is the frozen copy
/// an event keeps for life (decision Q-11).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct Params {
    pub min_commit: u64,
    /// 0 means no cap.
    pub max_commit_per_wallet: u64,
    pub min_raise: u64,
    pub max_candidates_per_outcome: u8,
    pub dispute_window: i64,
    pub dispute_bond: u64,
    pub max_resolution_delay: i64,
    /// Time allowed to launch a winner before it refunds (atomic launch, Q-02).
    pub launch_window: i64,
    pub proposal_fee: u64,
    pub success_fee_bps: u16,
    pub split_proposer_bps: u16,
    pub split_coprop_bps: u16,
    pub split_treasury_bps: u16,
    pub require_attestation: bool,
}

impl Params {
    /// SPEC §3 defaults, with attestation on as agreed for the beta.
    pub fn spec_defaults() -> Self {
        Params {
            min_commit: LAMPORTS_PER_SOL / 20,
            max_commit_per_wallet: 50 * LAMPORTS_PER_SOL,
            min_raise: 5 * LAMPORTS_PER_SOL,
            max_candidates_per_outcome: 12,
            dispute_window: 2 * HOUR,
            dispute_bond: LAMPORTS_PER_SOL,
            max_resolution_delay: 7 * DAY,
            launch_window: DAY,
            proposal_fee: LAMPORTS_PER_SOL / 50,
            success_fee_bps: 100,
            split_proposer_bps: 3_000,
            split_coprop_bps: 2_000,
            split_treasury_bps: 5_000,
            require_attestation: true,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum EventStatus {
    Open,
    Closed,
    ResolutionProposed,
    Disputed,
    Resolved,
    Voided,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum CandidateStatus {
    Pending,
    Born,
    Refunding,
    /// Rejected by the admin before close: refundable, slot freed (Q-13).
    Rejected,
}

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    pub resolver: Pubkey,
    pub canonicalizer: Pubkey,
    pub treasury: Pubkey,
    /// Defaults copied into each new event.
    pub params: Params,
    pub pause_proposals: bool,
    pub pause_commits: bool,
    pub version: u8,
    pub next_event_id: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct EventState {
    pub id: u64,
    pub source: u8,
    pub external_id_hash: [u8; 32],
    pub outcome_count: u8,
    pub window_open: i64,
    pub window_close: i64,
    pub event_start: i64,
    pub expected_resolution: i64,
    pub status: EventStatus,
    /// `NO_OUTCOME` until set (Phase 2).
    pub proposed_outcome: u8,
    pub resolved_outcome: u8,
    pub evidence_hash: [u8; 32],
    pub dispute_deadline: i64,
    pub disputer: Pubkey,
    pub resolved_at: i64,
    /// Set at resolution: a winner not launched by then refunds (Q-02, Phase 2/3).
    pub launch_deadline: i64,
    pub candidate_count: [u8; 8],
    /// Frozen copy of the settings at creation (Q-11).
    pub params: Params,
    pub canon_version: u8,
    pub created_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct ExternalListing {
    pub event: Pubkey,
    pub external_id_hash: [u8; 32],
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Candidate {
    pub event: Pubkey,
    pub outcome: u8,
    pub canon_hash: [u8; 32],
    pub proposer: Pubkey,
    #[max_len(32)]
    pub name: String,
    /// Uppercased input without confusable mapping (what users see).
    #[max_len(10)]
    pub ticker: String,
    #[max_len(200)]
    pub uri: String,
    pub image_hash: [u8; 32],
    pub total_committed: u64,
    pub committer_count: u32,
    pub co_proposer_total: u64,
    pub co_proposer_count: u16,
    pub refunded_total: u64,
    pub status: CandidateStatus,
    pub created_at: i64,
    // Reserved for the launch and claims (Phases 3 and 4); zero until then.
    pub mint: Pubkey,
    pub pool: Pubkey,
    pub committer_tokens: u64,
    pub pool_tokens: u64,
    pub claimed_total: u64,
    pub bump: u8,
    pub vault_bump: u8,
}

/// Program-owned so that donations never block a refund and refunds can debit lamports directly (D-010).
#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub candidate: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Commitment {
    pub candidate: Pubkey,
    pub wallet: Pubkey,
    pub amount: u64,
    pub first_commit_ts: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct CoProposerShare {
    pub candidate: Pubkey,
    pub wallet: Pubkey,
    /// Frozen at the amount committed when joining (Q-14).
    pub weight: u64,
    pub claimed: u64,
    pub reward_debt: u128,
    pub joined_at: i64,
    pub bump: u8,
}
