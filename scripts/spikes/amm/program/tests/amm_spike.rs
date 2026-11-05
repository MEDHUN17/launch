//! Phase 0 spike A, localnet half: Meteora DAMM v2 (devnet program + public config 11, both dumped by
//! ../fetch-fixtures.sh) driven by the PDA-only `amm_spike` program inside LiteSVM.
//!
//! Run: scripts/spikes/amm/fetch-fixtures.sh && (cd scripts/spikes/amm && cargo build-sbf --manifest-path program/Cargo.toml && cargo test -- --nocapture)

use {
    amm_spike::cp_amm,
    anchor_lang::{
        prelude::{pubkey, Clock, Pubkey},
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    base64::Engine,
    litesvm::LiteSVM,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const SPL_TOKEN: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const TOKEN_2022: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const NATIVE_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const CONFIG_11: Pubkey = pubkey!("AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW");
const COMPUTE_BUDGET: Pubkey = pubkey!("ComputeBudget111111111111111111111111111111");

// 5 SOL raise (the default min_raise), 1% success fee, pool_costs = 0 (the seeder PDA is funded separately).
// Constants computed with the SDK's concentrated-liquidity formulas (see docs/DECISIONS.md D-002).
const POOL_SOL: u64 = 4_950_000_000;
const POOL_TOKENS: u64 = 497_487_437_185_929;
const SQRT_PRICE: u128 = 58_187_709_620_360_055;
const LIQUIDITY: u128 = 28_947_654_534_773_209_928_812_175_280_602;

fn damm() -> Pubkey {
    cp_amm::ID
}

fn pda(seeds: &[&[u8]], program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program).0
}

fn mint_account(decimals: u8, supply: u64) -> Account {
    let mut data = vec![0u8; 82];
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data[44] = decimals;
    data[45] = 1; // initialized; no mint authority, no freeze authority (COption::None)
    Account {
        lamports: 1_461_600,
        data,
        owner: SPL_TOKEN,
        executable: false,
        rent_epoch: 0,
    }
}

fn token_account(mint: &Pubkey, owner: &Pubkey, amount: u64, native: bool) -> Account {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // initialized
    let reserve = 2_039_280u64;
    if native {
        data[109..113].copy_from_slice(&1u32.to_le_bytes()); // is_native = Some(reserve)
        data[113..121].copy_from_slice(&reserve.to_le_bytes());
    }
    let lamports = if native { reserve + amount } else { reserve };
    Account {
        lamports,
        data,
        owner: SPL_TOKEN,
        executable: false,
        rent_epoch: 0,
    }
}

fn amount_of(svm: &LiteSVM, key: &Pubkey) -> u64 {
    let acc = svm
        .get_account(key)
        .unwrap_or_else(|| panic!("missing account {key}"));
    u64::from_le_bytes(acc.data[64..72].try_into().unwrap())
}

struct Sent {
    cu: u64,
    logs: Vec<String>,
    unique_accounts: usize,
    message_bytes: usize,
}

fn send(svm: &mut LiteSVM, ixs: &[Instruction], payer: &Keypair) -> Sent {
    let mut all = vec![Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &[&[2u8][..], &1_400_000u32.to_le_bytes()].concat(),
        vec![],
    )];
    all.extend_from_slice(ixs);
    let msg = Message::new_with_blockhash(&all, Some(&payer.pubkey()), &svm.latest_blockhash());
    let unique_accounts = msg.account_keys.len();
    let versioned = VersionedMessage::Legacy(msg);
    let message_bytes = versioned.serialize().len();
    let tx = VersionedTransaction::try_new(versioned, &[payer]).unwrap();
    match svm.send_transaction(tx) {
        Ok(meta) => Sent {
            cu: meta.compute_units_consumed,
            logs: meta.logs,
            unique_accounts,
            message_bytes,
        },
        Err(e) => panic!(
            "transaction failed: {:?}\nlogs:\n{}",
            e.err,
            e.meta.logs.join("\n")
        ),
    }
}

