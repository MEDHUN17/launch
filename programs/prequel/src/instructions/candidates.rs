//! `propose_candidate`, `commit`, `co_propose` (SPEC §5; decisions Q-11, Q-13, Q-14, Q-16, Q-19).
//!
//! All three take money from a wallet into a candidate's program-owned vault, so they share one rule set:
//! the window must be open by the *effective* status (never trust the stored status alone), amounts respect
//! the event's frozen settings, and totals only move with checked math.

use crate::{
    attestation::{self, Attested, INSTRUCTIONS_SYSVAR_ID},
    canon,
    constants::*,
    error::PrequelError,
    events::*,
    logic,
    state::*,
};
use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

fn pay<'info>(
    system_program: &Program<'info, System>,
    from: &Signer<'info>,
    to: AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    transfer(
        CpiContext::new(
            system_program.key(),
            Transfer {
                from: from.to_account_info(),
                to,
            },
        ),
        amount,
    )
}

fn canon_err(e: canon::CanonError) -> PrequelError {
    match e {
        canon::CanonError::BadLength => PrequelError::TickerLength,
        canon::CanonError::BadCharacter => PrequelError::TickerCharacters,
    }
}

// ---- propose_candidate ---------------------------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ProposeArgs {
    pub outcome: u8,
    /// sha256 of the canonical ticker. Supplied because it seeds the Candidate address; the program recomputes it
    /// from `ticker` and rejects a mismatch, so a duplicate cannot be created even by calling the program directly.
    pub canon_hash: [u8; 32],
    pub name: String,
    pub ticker: String,
    pub uri: String,
    pub image_hash: [u8; 32],
    /// The opening commit, in lamports.
    pub amount: u64,
    /// Used only when the event requires an attestation: the expiry the canonicalizer signed.
    pub attestation_expiry: i64,
}

#[derive(Accounts)]
#[instruction(args: ProposeArgs)]
pub struct ProposeCandidate<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [EVENT_SEED, &event.id.to_le_bytes()], bump = event.bump)]
    pub event: Box<Account<'info, EventState>>,
    #[account(
        init,
        payer = proposer,
        space = 8 + Candidate::INIT_SPACE,
        seeds = [CANDIDATE_SEED, event.key().as_ref(), &[args.outcome], &args.canon_hash],
        bump
    )]
    pub candidate: Box<Account<'info, Candidate>>,
    #[account(
        init,
        payer = proposer,
        space = 8 + Vault::INIT_SPACE,
        seeds = [VAULT_SEED, candidate.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        init,
        payer = proposer,
        space = 8 + Commitment::INIT_SPACE,
        seeds = [COMMITMENT_SEED, candidate.key().as_ref(), proposer.key().as_ref()],
        bump
    )]
    pub commitment: Account<'info, Commitment>,
    /// Receives the non-refundable proposal fee.
    #[account(mut, address = config.treasury @ PrequelError::AccountMismatch)]
    pub treasury: SystemAccount<'info>,
    /// CHECK: the Instructions sysvar, read only when the event requires an attestation.
    #[account(address = INSTRUCTIONS_SYSVAR_ID @ PrequelError::AccountMismatch)]
    pub instructions: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_propose_candidate(ctx: Context<ProposeCandidate>, args: ProposeArgs) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        !ctx.accounts.config.pause_proposals,
        PrequelError::ProposalsPaused
    );
    let event_key = ctx.accounts.event.key();
    let event = &mut ctx.accounts.event;
    logic::check_accepts_commits(event.status, event.window_open, event.window_close, now)?;
    require!(
        args.outcome < event.outcome_count,
        PrequelError::InvalidOutcome
    );
    logic::validate_name(&args.name)?;
    logic::validate_uri(&args.uri)?;
    let display = String::from_utf8(canon::display_ticker(&args.ticker).map_err(canon_err)?)
        .map_err(|_| error!(PrequelError::TickerCharacters))?;
    let canon_hash = canon::canonical_ticker_hash(&args.ticker).map_err(canon_err)?;
    require!(
        canon_hash == args.canon_hash,
        PrequelError::CanonHashMismatch
    );

    // slate cap (an admin rejection frees a slot again)
    let max_slate = event.params.max_candidates_per_outcome;
    let slot = &mut event.candidate_count[args.outcome as usize];
    require!(*slot < max_slate, PrequelError::SlateFull);
    *slot = slot.checked_add(1).ok_or(PrequelError::MathOverflow)?;

    if event.params.require_attestation {
        let content_hash = attestation::content_hash(&args.name, &display, &args.uri);
        attestation::verify(
            &ctx.accounts.instructions.to_account_info(),
            &ctx.accounts.config.canonicalizer,
            &Attested {
                program_id: &crate::ID,
                config: &ctx.accounts.config.key(),
                event: &event_key,
                outcome: args.outcome,
                canon_hash: &canon_hash,
                content_hash: &content_hash,
                image_hash: &args.image_hash,
                proposer: &ctx.accounts.proposer.key(),
                expiry: args.attestation_expiry,
            },
            now,
        )?;
    }

    let amount = logic::check_commit_amount(&event.params, 0, args.amount)?;
    let proposal_fee = event.params.proposal_fee;
    pay(
        &ctx.accounts.system_program,
        &ctx.accounts.proposer,
        ctx.accounts.treasury.to_account_info(),
        proposal_fee,
    )?;
    pay(
        &ctx.accounts.system_program,
        &ctx.accounts.proposer,
        ctx.accounts.vault.to_account_info(),
        amount,
    )?;

    let candidate_key = ctx.accounts.candidate.key();
    let proposer_key = ctx.accounts.proposer.key();
    let candidate = &mut ctx.accounts.candidate;
    candidate.event = event_key;
    candidate.outcome = args.outcome;
    candidate.canon_hash = canon_hash;
    candidate.proposer = proposer_key;
    candidate.name = args.name.clone();
    candidate.ticker = display.clone();
    candidate.uri = args.uri.clone();
    candidate.image_hash = args.image_hash;
    candidate.total_committed = amount;
    candidate.committer_count = 1;
    candidate.status = CandidateStatus::Pending;
    candidate.created_at = now;
    candidate.bump = ctx.bumps.candidate;
    candidate.vault_bump = ctx.bumps.vault;

    let vault = &mut ctx.accounts.vault;
    vault.candidate = candidate_key;
    vault.bump = ctx.bumps.vault;

    let commitment = &mut ctx.accounts.commitment;
    commitment.candidate = candidate_key;
    commitment.wallet = proposer_key;
    commitment.amount = amount;
    commitment.first_commit_ts = now;
    commitment.bump = ctx.bumps.commitment;

    emit!(CandidateProposed {
        event: event_key,
        candidate: candidate_key,
        outcome: args.outcome,
        canon_hash,
        proposer: proposer_key,
        name: args.name,
        ticker: display,
        uri: args.uri,
        image_hash: args.image_hash,
        amount,
    });
    emit!(Committed {
        event: event_key,
        candidate: candidate_key,
        wallet: proposer_key,
        amount,
        wallet_total: amount,
        total_committed: amount,
        committer_count: 1
    });
    Ok(())
}

