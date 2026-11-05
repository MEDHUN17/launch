//! `create_event` (SPEC §5): lists an event, freezes the economic settings into it, and blocks listing the same
//! external market twice.

use crate::{constants::*, error::PrequelError, events::*, logic, state::*};
use anchor_lang::prelude::*;
use solana_sha256_hasher::hash;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateEventArgs {
    /// Source-specific market id, for example a Kalshi ticker.
    pub external_id: String,
    /// sha256 of `external_id`, supplied so it can seed `ExternalListing`; the program recomputes it.
    pub external_id_hash: [u8; 32],
    pub source: u8,
    pub outcome_count: u8,
    pub window_open: i64,
    pub window_close: i64,
    pub event_start: i64,
    pub expected_resolution: i64,
}

#[derive(Accounts)]
#[instruction(args: CreateEventArgs)]
pub struct CreateEvent<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin @ PrequelError::NotAdmin)]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = admin,
        space = 8 + EventState::INIT_SPACE,
        seeds = [EVENT_SEED, &config.next_event_id.to_le_bytes()],
        bump
    )]
    pub event: Account<'info, EventState>,
    #[account(
        init,
        payer = admin,
        space = 8 + ExternalListing::INIT_SPACE,
        seeds = [EXTERNAL_SEED, &args.external_id_hash],
        bump
    )]
    pub external_listing: Account<'info, ExternalListing>,
    pub system_program: Program<'info, System>,
}

pub fn handle_create_event(ctx: Context<CreateEvent>, args: CreateEventArgs) -> Result<()> {
    require!(
        args.source == SOURCE_KALSHI,
        PrequelError::UnsupportedSource
    );
    logic::validate_external_id(&args.external_id)?;
    require!(
        hash(args.external_id.as_bytes()).to_bytes() == args.external_id_hash,
        PrequelError::ExternalIdHashMismatch
    );
    logic::validate_outcome_count(args.outcome_count)?;
    let now = Clock::get()?.unix_timestamp;
    logic::validate_event_times(
        now,
        args.window_open,
        args.window_close,
        args.event_start,
        args.expected_resolution,
    )?;

    let config = &mut ctx.accounts.config;
    let id = config.next_event_id;
    config.next_event_id = id.checked_add(1).ok_or(PrequelError::MathOverflow)?;

    let event = &mut ctx.accounts.event;
    event.id = id;
    event.source = args.source;
    event.external_id_hash = args.external_id_hash;
    event.outcome_count = args.outcome_count;
    event.window_open = args.window_open;
    event.window_close = args.window_close;
    event.event_start = args.event_start;
    event.expected_resolution = args.expected_resolution;
    event.status = EventStatus::Open;
    event.proposed_outcome = NO_OUTCOME;
    event.resolved_outcome = NO_OUTCOME;
    event.evidence_hash = [0; 32];
    event.dispute_deadline = 0;
    event.disputer = Pubkey::default();
    event.resolved_at = 0;
    event.launch_deadline = 0;
    event.candidate_count = [0; 8];
    event.params = config.params; // frozen for the life of the event (Q-11)
    event.canon_version = crate::canon::CANON_VERSION;
    event.created_at = now;
    event.bump = ctx.bumps.event;

    let listing = &mut ctx.accounts.external_listing;
    listing.event = event.key();
    listing.external_id_hash = args.external_id_hash;
    listing.bump = ctx.bumps.external_listing;

    emit!(EventCreated {
        event: event.key(),
        id,
        external_id_hash: args.external_id_hash,
        outcome_count: args.outcome_count,
        window_open: args.window_open,
        window_close: args.window_close,
        event_start: args.event_start,
        expected_resolution: args.expected_resolution,
        params: event.params,
        canon_version: event.canon_version,
    });
    Ok(())
}
