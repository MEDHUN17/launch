//! One `emit!` per state change, for the indexer (SPEC §5 logs). Phase 1 subset.

use crate::state::Params;
use anchor_lang::prelude::*;

#[event]
pub struct ConfigUpdated {
    pub admin: Pubkey,
    pub resolver: Pubkey,
    pub canonicalizer: Pubkey,
    pub treasury: Pubkey,
    pub params: Params,
}

#[event]
pub struct PauseChanged {
    pub pause_proposals: bool,
    pub pause_commits: bool,
}

#[event]
pub struct EventCreated {
    pub event: Pubkey,
    pub id: u64,
    pub external_id_hash: [u8; 32],
    pub outcome_count: u8,
    pub window_open: i64,
    pub window_close: i64,
    pub event_start: i64,
    pub expected_resolution: i64,
    pub params: Params,
    pub canon_version: u8,
}

#[event]
pub struct CandidateProposed {
    pub event: Pubkey,
    pub candidate: Pubkey,
    pub outcome: u8,
    pub canon_hash: [u8; 32],
    pub proposer: Pubkey,
    pub name: String,
    pub ticker: String,
    pub uri: String,
    pub image_hash: [u8; 32],
    pub amount: u64,
}

#[event]
pub struct CoProposed {
    pub event: Pubkey,
    pub candidate: Pubkey,
    pub wallet: Pubkey,
    pub amount: u64,
    pub weight: u64,
    pub co_proposer_total: u64,
}

#[event]
pub struct Committed {
    pub event: Pubkey,
    pub candidate: Pubkey,
    pub wallet: Pubkey,
    pub amount: u64,
    pub wallet_total: u64,
    pub total_committed: u64,
    pub committer_count: u32,
}

#[event]
pub struct WindowClosed {
    pub event: Pubkey,
    pub closed_at: i64,
}

#[event]
pub struct EventVoided {
    pub event: Pubkey,
    pub by: Pubkey,
}

#[event]
pub struct CandidateRejected {
    pub event: Pubkey,
    pub candidate: Pubkey,
    pub outcome: u8,
}

#[event]
pub struct Refunded {
    pub event: Pubkey,
    pub candidate: Pubkey,
    pub wallet: Pubkey,
    pub amount: u64,
    pub refunded_total: u64,
}
