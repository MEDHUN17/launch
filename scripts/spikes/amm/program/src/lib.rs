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
// probe2: the coin mint and the seeder's two deposit token accounts are PDAs created by `launch`.
pub const MINT: &[u8] = b"mint";
pub const SEEDER_A: &[u8] = b"seeder_a";
pub const SEEDER_B: &[u8] = b"seeder_b";
pub const COIN_SUPPLY: u64 = 1_000_000_000_000_000;
pub const COIN_DECIMALS: u8 = 6;
pub const NATIVE_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
pub const SPL_TOKEN_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// How `launch` creates its own PDAs (mint, seeder_a, seeder_b) when they may already hold lamports.
pub const CREATE_NAIVE: u8 = 0; // plain system create_account (fails on any pre-funded address)
pub const CREATE_FUND_ALLOCATE_ASSIGN: u8 = 1; // pre-1.0 Anchor `init` path: top up, allocate, assign
pub const CREATE_ALLOW_PREFUND: u8 = 2; // Anchor 1.2.2 `init` path: system CreateAccountAllowPrefund

fn create_pda<'info>(
    system_program: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    payer_seeds: &[&[u8]],
    target: &AccountInfo<'info>,
    target_seeds: &[&[u8]],
    space: usize,
    owner: &Pubkey,
    mode: u8,
) -> Result<()> {
    use anchor_lang::system_program as sp;
    let rent = Rent::get()?.minimum_balance(space);
    let both: &[&[&[u8]]] = &[payer_seeds, target_seeds];
    let current = target.lamports();
    if current == 0 || mode == CREATE_NAIVE {
        return sp::create_account(
            CpiContext::new_with_signer(
                system_program.key(),
                sp::CreateAccount {
                    from: payer.clone(),
                    to: target.clone(),
                },
                both,
            ),
            rent,
            space as u64,
            owner,
        );
    }
    let need = rent.max(1).saturating_sub(current);
    if mode == CREATE_ALLOW_PREFUND {
        return sp::create_account_allow_prefund(
            CpiContext::new_with_signer(
                system_program.key(),
                sp::CreateAccountAllowPrefund {
                    from: payer.clone(),
                    to: target.clone(),
                },
                both,
            ),
            need,
            space as u64,
            owner,
        );
    }
    if need > 0 {
        sp::transfer(
            CpiContext::new_with_signer(
                system_program.key(),
                sp::Transfer {
                    from: payer.clone(),
                    to: target.clone(),
                },
                both,
            ),
            need,
        )?;
    }
    let target_only: &[&[&[u8]]] = &[target_seeds];
    sp::allocate(
        CpiContext::new_with_signer(
            system_program.key(),
            sp::Allocate {
                account_to_allocate: target.clone(),
            },
            target_only,
        ),
        space as u64,
    )?;
    sp::assign(
        CpiContext::new_with_signer(
            system_program.key(),
            sp::Assign {
                account_to_assign: target.clone(),
            },
            target_only,
        ),
        owner,
    )
}

/// Classic SPL Token instruction, hand-encoded (no anchor-spl dependency).
fn token_ix<'info>(
    token_program: &AccountInfo<'info>,
    metas: Vec<AccountMeta>,
    infos: &[AccountInfo<'info>],
    data: Vec<u8>,
    signers: &[&[&[u8]]],
) -> Result<()> {
    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: token_program.key(),
        accounts: metas,
        data,
    };
    anchor_lang::solana_program::program::invoke_signed(&ix, infos, signers).map_err(Into::into)
}

#[program]
pub mod amm_spike {
    use super::*;

