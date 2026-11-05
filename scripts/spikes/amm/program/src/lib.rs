//! Phase 0 spike A (task 0.4): can a Prequel-style program create a Meteora DAMM v2 pool by CPI, signed only
//! by PDAs, lock the position permanently, and later claim trading fees?
//!
//! Throwaway code to answer D-002 questions. It is NOT the Phase 4 `seed_pool`: it trusts the caller's
//! amounts and accounts, which the real instruction must never do (see docs/DECISIONS.md H-2).
//!
//! PDAs (all owned by the System Program; none holds data):
//! - `seeder`: pays rent and owns the deposit token accounts (the payer must be system-owned),
//! - `owner`: owns the position NFT, so it signs the lock and every fee claim,
//! - `nft_mint`: signs as the (new) position NFT mint account the AMM creates.

use anchor_lang::prelude::*;

declare_id!("ELiN5WkrxAxVB3nhEhm8H4M6Qp2vJqigeuZrURDb1qWc");

// Generated from idls/cp_amm.json (IDL extracted from @meteora-ag/cp-amm-sdk 1.5.1, cp_amm 0.2.5).
declare_program!(cp_amm);

use cp_amm::{
    cpi::{self, accounts as amm},
    program::CpAmm,
    types::InitializePoolParameters,
};

pub const SEEDER: &[u8] = b"seeder";
pub const OWNER: &[u8] = b"owner";
pub const NFT_MINT: &[u8] = b"nft_mint";

#[program]
pub mod amm_spike {
    use super::*;

    /// Create the pool (token A = new token, token B = wSOL) and lock the whole position permanently.
    pub fn seed(ctx: Context<Seed>, liquidity: u128, sqrt_price: u128) -> Result<()> {
        let a = &ctx.accounts;
        let seeder_seeds: &[&[u8]] = &[SEEDER, &[ctx.bumps.seeder]];
        let nft_seeds: &[&[u8]] = &[NFT_MINT, &[ctx.bumps.position_nft_mint]];
        let owner_seeds: &[&[u8]] = &[OWNER, &[ctx.bumps.owner]];

        let init_accounts = amm::InitializePool {
            creator: a.owner.to_account_info(),
            position_nft_mint: a.position_nft_mint.to_account_info(),
            position_nft_account: a.position_nft_account.to_account_info(),
            payer: a.seeder.to_account_info(),
            config: a.config.to_account_info(),
            pool_authority: a.pool_authority.to_account_info(),
            pool: a.pool.to_account_info(),
            position: a.position.to_account_info(),
            token_a_mint: a.token_a_mint.to_account_info(),
            token_b_mint: a.token_b_mint.to_account_info(),
            token_a_vault: a.token_a_vault.to_account_info(),
            token_b_vault: a.token_b_vault.to_account_info(),
            payer_token_a: a.payer_token_a.to_account_info(),
            payer_token_b: a.payer_token_b.to_account_info(),
            token_a_program: a.token_program.to_account_info(),
            token_b_program: a.token_program.to_account_info(),
            token_2022_program: a.token_2022_program.to_account_info(),
            system_program: a.system_program.to_account_info(),
            event_authority: a.event_authority.to_account_info(),
            program: a.amm_program.to_account_info(),
        };
        let signers: &[&[&[u8]]] = &[seeder_seeds, nft_seeds];
        cpi::initialize_pool(
            CpiContext::new_with_signer(a.amm_program.key(), init_accounts, signers),
            InitializePoolParameters {
                liquidity,
                sqrt_price,
                activation_point: None,
            },
        )?;

        // Lock exactly what the position now holds, read back from the AMM's own account.
        let unlocked = {
            let data = a.position.try_borrow_data()?;
            cp_amm::accounts::Position::try_deserialize(&mut &data[..])?.unlocked_liquidity
        };
        msg!(
            "spike: position unlocked_liquidity before lock = {}",
            unlocked
        );

        let lock_accounts = amm::PermanentLockPosition {
            pool: a.pool.to_account_info(),
            position: a.position.to_account_info(),
            position_nft_account: a.position_nft_account.to_account_info(),
            signer: a.owner.to_account_info(),
            event_authority: a.event_authority.to_account_info(),
            program: a.amm_program.to_account_info(),
        };
        let owner_signers: &[&[&[u8]]] = &[owner_seeds];
        cpi::permanent_lock_position(
            CpiContext::new_with_signer(a.amm_program.key(), lock_accounts, owner_signers),
            unlocked,
        )?;
        Ok(())
    }

