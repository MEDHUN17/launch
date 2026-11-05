//! Prequel: a Solana launchpad for event-conditional memecoins (SPEC.md). Phase 1: program core.

pub mod attestation;
pub mod canon;
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod logic;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use error::*;
pub use instructions::*;
pub use state::*;

// Placeholder until the real program keypair is generated for the devnet beta (Phase 10); never committed.
declare_id!("FA6eApbY1Qqghuxo9gpRE8tpGWgsajQ6cWkotaKTrwEQ");

#[program]
pub mod prequel {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        args: InitializeConfigArgs,
    ) -> Result<()> {
        instructions::config::handle_initialize_config(ctx, args)
    }

    pub fn update_config(ctx: Context<UpdateConfig>, args: UpdateConfigArgs) -> Result<()> {
        instructions::config::handle_update_config(ctx, args)
    }

    pub fn set_pause(
        ctx: Context<SetPause>,
        pause_proposals: Option<bool>,
        pause_commits: Option<bool>,
    ) -> Result<()> {
        instructions::config::handle_set_pause(ctx, pause_proposals, pause_commits)
    }

    pub fn create_event(ctx: Context<CreateEvent>, args: CreateEventArgs) -> Result<()> {
        instructions::events::handle_create_event(ctx, args)
    }

    pub fn propose_candidate(ctx: Context<ProposeCandidate>, args: ProposeArgs) -> Result<()> {
        instructions::candidates::handle_propose_candidate(ctx, args)
    }

    pub fn commit(ctx: Context<Commit>, amount: u64) -> Result<()> {
        instructions::candidates::handle_commit(ctx, amount)
    }

    pub fn co_propose(ctx: Context<CoPropose>, amount: u64) -> Result<()> {
        instructions::candidates::handle_co_propose(ctx, amount)
    }
}