// ---- commit ----------------------------------------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Commit<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
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
    #[account(
        init_if_needed,
        payer = wallet,
        space = 8 + Commitment::INIT_SPACE,
        seeds = [COMMITMENT_SEED, candidate.key().as_ref(), wallet.key().as_ref()],
        bump
    )]
    pub commitment: Account<'info, Commitment>,
    pub system_program: Program<'info, System>,
}

/// Who is depositing where: keys and the commitment's bump, kept together so `deposit` stays readable.
struct Depositor {
    candidate_key: Pubkey,
    wallet_key: Pubkey,
    commitment_bump: u8,
}

/// Adds `amount` to the wallet's commitment, creating it on the first commit. Returns the wallet's new total.
fn deposit(
    now: i64,
    event: &EventState,
    candidate: &mut Candidate,
    commitment: &mut Commitment,
    who: Depositor,
    amount: u64,
) -> Result<u64> {
    let Depositor {
        candidate_key,
        wallet_key,
        commitment_bump,
    } = who;
    logic::check_accepts_commits(event.status, event.window_open, event.window_close, now)?;
    require!(
        candidate.status == CandidateStatus::Pending,
        PrequelError::CandidateNotPending
    );
    // `init_if_needed` leaves a brand-new account zeroed; a real commitment always has a first-commit time.
    let fresh = commitment.first_commit_ts == 0;
    let wallet_total = logic::check_commit_amount(
        &event.params,
        if fresh { 0 } else { commitment.amount },
        amount,
    )?;
    if fresh {
        commitment.candidate = candidate_key;
        commitment.wallet = wallet_key;
        commitment.first_commit_ts = now;
        commitment.bump = commitment_bump;
        candidate.committer_count = candidate
            .committer_count
            .checked_add(1)
            .ok_or(PrequelError::MathOverflow)?;
    } else {
        require!(
            commitment.candidate == candidate_key && commitment.wallet == wallet_key,
            PrequelError::AccountMismatch
        );
    }
    commitment.amount = wallet_total;
    candidate.total_committed = candidate
        .total_committed
        .checked_add(amount)
        .ok_or(PrequelError::MathOverflow)?;
    Ok(wallet_total)
}