    /// Claim the position's accrued trading fees into `fee_account_a` / `fee_account_b`.
    pub fn harvest(ctx: Context<Harvest>) -> Result<()> {
        let a = &ctx.accounts;
        let owner_seeds: &[&[u8]] = &[OWNER, &[ctx.bumps.owner]];
        let claim = amm::ClaimPositionFee {
            pool_authority: a.pool_authority.to_account_info(),
            pool: a.pool.to_account_info(),
            position: a.position.to_account_info(),
            token_a_account: a.fee_account_a.to_account_info(),
            token_b_account: a.fee_account_b.to_account_info(),
            token_a_vault: a.token_a_vault.to_account_info(),
            token_b_vault: a.token_b_vault.to_account_info(),
            token_a_mint: a.token_a_mint.to_account_info(),
            token_b_mint: a.token_b_mint.to_account_info(),
            position_nft_account: a.position_nft_account.to_account_info(),
            signer: a.owner.to_account_info(),
            token_a_program: a.token_program.to_account_info(),
            token_b_program: a.token_program.to_account_info(),
            event_authority: a.event_authority.to_account_info(),
            program: a.amm_program.to_account_info(),
        };
        let signers: &[&[&[u8]]] = &[owner_seeds];
        cpi::claim_position_fee(CpiContext::new_with_signer(
            a.amm_program.key(),
            claim,
            signers,
        ))
    }
}

#[derive(Accounts)]
pub struct Seed<'info> {
    /// CHECK: system-owned PDA; pays rent and owns the deposit token accounts.
    #[account(mut, seeds = [SEEDER], bump)]
    pub seeder: UncheckedAccount<'info>,
    /// CHECK: system-owned PDA; owns the position NFT.
    #[account(seeds = [OWNER], bump)]
    pub owner: UncheckedAccount<'info>,
    /// CHECK: PDA that signs as the position NFT mint; the AMM creates the account.
    #[account(mut, seeds = [NFT_MINT], bump)]
    pub position_nft_mint: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub position_nft_account: UncheckedAccount<'info>,
    /// CHECK: AMM config (public config 11 in this spike).
    pub config: UncheckedAccount<'info>,
    /// CHECK: AMM pool authority PDA.
    pub pool_authority: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub position: UncheckedAccount<'info>,
    /// CHECK: mint of token A (the new token).
    pub token_a_mint: UncheckedAccount<'info>,
    /// CHECK: mint of token B (wrapped SOL).
    pub token_b_mint: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub token_a_vault: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub token_b_vault: UncheckedAccount<'info>,
    /// CHECK: token account owned by `seeder`, holds token A.
    #[account(mut)]
    pub payer_token_a: UncheckedAccount<'info>,
    /// CHECK: token account owned by `seeder`, holds wrapped SOL.
    #[account(mut)]
    pub payer_token_b: UncheckedAccount<'info>,
    /// CHECK: classic SPL Token program.
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: Token-2022 program (the AMM uses it for the position NFT).
    pub token_2022_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: AMM event authority PDA.
    pub event_authority: UncheckedAccount<'info>,
    pub amm_program: Program<'info, CpAmm>,
}

#[derive(Accounts)]
pub struct Harvest<'info> {
    /// CHECK: system-owned PDA; owns the position NFT and signs the claim.
    #[account(seeds = [OWNER], bump)]
    pub owner: UncheckedAccount<'info>,
    /// CHECK: AMM pool authority PDA.
    pub pool_authority: UncheckedAccount<'info>,
    /// CHECK: validated by the AMM.
    #[account(mut)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: validated by the AMM.
    #[account(mut)]
    pub position: UncheckedAccount<'info>,
    /// CHECK: receives token A fees.
    #[account(mut)]
    pub fee_account_a: UncheckedAccount<'info>,
    /// CHECK: receives token B fees.
    #[account(mut)]
    pub fee_account_b: UncheckedAccount<'info>,
    /// CHECK: validated by the AMM.
    #[account(mut)]
    pub token_a_vault: UncheckedAccount<'info>,
    /// CHECK: validated by the AMM.
    #[account(mut)]
    pub token_b_vault: UncheckedAccount<'info>,
    /// CHECK: mint of token A.
    pub token_a_mint: UncheckedAccount<'info>,
    /// CHECK: mint of token B.
    pub token_b_mint: UncheckedAccount<'info>,
    /// CHECK: validated by the AMM.
    pub position_nft_account: UncheckedAccount<'info>,
    /// CHECK: classic SPL Token program.
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: AMM event authority PDA.
    pub event_authority: UncheckedAccount<'info>,
    pub amm_program: Program<'info, CpAmm>,
}