    /// probe2 Q1: atomic launch, signed only by PDAs. Creates the coin mint PDA (6 decimals, no freeze authority),
    /// mints the whole supply to `seeder_a`, revokes the mint authority, wraps `pool_sol` lamports into `seeder_b`
    /// (native mint), creates the DAMM v2 pool (coin, wSOL) on the given config and locks the position permanently.
    pub fn launch(
        ctx: Context<Launch>,
        liquidity: u128,
        sqrt_price: u128,
        pool_sol: u64,
        create_mode: u8,
    ) -> Result<()> {
        let a = &ctx.accounts;
        let seeder_seeds: &[&[u8]] = &[SEEDER, &[ctx.bumps.seeder]];
        let mint_seeds: &[&[u8]] = &[MINT, &[ctx.bumps.coin_mint]];
        let sa_seeds: &[&[u8]] = &[SEEDER_A, &[ctx.bumps.seeder_coin]];
        let sb_seeds: &[&[u8]] = &[SEEDER_B, &[ctx.bumps.seeder_wsol]];
        let nft_seeds: &[&[u8]] = &[NFT_MINT, &[ctx.bumps.position_nft_mint]];
        let owner_seeds: &[&[u8]] = &[OWNER, &[ctx.bumps.owner]];
        let sys = a.system_program.to_account_info();
        let tok = a.token_program.to_account_info();
        let seeder = a.seeder.to_account_info();
        let mint = a.coin_mint.to_account_info();
        let sa = a.seeder_coin.to_account_info();
        let sb = a.seeder_wsol.to_account_info();
        let native = a.native_mint.to_account_info();
        require_keys_eq!(tok.key(), SPL_TOKEN_ID);
        require_keys_eq!(native.key(), NATIVE_MINT);

        // (a) mint PDA: InitializeMint2 { decimals 6, mint_authority = seeder, freeze_authority = None }
        create_pda(
            &sys,
            &seeder,
            seeder_seeds,
            &mint,
            mint_seeds,
            82,
            &SPL_TOKEN_ID,
            create_mode,
        )?;
        let mut d = vec![20u8, COIN_DECIMALS];
        d.extend_from_slice(seeder.key.as_ref());
        d.push(0); // COption::None freeze authority
        token_ix(
            &tok,
            vec![AccountMeta::new(mint.key(), false)],
            &[mint.clone()],
            d,
            &[],
        )?;

        // (b) seeder_a token account (coin), then MintTo the whole supply
        create_pda(
            &sys,
            &seeder,
            seeder_seeds,
            &sa,
            sa_seeds,
            165,
            &SPL_TOKEN_ID,
            create_mode,
        )?;
        let mut d = vec![18u8];
        d.extend_from_slice(seeder.key.as_ref());
        token_ix(
            &tok,
            vec![
                AccountMeta::new(sa.key(), false),
                AccountMeta::new_readonly(mint.key(), false),
            ],
            &[sa.clone(), mint.clone()],
            d,
            &[],
        )?;
        let mut d = vec![7u8];
        d.extend_from_slice(&COIN_SUPPLY.to_le_bytes());
        token_ix(
            &tok,
            vec![
                AccountMeta::new(mint.key(), false),
                AccountMeta::new(sa.key(), false),
                AccountMeta::new_readonly(seeder.key(), true),
            ],
            &[mint.clone(), sa.clone(), seeder.clone()],
            d,
            &[seeder_seeds],
        )?;

        // (f) revoke the mint authority: SetAuthority { MintTokens, None }
        token_ix(
            &tok,
            vec![
                AccountMeta::new(mint.key(), false),
                AccountMeta::new_readonly(seeder.key(), true),
            ],
            &[mint.clone(), seeder.clone()],
            vec![6u8, 0, 0],
            &[seeder_seeds],
        )?;

        // (c) seeder_b wSOL account: create, InitializeAccount3(native mint), fund, SyncNative
        create_pda(
            &sys,
            &seeder,
            seeder_seeds,
            &sb,
            sb_seeds,
            165,
            &SPL_TOKEN_ID,
            create_mode,
        )?;
        let mut d = vec![18u8];
        d.extend_from_slice(seeder.key.as_ref());
        token_ix(
            &tok,
            vec![
                AccountMeta::new(sb.key(), false),
                AccountMeta::new_readonly(native.key(), false),
            ],
            &[sb.clone(), native.clone()],
            d,
            &[],
        )?;
        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                sys.key(),
                anchor_lang::system_program::Transfer {
                    from: seeder.clone(),
                    to: sb.clone(),
                },
                &[seeder_seeds],
            ),
            pool_sol,
        )?;
        token_ix(
            &tok,
            vec![AccountMeta::new(sb.key(), false)],
            &[sb.clone()],
            vec![17u8],
            &[],
        )?;

        // (d) create the pool
        let init_accounts = amm::InitializePool {
            creator: a.owner.to_account_info(),
            position_nft_mint: a.position_nft_mint.to_account_info(),
            position_nft_account: a.position_nft_account.to_account_info(),
            payer: seeder.clone(),
            config: a.config.to_account_info(),
            pool_authority: a.pool_authority.to_account_info(),
            pool: a.pool.to_account_info(),
            position: a.position.to_account_info(),
            token_a_mint: mint.clone(),
            token_b_mint: native.clone(),
            token_a_vault: a.token_a_vault.to_account_info(),
            token_b_vault: a.token_b_vault.to_account_info(),
            payer_token_a: sa.clone(),
            payer_token_b: sb.clone(),
            token_a_program: tok.clone(),
            token_b_program: tok.clone(),
            token_2022_program: a.token_2022_program.to_account_info(),
            system_program: sys.clone(),
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

        // (e) lock everything the position holds
        let unlocked = {
            let data = a.position.try_borrow_data()?;
            cp_amm::accounts::Position::try_deserialize(&mut &data[..])?.unlocked_liquidity
        };
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
        msg!("probe2: launched and locked {}", unlocked);
        Ok(())
    }

    /// probe2 Q3: forward an IDL-encoded DAMM v2 instruction (accounts = remaining_accounts) with the `owner`
    /// PDA as a signer.
    ///
    /// UNSAFE BY DESIGN, SPIKE ONLY: anyone can call this and make the program sign as the position owner
    /// (it can split the locked position away, among other things). It exists to test what the owner PDA could
    /// do after the lock. A real program must never expose a generic owner-signed proxy, and must never sign
    /// `split_position`/`split_position2` or transfer the position NFT (see docs/DECISIONS.md D-002).
    pub fn owner_cpi<'info>(ctx: Context<'info, OwnerCpi<'info>>, data: Vec<u8>) -> Result<()> {
        let owner_key = ctx.accounts.owner.key();
        let metas: Vec<AccountMeta> = ctx
            .remaining_accounts
            .iter()
            .map(|acc| AccountMeta {
                pubkey: *acc.key,
                is_signer: acc.is_signer || *acc.key == owner_key,
                is_writable: acc.is_writable,
            })
            .collect();
        let ix = anchor_lang::solana_program::instruction::Instruction {
            program_id: cp_amm::ID,
            accounts: metas,
            data,
        };
        let mut infos = ctx.remaining_accounts.to_vec();
        infos.push(ctx.accounts.owner.to_account_info());
        infos.push(ctx.accounts.amm_program.to_account_info());
        let owner_seeds: &[&[u8]] = &[OWNER, &[ctx.bumps.owner]];
        anchor_lang::solana_program::program::invoke_signed(&ix, &infos, &[owner_seeds])
            .map_err(Into::into)
    }

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
pub struct Launch<'info> {
    /// CHECK: system-owned PDA; pays all rent and the wSOL deposit, owns the deposit accounts, is the
    /// (revoked-in-the-same-instruction) mint authority.
    #[account(mut, seeds = [SEEDER], bump)]
    pub seeder: UncheckedAccount<'info>,
    /// CHECK: system-owned PDA; owns the position NFT.
    #[account(seeds = [OWNER], bump)]
    pub owner: UncheckedAccount<'info>,
    /// CHECK: coin mint PDA, created here.
    #[account(mut, seeds = [MINT], bump)]
    pub coin_mint: UncheckedAccount<'info>,
    /// CHECK: seeder's coin token account PDA, created here.
    #[account(mut, seeds = [SEEDER_A], bump)]
    pub seeder_coin: UncheckedAccount<'info>,
    /// CHECK: seeder's wSOL token account PDA, created here.
    #[account(mut, seeds = [SEEDER_B], bump)]
    pub seeder_wsol: UncheckedAccount<'info>,
    /// CHECK: PDA that signs as the position NFT mint; the AMM creates the account.
    #[account(mut, seeds = [NFT_MINT], bump)]
    pub position_nft_mint: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub position_nft_account: UncheckedAccount<'info>,
    /// CHECK: AMM config (public config 11).
    pub config: UncheckedAccount<'info>,
    /// CHECK: AMM pool authority PDA.
    pub pool_authority: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub position: UncheckedAccount<'info>,
    /// CHECK: checked against NATIVE_MINT in the handler.
    pub native_mint: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub token_a_vault: UncheckedAccount<'info>,
    /// CHECK: derived and validated by the AMM.
    #[account(mut)]
    pub token_b_vault: UncheckedAccount<'info>,
    /// CHECK: checked against the classic SPL Token id in the handler.
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: Token-2022 program (the AMM uses it for the position NFT).
    pub token_2022_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: AMM event authority PDA.
    pub event_authority: UncheckedAccount<'info>,
    pub amm_program: Program<'info, CpAmm>,
}

#[derive(Accounts)]
pub struct OwnerCpi<'info> {
    /// CHECK: system-owned PDA that holds the position NFT.
    #[account(seeds = [OWNER], bump)]
    pub owner: UncheckedAccount<'info>,
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