pub fn handle_commit(ctx: Context<Commit>, amount: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        !ctx.accounts.config.pause_commits,
        PrequelError::CommitsPaused
    );
    let (candidate_key, wallet_key, event_key) = (
        ctx.accounts.candidate.key(),
        ctx.accounts.wallet.key(),
        ctx.accounts.event.key(),
    );
    let wallet_total = deposit(
        now,
        &ctx.accounts.event,
        &mut ctx.accounts.candidate,
        &mut ctx.accounts.commitment,
        Depositor {
            candidate_key,
            wallet_key,
            commitment_bump: ctx.bumps.commitment,
        },
        amount,
    )?;
    pay(
        &ctx.accounts.system_program,
        &ctx.accounts.wallet,
        ctx.accounts.vault.to_account_info(),
        amount,
    )?;
    emit!(Committed {
        event: event_key,
        candidate: candidate_key,
        wallet: wallet_key,
        amount,
        wallet_total,
        total_committed: ctx.accounts.candidate.total_committed,
        committer_count: ctx.accounts.candidate.committer_count,
    });
    Ok(())
}

// ---- co_propose ----------------------------------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CoPropose<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
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
    #[account(
        init_if_needed,
        payer = wallet,
        space = 8 + Commitment::INIT_SPACE,
        seeds = [COMMITMENT_SEED, candidate.key().as_ref(), wallet.key().as_ref()],
        bump
    )]
    pub commitment: Account<'info, Commitment>,
    #[account(
        init,
        payer = wallet,
        space = 8 + CoProposerShare::INIT_SPACE,
        seeds = [COPROPOSER_SEED, candidate.key().as_ref(), wallet.key().as_ref()],
        bump
    )]
    pub co_proposer_share: Account<'info, CoProposerShare>,
    /// Receives the same non-refundable fee a proposer pays (Q-14).
    #[account(mut, address = config.treasury @ PrequelError::AccountMismatch)]
    pub treasury: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_co_propose(ctx: Context<CoPropose>, amount: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    // Co-proposing is both a proposal and a commit, so either pause stops it.
    require!(
        !ctx.accounts.config.pause_proposals,
        PrequelError::ProposalsPaused
    );
    require!(
        !ctx.accounts.config.pause_commits,
        PrequelError::CommitsPaused
    );
    let (candidate_key, wallet_key, event_key) = (
        ctx.accounts.candidate.key(),
        ctx.accounts.wallet.key(),
        ctx.accounts.event.key(),
    );
    require!(
        wallet_key != ctx.accounts.candidate.proposer,
        PrequelError::ProposerCannotCoPropose
    );
    let wallet_total = deposit(
        now,
        &ctx.accounts.event,
        &mut ctx.accounts.candidate,
        &mut ctx.accounts.commitment,
        Depositor {
            candidate_key,
            wallet_key,
            commitment_bump: ctx.bumps.commitment,
        },
        amount,
    )?;
    let proposal_fee = ctx.accounts.event.params.proposal_fee;
    pay(
        &ctx.accounts.system_program,
        &ctx.accounts.wallet,
        ctx.accounts.treasury.to_account_info(),
        proposal_fee,
    )?;
    pay(
        &ctx.accounts.system_program,
        &ctx.accounts.wallet,
        ctx.accounts.vault.to_account_info(),
        amount,
    )?;

    // The reward weight is frozen at the amount committed when joining; later commits do not raise it (Q-14).
    let candidate = &mut ctx.accounts.candidate;
    candidate.co_proposer_total = candidate
        .co_proposer_total
        .checked_add(amount)
        .ok_or(PrequelError::MathOverflow)?;
    candidate.co_proposer_count = candidate
        .co_proposer_count
        .checked_add(1)
        .ok_or(PrequelError::MathOverflow)?;
    let share = &mut ctx.accounts.co_proposer_share;
    share.candidate = candidate_key;
    share.wallet = wallet_key;
    share.weight = amount;
    share.claimed = 0;
    share.reward_debt = 0;
    share.joined_at = now;
    share.bump = ctx.bumps.co_proposer_share;

    emit!(CoProposed {
        event: event_key,
        candidate: candidate_key,
        wallet: wallet_key,
        amount,
        weight: amount,
        co_proposer_total: candidate.co_proposer_total
    });
    emit!(Committed {
        event: event_key,
        candidate: candidate_key,
        wallet: wallet_key,
        amount,
        wallet_total,
        total_committed: candidate.total_committed,
        committer_count: candidate.committer_count,
    });
    Ok(())
}
