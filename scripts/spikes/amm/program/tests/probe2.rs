//! probe2 (scratch): atomic launch (Q1), pre-funded addresses (Q2), instructions after the lock (Q3) and pool
//! sizes (Q4), all against the real devnet DAMM v2 binary + public config 11 inside LiteSVM.

use {
    amm_spike::cp_amm,
    anchor_lang::{
        prelude::{pubkey, Clock, Pubkey},
        solana_program::{
            instruction::{AccountMeta, Instruction},
            system_instruction, system_program,
        },
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
const POOL_TOKENS: u64 = 497_487_437_185_929;

struct Size {
    name: &'static str,
    pool_sol: u64,
    sqrt_price: u128,
    liquidity: u128,
}
const SIZES: [Size; 3] = [
    Size {
        name: "5 SOL",
        pool_sol: 4_950_000_000,
        sqrt_price: 58_187_709_620_360_055,
        liquidity: 28_947_654_534_773_209_928_812_175_280_602,
    },
    Size {
        name: "50 SOL",
        pool_sol: 49_500_000_000,
        sqrt_price: 184_005_694_228_829_291,
        liquidity: 91_540_521_249_730_575_089_181_662_024_690,
    },
    Size {
        name: "500 SOL",
        pool_sol: 495_000_000_000,
        sqrt_price: 581_877_096_203_600_552,
        liquidity: 289_476_545_349_645_544_174_240_085_709_385,
    },
];

fn damm() -> Pubkey {
    cp_amm::ID
}
fn pda(seeds: &[&[u8]], program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program).0
}

fn token_account(mint: &Pubkey, owner: &Pubkey, amount: u64, native: bool) -> Account {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    let reserve = 2_039_280u64;
    if native {
        data[109..113].copy_from_slice(&1u32.to_le_bytes());
        data[113..121].copy_from_slice(&reserve.to_le_bytes());
    }
    Account {
        lamports: if native { reserve + amount } else { reserve },
        data,
        owner: SPL_TOKEN,
        executable: false,
        rent_epoch: 0,
    }
}

struct Env {
    svm: LiteSVM,
    program_id: Pubkey,
    fee_payer: Keypair,
    seeder: Pubkey,
    owner: Pubkey,
    nft_mint: Pubkey,
    mint: Pubkey,
    seeder_a: Pubkey,
    seeder_b: Pubkey,
    pool: Pubkey,
    position: Pubkey,
    nft_account: Pubkey,
    vault_a: Pubkey,
    vault_b: Pubkey,
    event_authority: Pubkey,
    pool_authority: Pubkey,
}

impl Env {
    fn new(seeder_lamports: u64) -> Env {
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
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/config11.json");
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let acc = &json["account"];
        let data = base64::engine::general_purpose::STANDARD
            .decode(acc["data"][0].as_str().unwrap())
            .unwrap();
        svm.set_account(
            CONFIG_11,
            Account {
                lamports: acc["lamports"].as_u64().unwrap(),
                data,
                owner: damm(),
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();
        if svm.get_account(&NATIVE_MINT).is_none() {
            let mut data = vec![0u8; 82];
            data[44] = 9;
            data[45] = 1;
            svm.set_account(
                NATIVE_MINT,
                Account {
                    lamports: 1_461_600,
                    data,
                    owner: SPL_TOKEN,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
        }
        let seeder = pda(&[amm_spike::SEEDER], &program_id);
        let owner = pda(&[amm_spike::OWNER], &program_id);
        let nft_mint = pda(&[amm_spike::NFT_MINT], &program_id);
        let mint = pda(&[amm_spike::MINT], &program_id);
        let seeder_a = pda(&[amm_spike::SEEDER_A], &program_id);
        let seeder_b = pda(&[amm_spike::SEEDER_B], &program_id);
        let (hi, lo) = if mint > NATIVE_MINT {
            (mint, NATIVE_MINT)
        } else {
            (NATIVE_MINT, mint)
        };
        let pool = pda(
            &[b"pool", CONFIG_11.as_ref(), hi.as_ref(), lo.as_ref()],
            &damm(),
        );
        let position = pda(&[b"position", nft_mint.as_ref()], &damm());
        let nft_account = pda(&[b"position_nft_account", nft_mint.as_ref()], &damm());
        let vault_a = pda(&[b"token_vault", mint.as_ref(), pool.as_ref()], &damm());
        let vault_b = pda(
            &[b"token_vault", NATIVE_MINT.as_ref(), pool.as_ref()],
            &damm(),
        );
        let event_authority = pda(&[b"__event_authority"], &damm());
        let pool_authority = pda(&[b"pool_authority"], &damm());
        svm.set_account(
            seeder,
            Account {
                lamports: seeder_lamports,
                data: vec![],
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();
        let fee_payer = Keypair::new();
        svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();
        Env {
            svm,
            program_id,
            fee_payer,
            seeder,
            owner,
            nft_mint,
            mint,
            seeder_a,
            seeder_b,
            pool,
            position,
            nft_account,
            vault_a,
            vault_b,
            event_authority,
            pool_authority,
        }
    }

    fn launch_ix(&self, s: &Size, mode: u8) -> Instruction {
        Instruction::new_with_bytes(
            self.program_id,
            &amm_spike::instruction::Launch {
                liquidity: s.liquidity,
                sqrt_price: s.sqrt_price,
                pool_sol: s.pool_sol,
                create_mode: mode,
            }
            .data(),
            amm_spike::accounts::Launch {
                seeder: self.seeder,
                owner: self.owner,
                coin_mint: self.mint,
                seeder_coin: self.seeder_a,
                seeder_wsol: self.seeder_b,
                position_nft_mint: self.nft_mint,
                position_nft_account: self.nft_account,
                config: CONFIG_11,
                pool_authority: self.pool_authority,
                pool: self.pool,
                position: self.position,
                native_mint: NATIVE_MINT,
                token_a_vault: self.vault_a,
                token_b_vault: self.vault_b,
                token_program: SPL_TOKEN,
                token_2022_program: TOKEN_2022,
                system_program: system_program::ID,
                event_authority: self.event_authority,
                amm_program: damm(),
            }
            .to_account_metas(None),
        )
    }

    fn lamports(&self, k: &Pubkey) -> u64 {
        self.svm.get_account(k).map_or(0, |a| a.lamports)
    }
    fn amount(&self, k: &Pubkey) -> u64 {
        let acc = self
            .svm
            .get_account(k)
            .unwrap_or_else(|| panic!("missing {k}"));
        u64::from_le_bytes(acc.data[64..72].try_into().unwrap())
    }
    fn pos(&self, k: &Pubkey) -> String {
        match self.svm.get_account(k) {
            Some(acc) if acc.data.len() > 8 => {
                let p = cp_amm::accounts::Position::try_deserialize_unchecked(&mut &acc.data[..])
                    .unwrap();
                format!(
                    "unlocked={} permanent_locked={} vested={} inner_vesting? fee_b_pending={}",
                    p.unlocked_liquidity,
                    p.permanent_locked_liquidity,
                    p.vested_liquidity,
                    p.fee_b_pending
                )
            }
            _ => "ACCOUNT GONE".into(),
        }
    }
    fn pool_liquidity(&self) -> String {
        let acc = self.svm.get_account(&self.pool).unwrap();
        let p = cp_amm::accounts::Pool::try_deserialize_unchecked(&mut &acc.data[..]).unwrap();
        format!(
            "pool.liquidity={} pool.permanent_lock_liquidity={}",
            p.liquidity, p.permanent_lock_liquidity
        )
    }
}

struct Sent {
    cu: u64,
    logs: Vec<String>,
    unique_accounts: usize,
    message_bytes: usize,
}

fn send(svm: &mut LiteSVM, ixs: &[Instruction], signers: &[&Keypair]) -> Result<Sent, String> {
    let mut all = vec![Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &[&[2u8][..], &1_400_000u32.to_le_bytes()].concat(),
        vec![],
    )];
    all.extend_from_slice(ixs);
    svm.expire_blockhash();
    let msg =
        Message::new_with_blockhash(&all, Some(&signers[0].pubkey()), &svm.latest_blockhash());
    let unique_accounts = msg.account_keys.len();
    let versioned = VersionedMessage::Legacy(msg);
    let message_bytes = versioned.serialize().len();
    let tx = VersionedTransaction::try_new(versioned, signers).map_err(|e| format!("sign: {e}"))?;
    match svm.send_transaction(tx) {
        Ok(meta) => Ok(Sent {
            cu: meta.compute_units_consumed,
            logs: meta.logs,
            unique_accounts,
            message_bytes,
        }),
        Err(e) => {
            let interesting: Vec<&String> = e
                .meta
                .logs
                .iter()
                .filter(|l| {
                    l.contains("Error")
                        || l.contains("error")
                        || l.contains("failed")
                        || l.contains("already in use")
                        || l.contains("insufficient")
                        || l.contains("Insufficient")
                })
                .collect();
            Err(format!("{:?} | {:?}", e.err, interesting))
        }
    }
}

// ------------------------------------------------------------------------------------------------ Q1 + Q4

fn run_launch(s: &Size, verbose: bool) -> Env {
    let start = s.pool_sol + 1_000_000_000;
    let mut e = Env::new(start);
    let ix = e.launch_ix(s, amm_spike::CREATE_FUND_ALLOCATE_ASSIGN);
    let fp = e.fee_payer.insecure_clone();
    let r = send(&mut e.svm, &[ix], &[&fp]).unwrap_or_else(|err| panic!("launch failed: {err}"));
    let dep_a = e.amount(&e.vault_a);
    let dep_b = e.amount(&e.vault_b);
    let seeder_left = e.lamports(&e.seeder);
    let our_rent = e.lamports(&e.mint) + 2_039_280 * 2; // seeder_b also keeps the wSOL dust above its reserve
    let amm_rent: u64 = [
        e.pool,
        e.position,
        e.vault_a,
        e.vault_b,
        e.nft_mint,
        e.nft_account,
    ]
    .iter()
    .map(|k| {
        e.svm
            .minimum_balance_for_rent_exemption(e.svm.get_account(k).unwrap().data.len())
    })
    .sum();
    let spent = start - seeder_left;
    println!(
        "Q4 {}: CU={} unique_accounts={} legacy_message_bytes={} (tx bytes with 1 sig = {}) | deposit token A {} (dust {}), wSOL {} (dust {} lamports) | seeder spent {} = wSOL wrapped {} + rent {} (AMM accounts {}, our mint+2 token accounts {})",
        s.name,
        r.cu,
        r.unique_accounts,
        r.message_bytes,
        r.message_bytes + 65,
        dep_a,
        POOL_TOKENS as i128 - dep_a as i128,
        dep_b,
        s.pool_sol - dep_b,
        spent,
        s.pool_sol,
        spent - s.pool_sol,
        amm_rent,
        our_rent
    );
    if verbose {
        for l in r
            .logs
            .iter()
            .filter(|l| l.contains("consumed") || l.contains("probe2"))
        {
            println!("   log: {l}");
        }
        for (n, k) in [
            ("mint", e.mint),
            ("seeder_a", e.seeder_a),
            ("seeder_b", e.seeder_b),
            ("pool", e.pool),
            ("vault_a", e.vault_a),
            ("vault_b", e.vault_b),
            ("position", e.position),
            ("nft_mint", e.nft_mint),
            ("nft_account", e.nft_account),
        ] {
            let a = e.svm.get_account(&k).unwrap();
            println!(
                "   created {n:12} {k} owner={} len={} lamports={}",
                a.owner,
                a.data.len(),
                a.lamports
            );
        }
    }
    // assertions shared by all sizes
    let m = e.svm.get_account(&e.mint).unwrap();
    assert_eq!(m.owner, SPL_TOKEN);
    assert_eq!(&m.data[0..4], &[0, 0, 0, 0], "mint authority is None");
    assert_eq!(
        u64::from_le_bytes(m.data[36..44].try_into().unwrap()),
        amm_spike::COIN_SUPPLY
    );
    assert_eq!(m.data[44], 6, "decimals");
    assert_eq!(m.data[45], 1, "initialized");
    assert_eq!(&m.data[46..50], &[0, 0, 0, 0], "freeze authority is None");
    assert_eq!(dep_a, POOL_TOKENS);
    assert!(dep_b <= s.pool_sol && s.pool_sol - dep_b < 1_000_000);
    assert_eq!(e.amount(&e.seeder_a), amm_spike::COIN_SUPPLY - dep_a);
    assert_eq!(e.amount(&e.seeder_b), s.pool_sol - dep_b);
    let nft = e.svm.get_account(&e.nft_account).unwrap();
    assert_eq!(nft.owner, TOKEN_2022);
    assert_eq!(
        &nft.data[32..64],
        e.owner.as_ref(),
        "owner PDA holds the NFT"
    );
    assert_eq!(u64::from_le_bytes(nft.data[64..72].try_into().unwrap()), 1);
    let pos_acc = e.svm.get_account(&e.position).unwrap();
    let p = cp_amm::accounts::Position::try_deserialize_unchecked(&mut &pos_acc.data[..]).unwrap();
    assert_eq!(p.unlocked_liquidity, 0);
    assert_eq!(p.permanent_locked_liquidity, s.liquidity);
    e
}

#[test]
fn q1_atomic_launch_signed_only_by_pdas() {
    let mut e = run_launch(&SIZES[0], true);
    let m = e.svm.get_account(&e.mint).unwrap();
    println!(
        "Q1 mint {}: mint_authority tag={:?} (0 = None) supply={} decimals={} freeze_authority tag={:?} (0 = None)",
        e.mint,
        &m.data[0..4],
        u64::from_le_bytes(m.data[36..44].try_into().unwrap()),
        m.data[44],
        &m.data[46..50]
    );
    println!(
        "Q1 vault_a={} vault_b={} seeder_a left={} seeder_b left={} nft holder={} (owner PDA {}) position: {} | {}",
        e.amount(&e.vault_a),
        e.amount(&e.vault_b),
        e.amount(&e.seeder_a),
        e.amount(&e.seeder_b),
        Pubkey::try_from(&e.svm.get_account(&e.nft_account).unwrap().data[32..64]).unwrap(),
        e.owner,
        e.pos(&e.position),
        e.pool_liquidity()
    );
    // the pool trades: buy with 1 SOL
    let trader = Keypair::new();
    e.svm.airdrop(&trader.pubkey(), 1_000_000_000).unwrap();
    let (ua, ub) = (Pubkey::new_unique(), Pubkey::new_unique());
    e.svm
        .set_account(ua, token_account(&e.mint, &trader.pubkey(), 0, false))
        .unwrap();
    e.svm
        .set_account(
            ub,
            token_account(&NATIVE_MINT, &trader.pubkey(), 1_000_000_000, true),
        )
        .unwrap();
    let swap = Instruction::new_with_bytes(
        damm(),
        &cp_amm::client::args::Swap {
            _params: cp_amm::types::SwapParameters {
                amount_in: 1_000_000_000,
                minimum_amount_out: 0,
            },
        }
        .data(),
        cp_amm::client::accounts::Swap {
            pool_authority: e.pool_authority,
            pool: e.pool,
            input_token_account: ub,
            output_token_account: ua,
            token_a_vault: e.vault_a,
            token_b_vault: e.vault_b,
            token_a_mint: e.mint,
            token_b_mint: NATIVE_MINT,
            payer: trader.pubkey(),
            token_a_program: SPL_TOKEN,
            token_b_program: SPL_TOKEN,
            referral_token_account: None,
            event_authority: e.event_authority,
            program: damm(),
        }
        .to_account_metas(None),
    );
    let r = send(&mut e.svm, &[swap], &[&trader]).expect("swap");
    println!(
        "Q1 swap after launch: bought {} coin units for 1 SOL, CU={}",
        e.amount(&ua),
        r.cu
    );
    // the revoked authority really is gone: a MintTo signed by nobody/anyone fails
    let mut d = vec![7u8];
    d.extend_from_slice(&1u64.to_le_bytes());
    let mint_again = Instruction::new_with_bytes(
        SPL_TOKEN,
        &d,
        vec![
            AccountMeta::new(e.mint, false),
            AccountMeta::new(ua, false),
            AccountMeta::new_readonly(trader.pubkey(), true),
        ],
    );
    println!(
        "Q1 MintTo after revoke (trader signs): {:?}",
        send(&mut e.svm, &[mint_again], &[&trader]).map(|s| s.cu)
    );
}

#[test]
fn q4_sizes() {
    for s in &SIZES {
        run_launch(s, false);
    }
}

// ------------------------------------------------------------------------------------------------ Q2

fn prefund(e: &mut Env, target: Pubkey, lamports: u64) -> String {
    let attacker = Keypair::new();
    e.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let ix = system_instruction::transfer(&attacker.pubkey(), &target, lamports);
    match send(&mut e.svm, &[ix], &[&attacker]) {
        Ok(_) => "real transfer OK".into(),
        Err(err) => {
            e.svm
                .set_account(
                    target,
                    Account {
                        lamports,
                        data: vec![],
                        owner: system_program::ID,
                        executable: false,
                        rent_epoch: 0,
                    },
                )
                .unwrap();
            format!("real transfer REJECTED ({err}); forced with set_account")
        }
    }
}

#[test]
fn q2_prefunded_addresses() {
    let s = &SIZES[0];
    let reference = run_launch(s, false);
    let targets = [
        ("mint PDA (ours)", reference.mint, true),
        (
            "seeder_a coin token account PDA (ours)",
            reference.seeder_a,
            true,
        ),
        (
            "seeder_b wSOL token account PDA (ours)",
            reference.seeder_b,
            true,
        ),
        ("pool PDA", reference.pool, false),
        ("token_a_vault PDA", reference.vault_a, false),
        ("token_b_vault PDA", reference.vault_b, false),
        ("position PDA", reference.position, false),
        ("position NFT mint PDA", reference.nft_mint, false),
        ("position NFT token account", reference.nft_account, false),
    ];
    for (name, addr, ours) in targets {
        let final_len = reference.svm.get_account(&addr).unwrap().data.len();
        let rent_full = reference.svm.minimum_balance_for_rent_exemption(final_len);
        let rent_zero = reference.svm.minimum_balance_for_rent_exemption(0);
        for amount in [1u64, rent_zero, rent_full] {
            let modes: &[u8] = if ours {
                &[
                    amm_spike::CREATE_NAIVE,
                    amm_spike::CREATE_FUND_ALLOCATE_ASSIGN,
                    amm_spike::CREATE_ALLOW_PREFUND,
                ]
            } else {
                &[amm_spike::CREATE_FUND_ALLOCATE_ASSIGN]
            };
            for &mode in modes {
                let mut e = Env::new(s.pool_sol + 1_000_000_000);
                let how = prefund(&mut e, addr, amount);
                let ix = e.launch_ix(s, mode);
                let fp = e.fee_payer.insecure_clone();
                let res = send(&mut e.svm, &[ix], &[&fp]);
                let mode_name = [
                    "naive create_account",
                    "fund+allocate+assign",
                    "CreateAccountAllowPrefund",
                ][mode as usize];
                match res {
                    Ok(r) => {
                        let acc = e.svm.get_account(&addr).unwrap();
                        println!(
                            "Q2 {name} prefund={amount} [{how}] mode={mode_name}: LAUNCH OK CU={} | account now len={} lamports={} (final len {final_len}, rent {rent_full}) | position {}",
                            r.cu,
                            acc.data.len(),
                            acc.lamports,
                            e.pos(&e.position)
                        );
                    }
                    Err(err) => println!(
                        "Q2 {name} prefund={amount} [{how}] mode={mode_name}: LAUNCH FAILED {err}"
                    ),
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ Q3

fn owner_call(e: &mut Env, amm_ix: Instruction, extra: &[&Keypair]) -> Result<Sent, String> {
    let mut metas = amm_spike::accounts::OwnerCpi {
        owner: e.owner,
        amm_program: damm(),
    }
    .to_account_metas(None);
    for m in amm_ix.accounts {
        let mut m = m;
        if m.pubkey == e.owner {
            m.is_signer = false; // the program signs for the PDA inside the CPI
        }
        metas.push(m);
    }
    let ix = Instruction::new_with_bytes(
        e.program_id,
        &amm_spike::instruction::OwnerCpi { data: amm_ix.data }.data(),
        metas,
    );
    let fp = e.fee_payer.insecure_clone();
    let mut signers = vec![&fp];
    signers.extend_from_slice(extra);
    send(&mut e.svm, &[ix], &signers)
}

fn report(label: &str, r: &Result<Sent, String>, e: &Env, positions: &[(&str, Pubkey)]) {
    let status = match r {
        Ok(s) => format!("SUCCEEDED CU={}", s.cu),
        Err(err) => format!("FAILED {err}"),
    };
    let states: Vec<String> = positions
        .iter()
        .map(|(n, k)| format!("{n}: {}", e.pos(k)))
        .collect();
    println!(
        "Q3 {label}: {status}\n      -> {} | {}",
        states.join(" || "),
        e.pool_liquidity()
    );
}

#[test]
fn q3_after_the_lock() {
    let s = &SIZES[0];
    let mut e = run_launch(s, false);
    let recv_a = Pubkey::new_unique();
    let recv_b = Pubkey::new_unique();
    e.svm
        .set_account(recv_a, token_account(&e.mint, &e.owner, 0, false))
        .unwrap();
    e.svm
        .set_account(recv_b, token_account(&NATIVE_MINT, &e.owner, 0, true))
        .unwrap();
    let first = [("first", e.position)];
    println!(
        "Q3 start: first: {} | {}",
        e.pos(&e.position),
        e.pool_liquidity()
    );

    let remove = |e: &Env, position: Pubkey, nft_account: Pubkey, signer: Pubkey, delta: u128| {
        Instruction::new_with_bytes(
            damm(),
            &cp_amm::client::args::RemoveLiquidity {
                params: cp_amm::types::RemoveLiquidityParameters {
                    liquidity_delta: delta,
                    token_a_amount_threshold: 0,
                    token_b_amount_threshold: 0,
                },
            }
            .data(),
            cp_amm::client::accounts::RemoveLiquidity {
                pool_authority: e.pool_authority,
                pool: e.pool,
                position,
                token_a_account: recv_a,
                token_b_account: recv_b,
                token_a_vault: e.vault_a,
                token_b_vault: e.vault_b,
                token_a_mint: e.mint,
                token_b_mint: NATIVE_MINT,
                position_nft_account: nft_account,
                signer,
                token_a_program: SPL_TOKEN,
                token_b_program: SPL_TOKEN,
                event_authority: e.event_authority,
                program: damm(),
            }
            .to_account_metas(None),
        )
    };
    let remove_all = |e: &Env, position: Pubkey, nft_account: Pubkey, signer: Pubkey| {
        Instruction::new_with_bytes(
            damm(),
            &cp_amm::client::args::RemoveAllLiquidity {
                token_a_amount_threshold: 0,
                token_b_amount_threshold: 0,
            }
            .data(),
            cp_amm::client::accounts::RemoveAllLiquidity {
                pool_authority: e.pool_authority,
                pool: e.pool,
                position,
                token_a_account: recv_a,
                token_b_account: recv_b,
                token_a_vault: e.vault_a,
                token_b_vault: e.vault_b,
                token_a_mint: e.mint,
                token_b_mint: NATIVE_MINT,
                position_nft_account: nft_account,
                signer,
                token_a_program: SPL_TOKEN,
                token_b_program: SPL_TOKEN,
                event_authority: e.event_authority,
                program: damm(),
            }
            .to_account_metas(None),
        )
    };
    let close =
        |e: &Env, nft_mint: Pubkey, nft_account: Pubkey, position: Pubkey, owner: Pubkey| {
            Instruction::new_with_bytes(
                damm(),
                &cp_amm::client::args::ClosePosition {}.data(),
                cp_amm::client::accounts::ClosePosition {
                    position_nft_mint: nft_mint,
                    position_nft_account: nft_account,
                    pool: e.pool,
                    position,
                    pool_authority: e.pool_authority,
                    rent_receiver: e.fee_payer.pubkey(),
                    owner,
                    token_program: TOKEN_2022,
                    event_authority: e.event_authority,
                    program: damm(),
                }
                .to_account_metas(None),
            )
        };

    let (pos, nft, own) = (e.position, e.nft_account, e.owner);
    let ix = remove(&e, pos, nft, own, 1);
    let r = owner_call(&mut e, ix, &[]);
    report("remove_liquidity(delta=1)", &r, &e, &first);
    let ix = remove(&e, pos, nft, own, s.liquidity);
    let r = owner_call(&mut e, ix, &[]);
    report("remove_liquidity(delta=permanent_locked)", &r, &e, &first);
    let ix = remove_all(&e, pos, nft, own);
    let r = owner_call(&mut e, ix, &[]);
    report(
        &format!(
            "remove_all_liquidity(0,0) [owner receivers after: a={} b={}]",
            e.amount(&recv_a),
            e.amount(&recv_b)
        ),
        &r,
        &e,
        &first,
    );
    let nm = e.nft_mint;
    let ix = close(&e, nm, nft, pos, own);
    let r = owner_call(&mut e, ix, &[]);
    report("close_position", &r, &e, &first);

    let vesting = Keypair::new();
    let vp = cp_amm::types::VestingParameters {
        cliff_point: None,
        period_frequency: 1,
        cliff_unlock_liquidity: 0,
        liquidity_per_period: 1,
        number_of_period: 1,
    };
    let lock_ix = Instruction::new_with_bytes(
        damm(),
        &cp_amm::client::args::LockPosition { params: vp.clone() }.data(),
        cp_amm::client::accounts::LockPosition {
            pool: e.pool,
            position: pos,
            vesting: vesting.pubkey(),
            position_nft_account: nft,
            signer: own,
            payer: e.fee_payer.pubkey(),
            system_program: system_program::ID,
            event_authority: e.event_authority,
            program: damm(),
        }
        .to_account_metas(None),
    );
    let r = owner_call(&mut e, lock_ix, &[&vesting]);
    report("lock_position(vesting 1 unit)", &r, &e, &first);
    let inner_ix = Instruction::new_with_bytes(
        damm(),
        &cp_amm::client::args::LockInnerPosition { params: vp }.data(),
        cp_amm::client::accounts::LockInnerPosition {
            pool: e.pool,
            position: pos,
            position_nft_account: nft,
            signer: own,
            event_authority: e.event_authority,
            program: damm(),
        }
        .to_account_metas(None),
    );
    let r = owner_call(&mut e, inner_ix, &[]);
    report("lock_inner_position(vesting 1 unit)", &r, &e, &first);

    // second position held by an OUTSIDER keypair (anyone can create an empty position on any pool)
    let outsider = Keypair::new();
    e.svm.airdrop(&outsider.pubkey(), 1_000_000_000).unwrap();
    let nft2 = Keypair::new();
    let pos2 = pda(&[b"position", nft2.pubkey().as_ref()], &damm());
    let nft2_acc = pda(&[b"position_nft_account", nft2.pubkey().as_ref()], &damm());
    let create2 = Instruction::new_with_bytes(
        damm(),
        &cp_amm::client::args::CreatePosition {}.data(),
        cp_amm::client::accounts::CreatePosition {
            owner: outsider.pubkey(),
            position_nft_mint: nft2.pubkey(),
            position_nft_account: nft2_acc,
            pool: e.pool,
            position: pos2,
            pool_authority: e.pool_authority,
            payer: outsider.pubkey(),
            token_program: TOKEN_2022,
            system_program: system_program::ID,
            event_authority: e.event_authority,
            program: damm(),
        }
        .to_account_metas(None),
    );
    let r = send(&mut e.svm, &[create2], &[&outsider, &nft2]);
    let both = [("first", pos), ("second(outsider)", pos2)];
    report("outsider create_position (direct, no CPI)", &r, &e, &both);

    let split2 = Instruction::new_with_bytes(
        damm(),
        &cp_amm::client::args::SplitPosition2 {
            numerator: 500_000_000,
        }
        .data(),
        cp_amm::client::accounts::SplitPosition2 {
            pool: e.pool,
            first_position: pos,
            first_position_nft_account: nft,
            second_position: pos2,
            second_position_nft_account: nft2_acc,
            first_owner: own,
            second_owner: outsider.pubkey(),
            event_authority: e.event_authority,
            program: damm(),
        }
        .to_account_metas(None),
    );
    let r = owner_call(&mut e, split2, &[&outsider]);
    report(
        "split_position2(numerator=500_000_000 of 1e9) first->outsider",
        &r,
        &e,
        &both,
    );

    let split1 = Instruction::new_with_bytes(
        damm(),
        &cp_amm::client::args::SplitPosition {
            params: cp_amm::types::SplitPositionParameters {
                unlocked_liquidity_percentage: 0,
                permanent_locked_liquidity_percentage: 50,
                fee_a_percentage: 0,
                fee_b_percentage: 0,
                reward_0_percentage: 0,
                reward_1_percentage: 0,
                inner_vesting_liquidity_percentage: 0,
                padding: [0; 15],
            },
        }
        .data(),
        cp_amm::client::accounts::SplitPosition {
            pool: e.pool,
            first_position: pos,
            first_position_nft_account: nft,
            second_position: pos2,
            second_position_nft_account: nft2_acc,
            first_owner: own,
            second_owner: outsider.pubkey(),
            event_authority: e.event_authority,
            program: damm(),
        }
        .to_account_metas(None),
    );
    let r = owner_call(&mut e, split1, &[&outsider]);
    report(
        "split_position(permanent_locked_liquidity_percentage=50) first->outsider",
        &r,
        &e,
        &both,
    );

    // the outsider now holds part of the permanently locked liquidity: can they withdraw it directly?
    let ix = remove(&e, pos2, nft2_acc, outsider.pubkey(), 1);
    let r = send(&mut e.svm, &[ix], &[&outsider]);
    report(
        "outsider remove_liquidity(delta=1) on second position (direct)",
        &r,
        &e,
        &both,
    );
    let ix = remove_all(&e, pos2, nft2_acc, outsider.pubkey());
    let r = send(&mut e.svm, &[ix], &[&outsider]);
    report(
        "outsider remove_all_liquidity on second position (direct)",
        &r,
        &e,
        &both,
    );
    let ix = close(&e, nft2.pubkey(), nft2_acc, pos2, outsider.pubkey());
    let r = send(&mut e.svm, &[ix], &[&outsider]);
    report(
        "outsider close_position on second position (direct)",
        &r,
        &e,
        &both,
    );
    println!(
        "Q3 final vaults: a={} b={} (deposit was a={} b<= {})",
        e.amount(&e.vault_a),
        e.amount(&e.vault_b),
        POOL_TOKENS,
        s.pool_sol
    );
}

// ------------------------------------------------------------------------------------------------ extras

/// Q2 edge: an attacker sends MORE than rent to the native-mint vault / our wSOL account before the launch.
#[test]
fn q2_overfunded_native_accounts() {
    let s = &SIZES[0];
    for (name, pick) in [
        ("token_b_vault (native) PDA", 0u8),
        ("seeder_b wSOL PDA (ours)", 1u8),
    ] {
        let mut e = Env::new(s.pool_sol + 1_000_000_000);
        let target = if pick == 0 { e.vault_b } else { e.seeder_b };
        let how = prefund(&mut e, target, 2_039_280 + 1_000_000_000);
        let ix = e.launch_ix(s, amm_spike::CREATE_FUND_ALLOCATE_ASSIGN);
        let fp = e.fee_payer.insecure_clone();
        match send(&mut e.svm, &[ix], &[&fp]) {
            Ok(r) => {
                let pool = cp_amm::accounts::Pool::try_deserialize_unchecked(
                    &mut &e.svm.get_account(&e.pool).unwrap().data[..],
                )
                .unwrap();
                println!(
                    "Q2X {name} prefund=rent+1 SOL [{how}]: LAUNCH OK CU={} | vault_b amount={} pool.token_b_amount={} seeder_b amount={} | position {}",
                    r.cu,
                    e.amount(&e.vault_b),
                    pool.token_b_amount,
                    e.amount(&e.seeder_b),
                    e.pos(&e.position)
                );
            }
            Err(err) => println!("Q2X {name} prefund=rent+1 SOL [{how}]: LAUNCH FAILED {err}"),
        }
    }
}

/// Q4 with the ORIGINAL `seed` instruction (pre-made mint and deposit accounts), for a seed-only CU figure.
#[test]
fn q4_original_seed_instruction_sizes() {
    for s in &SIZES {
        let start = 1_000_000_000u64;
        let mut e = Env::new(start);
        let mut mdata = vec![0u8; 82];
        mdata[36..44].copy_from_slice(&amm_spike::COIN_SUPPLY.to_le_bytes());
        mdata[44] = 6;
        mdata[45] = 1;
        e.svm
            .set_account(
                e.mint,
                Account {
                    lamports: 1_461_600,
                    data: mdata,
                    owner: SPL_TOKEN,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .unwrap();
        e.svm
            .set_account(
                e.seeder_a,
                token_account(&e.mint, &e.seeder, POOL_TOKENS, false),
            )
            .unwrap();
        e.svm
            .set_account(
                e.seeder_b,
                token_account(&NATIVE_MINT, &e.seeder, s.pool_sol, true),
            )
            .unwrap();
        let ix = Instruction::new_with_bytes(
            e.program_id,
            &amm_spike::instruction::Seed {
                liquidity: s.liquidity,
                sqrt_price: s.sqrt_price,
            }
            .data(),
            amm_spike::accounts::Seed {
                seeder: e.seeder,
                owner: e.owner,
                position_nft_mint: e.nft_mint,
                position_nft_account: e.nft_account,
                config: CONFIG_11,
                pool_authority: e.pool_authority,
                pool: e.pool,
                position: e.position,
                token_a_mint: e.mint,
                token_b_mint: NATIVE_MINT,
                token_a_vault: e.vault_a,
                token_b_vault: e.vault_b,
                payer_token_a: e.seeder_a,
                payer_token_b: e.seeder_b,
                token_program: SPL_TOKEN,
                token_2022_program: TOKEN_2022,
                system_program: system_program::ID,
                event_authority: e.event_authority,
                amm_program: damm(),
            }
            .to_account_metas(None),
        );
        let fp = e.fee_payer.insecure_clone();
        let r = send(&mut e.svm, &[ix], &[&fp]).unwrap_or_else(|err| panic!("seed failed: {err}"));
        let (dep_a, dep_b) = (e.amount(&e.vault_a), e.amount(&e.vault_b));
        println!(
            "Q4S seed-only {}: CU={} unique_accounts={} legacy_message_bytes={} | token A {} (dust {}), wSOL {} (dust {} lamports) | rent {}",
            s.name,
            r.cu,
            r.unique_accounts,
            r.message_bytes,
            dep_a,
            POOL_TOKENS - dep_a,
            dep_b,
            s.pool_sol - dep_b,
            start - e.lamports(&e.seeder)
        );
    }
}
