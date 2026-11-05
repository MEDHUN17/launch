//! `initialize_config`, `update_config`, `set_pause` (SPEC §5).

use crate::{constants::*, error::PrequelError, events::*, logic, state::*};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeConfigArgs {
    pub admin: Pubkey,
    pub resolver: Pubkey,
    pub canonicalizer: Pubkey,
    pub treasury: Pubkey,
    pub params: Params,
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    /// Must be the program's upgrade authority, so nobody can front-run the one-time setup after a deploy.
    #[account(mut)]
    pub deployer: Signer<'info>,
    #[account(
        init,
        payer = deployer,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(constraint = program.programdata_address()? == Some(program_data.key()) @ PrequelError::NotDeployer)]
    pub program: Program<'info, crate::program::Prequel>,
    #[account(constraint = program_data.upgrade_authority_address == Some(deployer.key()) @ PrequelError::NotDeployer)]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}

fn check_keys(keys: &[&Pubkey]) -> Result<()> {
    require!(
        keys.iter().all(|k| **k != Pubkey::default()),
        PrequelError::InvalidKey
    );
    Ok(())
}

pub fn handle_initialize_config(
    ctx: Context<InitializeConfig>,
    args: InitializeConfigArgs,
) -> Result<()> {
    check_keys(&[
        &args.admin,
        &args.resolver,
        &args.canonicalizer,
        &args.treasury,
    ])?;
    logic::validate_params(&args.params)?;
    let config = &mut ctx.accounts.config;
    config.admin = args.admin;
    config.resolver = args.resolver;
    config.canonicalizer = args.canonicalizer;
    config.treasury = args.treasury;
    config.params = args.params;
    config.pause_proposals = false;
    config.pause_commits = false;
    config.version = 1;
    config.next_event_id = 0;
    config.bump = ctx.bumps.config;
    emit!(ConfigUpdated {
        admin: config.admin,
        resolver: config.resolver,
        canonicalizer: config.canonicalizer,
        treasury: config.treasury,
        params: config.params,
    });
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateConfigArgs {
    pub admin: Option<Pubkey>,
    pub resolver: Option<Pubkey>,
    pub canonicalizer: Option<Pubkey>,
    pub treasury: Option<Pubkey>,
    /// New defaults. Events that already exist keep the settings they were created with (Q-11).
    pub params: Option<Params>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin @ PrequelError::NotAdmin)]
    pub config: Account<'info, Config>,
}

pub fn handle_update_config(ctx: Context<UpdateConfig>, args: UpdateConfigArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;
    if let Some(k) = args.admin {
        check_keys(&[&k])?;
        config.admin = k;
    }
    if let Some(k) = args.resolver {
        check_keys(&[&k])?;
        config.resolver = k;
    }
    if let Some(k) = args.canonicalizer {
        check_keys(&[&k])?;
        config.canonicalizer = k;
    }
    if let Some(k) = args.treasury {
        check_keys(&[&k])?;
        config.treasury = k;
    }
    if let Some(p) = args.params {
        logic::validate_params(&p)?;
        config.params = p;
    }
    emit!(ConfigUpdated {
        admin: config.admin,
        resolver: config.resolver,
        canonicalizer: config.canonicalizer,
        treasury: config.treasury,
        params: config.params,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct SetPause<'info> {
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin @ PrequelError::NotAdmin)]
    pub config: Account<'info, Config>,
}

/// Pauses new proposals and/or new commits. Never touches refunds, claims or fee withdrawals (invariant 6).
pub fn handle_set_pause(
    ctx: Context<SetPause>,
    pause_proposals: Option<bool>,
    pause_commits: Option<bool>,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    if let Some(p) = pause_proposals {
        config.pause_proposals = p;
    }
    if let Some(c) = pause_commits {
        config.pause_commits = c;
    }
    emit!(PauseChanged {
        pause_proposals: config.pause_proposals,
        pause_commits: config.pause_commits
    });
    Ok(())
}