fn load_fixture_config(svm: &mut LiteSVM) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/config11.json");
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("run fetch-fixtures.sh first")).unwrap();
    let acc = &json["account"];
    let data = base64::engine::general_purpose::STANDARD
        .decode(acc["data"][0].as_str().unwrap())
        .unwrap();
    let owner: Pubkey = acc["owner"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        owner,
        damm(),
        "config 11 must be owned by the DAMM v2 program"
    );
    svm.set_account(
        CONFIG_11,
        Account {
            lamports: acc["lamports"].as_u64().unwrap(),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

#[test]
fn no_instruction_can_unlock_a_permanently_locked_position() {
    // Evidence for "irreversible": none of the 39 instructions in the pinned IDL unlocks.
    let idl: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../idls/cp_amm.json")).unwrap();
    let names: Vec<&str> = idl["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 39);
    assert!(names.contains(&"permanent_lock_position"));
    assert!(
        !names.iter().any(|n| n.contains("unlock")),
        "unexpected unlock instruction: {names:?}"
    );
    println!(
        "SPIKE instructions that touch liquidity: {:?}",
        names
            .iter()
            .filter(|n| n.contains("liquidity") || n.contains("position"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn seed_lock_swap_and_harvest_by_cpi_from_pdas() {
    let mut svm = LiteSVM::new();
    svm.set_sysvar(&Clock {
        slot: 1_000,
        epoch_start_timestamp: 0,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp: 1_762_300_000,
    });
    let program_id = amm_spike::id();
    svm.add_program(
        program_id,
        include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/amm_spike.so"
        )),
    )
    .unwrap();
    let damm_so = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/damm_v2.so"
    ))
    .expect("run fetch-fixtures.sh first");
    svm.add_program(damm(), &damm_so).unwrap();
    load_fixture_config(&mut svm);
    if svm.get_account(&NATIVE_MINT).is_none() {
        svm.set_account(NATIVE_MINT, mint_account(9, 0)).unwrap();
    }

    // Token A = the new coin (all supply that goes to the pool), token B = wrapped SOL.
    let mint_a = Pubkey::new_unique();
    let mint_b = NATIVE_MINT;
    svm.set_account(mint_a, mint_account(6, 1_000_000_000_000_000))
        .unwrap();

    let seeder = pda(&[amm_spike::SEEDER], &program_id);
    let owner = pda(&[amm_spike::OWNER], &program_id);
    let nft_mint = pda(&[amm_spike::NFT_MINT], &program_id);
    svm.airdrop(&seeder, 1_000_000_000).unwrap(); // rent for pool, position, NFT mint and NFT account
    let payer_token_a = Pubkey::new_unique();
    let payer_token_b = Pubkey::new_unique();
    svm.set_account(
        payer_token_a,
        token_account(&mint_a, &seeder, POOL_TOKENS, false),
    )
    .unwrap();
    svm.set_account(
        payer_token_b,
        token_account(&mint_b, &seeder, POOL_SOL, true),
    )
    .unwrap();

    let (hi, lo) = if mint_a > mint_b {
        (mint_a, mint_b)
    } else {
        (mint_b, mint_a)
    };
    let pool = pda(
        &[b"pool", CONFIG_11.as_ref(), hi.as_ref(), lo.as_ref()],
        &damm(),
    );
    let position = pda(&[b"position", nft_mint.as_ref()], &damm());
    let position_nft_account = pda(&[b"position_nft_account", nft_mint.as_ref()], &damm());
    let vault_a = pda(&[b"token_vault", mint_a.as_ref(), pool.as_ref()], &damm());
    let vault_b = pda(&[b"token_vault", mint_b.as_ref(), pool.as_ref()], &damm());
    let event_authority = pda(&[b"__event_authority"], &damm());
    let pool_authority = pubkey!("HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC");
    assert_eq!(
        pool_authority,
        pda(&[b"pool_authority"], &damm()),
        "IDL pool_authority address"
    );

    let fee_payer = Keypair::new();
    svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();

    // 1. create the pool and lock the position, in one program instruction, signed only by PDAs
    let seed_ix = Instruction::new_with_bytes(
        program_id,
        &amm_spike::instruction::Seed {
            liquidity: LIQUIDITY,
            sqrt_price: SQRT_PRICE,
        }
        .data(),
        amm_spike::accounts::Seed {
            seeder,
            owner,
            position_nft_mint: nft_mint,
            position_nft_account,
            config: CONFIG_11,
            pool_authority,
            pool,
            position,
            token_a_mint: mint_a,
            token_b_mint: mint_b,
            token_a_vault: vault_a,
            token_b_vault: vault_b,
            payer_token_a,
            payer_token_b,
            token_program: SPL_TOKEN,
            token_2022_program: TOKEN_2022,
            system_program: system_program::ID,
            event_authority,
            amm_program: damm(),
        }
        .to_account_metas(None),
    );
    let seeded = send(&mut svm, &[seed_ix], &fee_payer);
    let dep_a = amount_of(&svm, &vault_a);
    let dep_b = amount_of(&svm, &vault_b);
    println!(
        "SPIKE seed: CU={} unique_accounts={} message_bytes={}",
        seeded.cu, seeded.unique_accounts, seeded.message_bytes
    );
    println!(
        "SPIKE deposit: token A {dep_a} (dust {}), wSOL {dep_b} (dust {} lamports)",
        POOL_TOKENS - dep_a,
        POOL_SOL - dep_b
    );
    println!(
        "SPIKE seeder lamports left: {} (spent {} on rent)",
        svm.get_account(&seeder).map_or(0, |a| a.lamports),
        1_000_000_000 - svm.get_account(&seeder).map_or(0, |a| a.lamports)
    );
    assert!(seeded
        .logs
        .iter()
        .any(|l| l.contains("unlocked_liquidity before lock")));
    assert!(dep_a <= POOL_TOKENS && dep_b <= POOL_SOL);
    assert_eq!(dep_a, POOL_TOKENS, "all pool tokens are deposited");
    let nft_holder = svm
        .get_account(&position_nft_account)
        .expect("NFT account exists");
    assert_eq!(
        &nft_holder.data[32..64],
        owner.as_ref(),
        "the owner PDA holds the position NFT"
    );

    let pos_acc = svm.get_account(&position).unwrap();
    let pos =
        cp_amm::accounts::Position::try_deserialize_unchecked(&mut &pos_acc.data[..]).unwrap();
    println!(
        "SPIKE position after lock: unlocked={} permanent_locked={} vested={}",
        pos.unlocked_liquidity, pos.permanent_locked_liquidity, pos.vested_liquidity
    );
    assert_eq!(pos.unlocked_liquidity, 0, "nothing left to withdraw");
    assert!(pos.permanent_locked_liquidity > 0);

    // 2. a trader buys the coin with 1 SOL and sells half of it back (fees accrue in token B only on config 11)
    let trader = Keypair::new();
    svm.airdrop(&trader.pubkey(), 1_000_000_000).unwrap();
    let user_a = Pubkey::new_unique();
    let user_b = Pubkey::new_unique();
    svm.set_account(user_a, token_account(&mint_a, &trader.pubkey(), 0, false))
        .unwrap();
    svm.set_account(
        user_b,
        token_account(&mint_b, &trader.pubkey(), 2_000_000_000, true),
    )
    .unwrap();
    let swap = |input: Pubkey, output: Pubkey, amount_in: u64| {
        Instruction::new_with_bytes(
            damm(),
            &cp_amm::client::args::Swap {
                _params: cp_amm::types::SwapParameters {
                    amount_in,
                    minimum_amount_out: 0,
                },
            }
            .data(),
            cp_amm::client::accounts::Swap {
                pool_authority,
                pool,
                input_token_account: input,
                output_token_account: output,
                token_a_vault: vault_a,
                token_b_vault: vault_b,
                token_a_mint: mint_a,
                token_b_mint: mint_b,
                payer: trader.pubkey(),
                token_a_program: SPL_TOKEN,
                token_b_program: SPL_TOKEN,
                referral_token_account: None,
                event_authority,
                program: damm(),
            }
            .to_account_metas(None),
        )
    };
    let buy_in = 1_000_000_000u64;
    send(&mut svm, &[swap(user_b, user_a, buy_in)], &trader);
    let bought = amount_of(&svm, &user_a);
    let sell_in = bought / 2;
    let b_before_sell = amount_of(&svm, &user_b);
    send(&mut svm, &[swap(user_a, user_b, sell_in)], &trader);
    let sell_out = amount_of(&svm, &user_b) - b_before_sell;
    println!("SPIKE swaps: bought {bought} tokens for {buy_in} lamports; sold {sell_in} tokens for {sell_out} lamports");
    let pool_price_after_buy = buy_in as f64 / bought as f64;
    let clearing = 5_000_000_000f64 / (1_000_000_000_000_000f64 - POOL_TOKENS as f64);
    println!("SPIKE first-trade avg price {pool_price_after_buy:.3e} lamports/unit vs clearing price {clearing:.3e} (includes 1% fee and slippage)");

    // 3. harvest by CPI as the owner PDA; fees must arrive in wSOL only
    let fee_a = Pubkey::new_unique();
    let fee_b = Pubkey::new_unique();
    svm.set_account(fee_a, token_account(&mint_a, &owner, 0, false))
        .unwrap();
    svm.set_account(fee_b, token_account(&mint_b, &owner, 0, true))
        .unwrap();
    let harvest_ix = Instruction::new_with_bytes(
        program_id,
        &amm_spike::instruction::Harvest {}.data(),
        amm_spike::accounts::Harvest {
            owner,
            pool_authority,
            pool,
            position,
            fee_account_a: fee_a,
            fee_account_b: fee_b,
            token_a_vault: vault_a,
            token_b_vault: vault_b,
            token_a_mint: mint_a,
            token_b_mint: mint_b,
            position_nft_account,
            token_program: SPL_TOKEN,
            event_authority,
            amm_program: damm(),
        }
        .to_account_metas(None),
    );
    let harvested = send(&mut svm, &[harvest_ix], &fee_payer);
    let (got_a, got_b) = (amount_of(&svm, &fee_a), amount_of(&svm, &fee_b));
    let volume_b = buy_in + sell_out; // B-denominated volume (fee charged on B in both directions)
    println!("SPIKE harvest: CU={} fee token A={got_a} fee wSOL={got_b}; wSOL volume={volume_b} => LP share {:.4}% of volume (expect ~0.8%: 1% fee less 20% protocol cut)", harvested.cu, got_b as f64 / volume_b as f64 * 100.0);
    assert_eq!(got_a, 0, "OnlyB fee mode: nothing in token A");
    assert!(got_b > 0, "fees accrued in wSOL");
}

// ---- squatting probe (Q-02): can someone create the (new coin, wSOL) pool without holding any of the coin? ----

fn try_send_with(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    signers: &[&Keypair],
) -> Result<u64, String> {
    let mut all = vec![Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &[&[2u8][..], &1_400_000u32.to_le_bytes()].concat(),
        vec![],
    )];
    all.extend_from_slice(ixs);
    let msg =
        Message::new_with_blockhash(&all, Some(&signers[0].pubkey()), &svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    match svm.send_transaction(tx) {
        Ok(meta) => Ok(meta.compute_units_consumed),
        Err(e) => Err(format!(
            "{:?} | last logs: {:?}",
            e.err,
            e.meta.logs.iter().rev().take(10).collect::<Vec<_>>()
        )),
    }
}

#[test]
fn squatting_probe_one_sided_pool_without_holding_the_coin() {
    let mut svm = LiteSVM::new();
    svm.set_sysvar(&Clock {
        slot: 1_000,
        epoch_start_timestamp: 0,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp: 1_762_300_000,
    });
    let damm_so = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/damm_v2.so"
    ))
    .expect("run fetch-fixtures.sh first");
    svm.add_program(damm(), &damm_so).unwrap();
    load_fixture_config(&mut svm);
    if svm.get_account(&NATIVE_MINT).is_none() {
        svm.set_account(NATIVE_MINT, mint_account(9, 0)).unwrap();
    }
    let mint_a = Pubkey::new_unique(); // the "new coin": the attacker holds none of it
    let mint_b = NATIVE_MINT;
    svm.set_account(mint_a, mint_account(6, 1_000_000_000_000_000))
        .unwrap();

    let attacker = Keypair::new();
    let nft = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 2_000_000_000).unwrap();
    let atk_a = Pubkey::new_unique(); // empty token-A account: anyone can create one
    let atk_b = Pubkey::new_unique();
    svm.set_account(atk_a, token_account(&mint_a, &attacker.pubkey(), 0, false))
        .unwrap();
    svm.set_account(
        atk_b,
        token_account(&mint_b, &attacker.pubkey(), 500_000_000, true),
    )
    .unwrap();

    let (hi, lo) = if mint_a > mint_b {
        (mint_a, mint_b)
    } else {
        (mint_b, mint_a)
    };
    let pool = pda(
        &[b"pool", CONFIG_11.as_ref(), hi.as_ref(), lo.as_ref()],
        &damm(),
    );
    let position = pda(&[b"position", nft.pubkey().as_ref()], &damm());
    let position_nft_account = pda(&[b"position_nft_account", nft.pubkey().as_ref()], &damm());
    let vault_a = pda(&[b"token_vault", mint_a.as_ref(), pool.as_ref()], &damm());
    let vault_b = pda(&[b"token_vault", mint_b.as_ref(), pool.as_ref()], &damm());
    let event_authority = pda(&[b"__event_authority"], &damm());
    let pool_authority = pda(&[b"pool_authority"], &damm());

    // price at the top of the range: token A needed = 0, token B needed > 0 (liquidity computed with the SDK formula)
    let sqrt_max: u128 = 79_226_673_521_066_979_257_578_248_091;
    let liquidity = squat_liquidity();
    let init = |nft_key: Pubkey, position: Pubkey, position_nft_account: Pubkey| {
        Instruction::new_with_bytes(
            damm(),
            &cp_amm::client::args::InitializePool {
                params: cp_amm::types::InitializePoolParameters {
                    liquidity,
                    sqrt_price: sqrt_max,
                    activation_point: None,
                },
            }
            .data(),
            cp_amm::client::accounts::InitializePool {
                creator: attacker.pubkey(),
                position_nft_mint: nft_key,
                position_nft_account,
                payer: attacker.pubkey(),
                config: CONFIG_11,
                pool_authority,
                pool,
                position,
                token_a_mint: mint_a,
                token_b_mint: mint_b,
                token_a_vault: vault_a,
                token_b_vault: vault_b,
                payer_token_a: atk_a,
                payer_token_b: atk_b,
                token_a_program: SPL_TOKEN,
                token_b_program: SPL_TOKEN,
                token_2022_program: TOKEN_2022,
                system_program: system_program::ID,
                event_authority,
                program: damm(),
            }
            .to_account_metas(None),
        )
    };
    let first = try_send_with(
        &mut svm,
        &[init(nft.pubkey(), position, position_nft_account)],
        &[&attacker, &nft],
    );
    println!(
        "SQUAT attacker pool creation (zero token A, wSOL only, price at top of range): {:?}",
        first.as_ref().map(|cu| format!("ACCEPTED, CU={cu}"))
    );
    first.expect(
        "the attacker's one-sided pool creation must be accepted; if the AMM now rejects it, re-evaluate D-002 and Q-02",
    );
    assert!(svm.get_account(&pool).is_some(), "squatted pool exists");

    // Prequel's own `seed` for the same (config, mints) must now fail: the pool address is taken.
    let program_id = amm_spike::id();
    svm.add_program(
        program_id,
        include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/amm_spike.so"
        )),
    )
    .unwrap();
    let seeder = pda(&[amm_spike::SEEDER], &program_id);
    let owner = pda(&[amm_spike::OWNER], &program_id);
    let our_nft_mint = pda(&[amm_spike::NFT_MINT], &program_id);
    svm.airdrop(&seeder, 1_000_000_000).unwrap();
    let payer_token_a = Pubkey::new_unique();
    let payer_token_b = Pubkey::new_unique();
    svm.set_account(
        payer_token_a,
        token_account(&mint_a, &seeder, POOL_TOKENS, false),
    )
    .unwrap();
    svm.set_account(
        payer_token_b,
        token_account(&mint_b, &seeder, POOL_SOL, true),
    )
    .unwrap();
    let our_position = pda(&[b"position", our_nft_mint.as_ref()], &damm());
    let our_nft_account = pda(&[b"position_nft_account", our_nft_mint.as_ref()], &damm());
    let fee_payer = Keypair::new();
    svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();
    let seed_ix = Instruction::new_with_bytes(
        program_id,
        &amm_spike::instruction::Seed {
            liquidity: LIQUIDITY,
            sqrt_price: SQRT_PRICE,
        }
        .data(),
        amm_spike::accounts::Seed {
            seeder,
            owner,
            position_nft_mint: our_nft_mint,
            position_nft_account: our_nft_account,
            config: CONFIG_11,
            pool_authority,
            pool,
            position: our_position,
            token_a_mint: mint_a,
            token_b_mint: mint_b,
            token_a_vault: vault_a,
            token_b_vault: vault_b,
            payer_token_a,
            payer_token_b,
            token_program: SPL_TOKEN,
            token_2022_program: TOKEN_2022,
            system_program: system_program::ID,
            event_authority,
            amm_program: damm(),
        }
        .to_account_metas(None),
    );
    let ours = try_send_with(&mut svm, &[seed_ix], &[&fee_payer]);
    println!("SQUAT Prequel's own seed for the same address: {ours:?}");
    let err = ours.expect_err("Prequel's seed must fail once the pool address is taken");
    assert!(err.contains("already in use"), "unexpected failure: {err}");
}

fn squat_liquidity() -> u128 {
    // L = (0.5 SOL << 128) / (sqrt_max - sqrt_min), computed offline (Python) because it overflows u128 here.
    LIQUIDITY_SQUAT
}
const LIQUIDITY_SQUAT: u128 = 2_147_524_008_000_000_000; // (0.5 SOL << 128) / (sqrt_max - sqrt_min)
