//! `close_window`, `void_event`, `reject_candidate`, `refund` (SPEC §5).
//!
//! None of these is ever blocked by the pause flags (invariant 6): users can always exit.

use crate::{constants::*, error::PrequelError, events::*, logic, state::*};
use anchor_lang::prelude::*;

// ---- close_window ----------------------------------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CloseWindow<'info> {
    /// Anyone can crank this.
    pub caller: Signer<'info>,
    #[account(mut, seeds = [EVENT_SEED, &event.id.to_le_bytes()], bump = event.bump)]
    pub event: Box<Account<'info, EventState>>,
}

/// Records the close in the stored status. The clock already stops commits at `window_close` whether or not this
/// ran; the stored status is only a cache for the keeper and indexer. A repeat call fails with a named error.
pub fn handle_close_window(ctx: Context<CloseWindow>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let event = &mut ctx.accounts.event;
    require!(
        event.status == EventStatus::Open,
        PrequelError::EventStateInvalid
    );
    require!(now >= event.window_close, PrequelError::WindowNotClosedYet);
    event.status = EventStatus::Closed;
    emit!(WindowClosed {
        event: event.key(),
        closed_at: now
    });
    Ok(())
}

// ---- void_event --------------------------------------------------------------------------------------------------------

pub const VOID_BY_ADMIN: u8 = 1;

#[derive(Accounts)]
pub struct VoidEvent<'info> {
    pub admin: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin @ PrequelError::NotAdmin)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [EVENT_SEED, &event.id.to_le_bytes()], bump = event.bump)]
    pub event: Box<Account<'info, EventState>>,
}

/// Admin void of an Open or Closed event: every candidate becomes refundable. (Phase 2 adds Disputed and the
/// permissionless void after `max_resolution_delay`.)
pub fn handle_void_event(ctx: Context<VoidEvent>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let event = &mut ctx.accounts.event;
    require!(
        matches!(event.status, EventStatus::Open | EventStatus::Closed),
        PrequelError::EventStateInvalid
    );
    event.status = EventStatus::Voided;
    event.voided_at = now;
    event.void_reason = VOID_BY_ADMIN;
    emit!(EventVoided {
        event: event.key(),
        by: ctx.accounts.admin.key()
    });
    Ok(())
}

// ---- reject_candidate ------------------------------------------------------------------------------------------------

#[derive(Accounts)]
pub struct RejectCandidate<'info> {
    pub admin: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin @ PrequelError::NotAdmin)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [EVENT_SEED, &event.id.to_le_bytes()], bump = event.bump)]
    pub event: Box<Account<'info, EventState>>,
    #[account(
        mut,
        seeds = [CANDIDATE_SEED, event.key().as_ref(), &[candidate.outcome], &candidate.canon_hash],
        bump = candidate.bump,
        has_one = event @ PrequelError::AccountMismatch
    )]
    pub candidate: Box<Account<'info, Candidate>>,
}

/// Before the window closes the admin can reject a candidate (offensive metadata, impersonation): committers get
/// 100% back, the slot is freed, the proposal fee stays with the treasury, and the ticker stays burned (Q-13).
pub fn handle_reject_candidate(ctx: Context<RejectCandidate>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let event = &mut ctx.accounts.event;
    logic::check_accepts_commits(event.status, event.window_open, event.window_close, now)?;
    let candidate = &mut ctx.accounts.candidate;
    require!(
        candidate.status == CandidateStatus::Pending,
        PrequelError::CandidateNotRejectable
    );
    candidate.status = CandidateStatus::Rejected;
    let slot = &mut event.candidate_count[candidate.outcome as usize];
    *slot = slot.checked_sub(1).ok_or(PrequelError::MathOverflow)?;
    emit!(CandidateRejected {
        event: event.key(),
        candidate: candidate.key(),
        outcome: candidate.outcome
    });
    Ok(())
}

// ---- refund ------------------------------------------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,
    #[account(seeds = [EVENT_SEED, &event.id.to_le_bytes()], bump = event.bump)]
    pub event: Box<Account<'info, EventState>>,
    #[account(
        mut,
        seeds = [CANDIDATE_SEED, event.key().as_ref(), &[candidate.outcome], &candidate.canon_hash],
        bump = candidate.bump,
        has_one = event @ PrequelError::AccountMismatch
    )]
    pub candidate: Box<Account<'info, Candidate>>,
    #[account(mut, seeds = [VAULT_SEED, candidate.key().as_ref()], bump = candidate.vault_bump)]
    pub vault: Account<'info, Vault>,
    /// Closed to the wallet, which gets its rent back along with the refund. Closing it is also what makes a
    /// second refund impossible (invariant 4).
    #[account(
        mut,
        close = wallet,
        seeds = [COMMITMENT_SEED, candidate.key().as_ref(), wallet.key().as_ref()],
        bump = commitment.bump
    )]
    pub commitment: Account<'info, Commitment>,
    /// Required when the wallet co-proposed (its Commitment says so) and closed with it; omit otherwise.
    #[account(
        mut,
        close = wallet,
        seeds = [COPROPOSER_SEED, candidate.key().as_ref(), wallet.key().as_ref()],
        bump = co_proposer_share.bump
    )]
    pub co_proposer_share: Option<Account<'info, CoProposerShare>>,
}

/// Pays the wallet exactly its commitment from the candidate's vault. No fee is taken. The vault is program-owned,
/// so the program debits lamports directly; donations only ever add surplus and never block the last refund.
pub fn handle_refund(ctx: Context<Refund>) -> Result<()> {
    let candidate = &mut ctx.accounts.candidate;
    require!(
        logic::is_refundable(ctx.accounts.event.status, candidate.status),
        PrequelError::NotRefundable
    );
    // A co-proposer's share can only be closed through its commitment, so it must come along now.
    require!(
        !ctx.accounts.commitment.has_share || ctx.accounts.co_proposer_share.is_some(),
        PrequelError::CoProposerShareRequired
    );
    // The first refund of a voided event records that the candidate is refunding.
    if candidate.status == CandidateStatus::Pending {
        candidate.status = CandidateStatus::Refunding;
    }

    let amount = ctx.accounts.commitment.amount;
    let refunded = candidate
        .refunded_total
        .checked_add(amount)
        .ok_or(PrequelError::MathOverflow)?;
    require!(
        refunded <= candidate.total_committed,
        PrequelError::VaultUnderfunded
    );
    require!(
        ctx.accounts.vault.to_account_info().lamports() >= amount,
        PrequelError::VaultUnderfunded
    );
    ctx.accounts.vault.sub_lamports(amount)?;
    ctx.accounts.wallet.add_lamports(amount)?;
    candidate.refunded_total = refunded;

    emit!(Refunded {
        event: ctx.accounts.event.key(),
        candidate: candidate.key(),
        wallet: ctx.accounts.wallet.key(),
        amount,
        refunded_total: refunded,
        candidate_status: candidate.status,
    });
    Ok(())
}
