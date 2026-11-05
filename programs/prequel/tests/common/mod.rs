//! LiteSVM harness shared by the Phase 1 tests (docs/PHASE1_DESIGN.md section 6).
//! Needs `anchor build` first: the program binary is read from target/deploy/prequel.so.
#![allow(dead_code)]

pub mod ix;

use {
    anchor_lang::{
        prelude::{pubkey, AccountDeserialize, Clock, Pubkey},
        solana_program::instruction::Instruction,
    },
    litesvm::LiteSVM,
    prequel::constants::*,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

pub const PROGRAM_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_TARGET_TMPDIR"),
    "/../deploy/prequel.so"
));
pub const BPF_LOADER_UPGRADEABLE: Pubkey = pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");
pub const ED25519_PROGRAM: Pubkey = pubkey!("Ed25519SigVerify111111111111111111111111111");
pub const INSTRUCTIONS_SYSVAR: Pubkey = pubkey!("Sysvar1nstructions1111111111111111111111111");
/// Start of every test: a plausible Unix time, so "T-1, T, T+1" boundaries are easy to read.
pub const T0: i64 = 1_800_000_000;
pub const SOL: u64 = 1_000_000_000;

pub fn program_id() -> Pubkey {
    prequel::id()
}

// ---- addresses -------------------------------------------------------------------------------------------------

pub fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &program_id()).0
}
pub fn config_pda() -> Pubkey {
    pda(&[CONFIG_SEED])
}
pub fn event_pda(id: u64) -> Pubkey {
    pda(&[EVENT_SEED, &id.to_le_bytes()])
}
pub fn external_pda(external_id_hash: &[u8; 32]) -> Pubkey {
    pda(&[EXTERNAL_SEED, external_id_hash])
}
pub fn candidate_pda(event: &Pubkey, outcome: u8, canon_hash: &[u8; 32]) -> Pubkey {
    pda(&[CANDIDATE_SEED, event.as_ref(), &[outcome], canon_hash])
}
pub fn vault_pda(candidate: &Pubkey) -> Pubkey {
    pda(&[VAULT_SEED, candidate.as_ref()])
}
pub fn commitment_pda(candidate: &Pubkey, wallet: &Pubkey) -> Pubkey {
    pda(&[COMMITMENT_SEED, candidate.as_ref(), wallet.as_ref()])
}
pub fn coproposer_pda(candidate: &Pubkey, wallet: &Pubkey) -> Pubkey {
    pda(&[COPROPOSER_SEED, candidate.as_ref(), wallet.as_ref()])
}
pub fn program_data_address() -> Pubkey {
    Pubkey::find_program_address(&[program_id().as_ref()], &BPF_LOADER_UPGRADEABLE).0
}

/// sha256 for fixtures that need an external-id hash or a name/ticker/URI digest.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hash(bytes).to_bytes()
}

// ---- results -----------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct Sent {
    pub logs: Vec<String>,
    pub compute_units: u64,
}

#[derive(Debug)]
pub struct Failed {
    pub debug: String,
    pub logs: Vec<String>,
}

impl Failed {
    /// The program's custom error number (6000 + variant index), if the failure was one.
    pub fn code(&self) -> Option<u32> {
        let rest = self.debug.split("Custom(").nth(1)?;
        rest.split(')').next()?.parse().ok()
    }
    pub fn is(&self, e: prequel::error::PrequelError) -> bool {
        self.code() == Some(e.into())
    }
}

/// Asserts a failure is the named program error, printing logs when it is not.
#[macro_export]
macro_rules! assert_err {
    ($result:expr, $err:expr) => {{
        match $result {
            Ok(_) => panic!("expected {:?} but the transaction succeeded", $err),
            Err(f) => assert!(
                f.is($err),
                "expected {:?} ({}), got {} | logs: {:#?}",
                $err,
                u32::from($err),
                f.debug,
                f.logs
            ),
        }
    }};
}

// ---- environment -------------------------------------------------------------------------------------------------

pub struct Env {
    pub svm: LiteSVM,
    pub deployer: Keypair,
    pub admin: Keypair,
    pub resolver: Keypair,
    pub canonicalizer: Keypair,
    pub treasury: Pubkey,
}

impl Env {
    /// Program deployed with `deployer` as upgrade authority, clock at `T0`, four funded role keys.
    pub fn new() -> Env {
        let mut svm = LiteSVM::new().with_precompiles();
        svm.set_sysvar(&Clock {
            slot: 1_000,
            epoch_start_timestamp: T0,
            epoch: 1,
            leader_schedule_epoch: 1,
            unix_timestamp: T0,
        });
        svm.add_program(program_id(), PROGRAM_BYTES).unwrap();
        let deployer = Keypair::new();
        set_upgrade_authority(&mut svm, &deployer.pubkey());
        let mut env = Env {
            svm,
            deployer,
            admin: Keypair::new(),
            resolver: Keypair::new(),
            canonicalizer: Keypair::new(),
            treasury: Pubkey::new_unique(),
        };
        for k in [&env.deployer, &env.admin, &env.resolver, &env.canonicalizer] {
            env.svm.airdrop(&k.pubkey(), 100 * SOL).unwrap();
        }
        env
    }

    pub fn wallet(&mut self, sol: u64) -> Keypair {
        let k = Keypair::new();
        self.svm.airdrop(&k.pubkey(), sol * SOL).unwrap();
        k
    }

    pub fn now(&self) -> i64 {
        self.svm.get_sysvar::<Clock>().unix_timestamp
    }

    pub fn warp_to(&mut self, unix_timestamp: i64) {
        let mut clock = self.svm.get_sysvar::<Clock>();
        clock.slot += 1;
        clock.unix_timestamp = unix_timestamp;
        self.svm.set_sysvar(&clock);
    }

    /// Sends one transaction; `signers[0]` pays. A fresh blockhash each time keeps identical retries distinct.
    pub fn send(&mut self, ixs: &[Instruction], signers: &[&Keypair]) -> Result<Sent, Failed> {
        self.svm.expire_blockhash();
        let msg = Message::new_with_blockhash(
            ixs,
            Some(&signers[0].pubkey()),
            &self.svm.latest_blockhash(),
        );
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
        match self.svm.send_transaction(tx) {
            Ok(meta) => Ok(Sent {
                logs: meta.logs,
                compute_units: meta.compute_units_consumed,
            }),
            Err(e) => Err(Failed {
                debug: format!("{:?}", e.err),
                logs: e.meta.logs,
            }),
        }
    }

    pub fn lamports(&self, key: &Pubkey) -> u64 {
        self.svm.get_account(key).map_or(0, |a| a.lamports)
    }

    pub fn exists(&self, key: &Pubkey) -> bool {
        self.svm.get_account(key).is_some_and(|a| a.lamports > 0)
    }

    pub fn load<T: AccountDeserialize>(&self, key: &Pubkey) -> T {
        let acc = self
            .svm
            .get_account(key)
            .unwrap_or_else(|| panic!("missing account {key}"));
        T::try_deserialize(&mut &acc.data[..])
            .unwrap_or_else(|e| panic!("cannot decode {key}: {e}"))
    }

    pub fn rent_minimum(&self, data_len: usize) -> u64 {
        self.svm.minimum_balance_for_rent_exemption(data_len)
    }
}

/// LiteSVM deploys through the upgradeable loader with no upgrade authority; give it one so that
/// `initialize_config`'s deployer check can be exercised. ProgramData = tag 3 (u32) | slot (u64) | Some(authority).
pub fn set_upgrade_authority(svm: &mut LiteSVM, authority: &Pubkey) {
    let address = program_data_address();
    let mut acc: Account = svm.get_account(&address).expect("program data account");
    acc.data[0..4].copy_from_slice(&3u32.to_le_bytes());
    acc.data[12] = 1;
    acc.data[13..45].copy_from_slice(authority.as_ref());
    svm.set_account(address, acc).unwrap();
}

// ---- Ed25519 precompile instruction (attestation tests) --------------------------------------------------------

/// A single-signature Ed25519 precompile instruction whose three offset-instruction-index fields are all
/// `u16::MAX` ("this instruction"), the only shape the program accepts (D-014).
pub fn ed25519_ix(signer: &Keypair, message: &[u8]) -> Instruction {
    let sig = signer.sign_message(message);
    ed25519_ix_raw(
        signer.pubkey().as_ref(),
        sig.as_ref(),
        message,
        [u16::MAX; 3],
    )
}

/// Same, with explicit pubkey/signature bytes and `[signature_ix, pubkey_ix, message_ix]` index fields, so tests
/// can build malformed variants.
pub fn ed25519_ix_raw(
    pubkey: &[u8],
    signature: &[u8],
    message: &[u8],
    indexes: [u16; 3],
) -> Instruction {
    assert_eq!(pubkey.len(), 32);
    assert_eq!(signature.len(), 64);
    let (pk_off, sig_off, msg_off) = (16u16, 48u16, 112u16);
    let mut data = vec![1u8, 0u8];
    for v in [
        sig_off,
        indexes[0],
        pk_off,
        indexes[1],
        msg_off,
        message.len() as u16,
        indexes[2],
    ] {
        data.extend_from_slice(&v.to_le_bytes());
    }
    data.extend_from_slice(pubkey);
    data.extend_from_slice(signature);
    data.extend_from_slice(message);
    Instruction {
        program_id: ED25519_PROGRAM,
        accounts: vec![],
        data,
    }
}

// ---- packet size, compute budget, events -------------------------------------------------------------------------

pub const COMPUTE_BUDGET_PROGRAM: Pubkey = pubkey!("ComputeBudget111111111111111111111111111111");

/// The two instructions wallets and apps usually prepend: a compute-unit limit and a priority fee.
pub fn compute_budget_ixs() -> Vec<Instruction> {
    let limit = Instruction {
        program_id: COMPUTE_BUDGET_PROGRAM,
        accounts: vec![],
        data: [&[2u8][..], &200_000u32.to_le_bytes()].concat(),
    };
    let price = Instruction {
        program_id: COMPUTE_BUDGET_PROGRAM,
        accounts: vec![],
        data: [&[3u8][..], &1_000u64.to_le_bytes()].concat(),
    };
    vec![limit, price]
}

/// Serialized size of a legacy transaction: signature count byte, 64 bytes per signature, and the message.
pub fn tx_size(env: &Env, ixs: &[Instruction], payer: &Pubkey, signatures: usize) -> usize {
    let msg = Message::new_with_blockhash(ixs, Some(payer), &env.svm.latest_blockhash());
    1 + 64 * signatures + VersionedMessage::Legacy(msg).serialize().len()
}

/// Decodes every `T` event the program logged in a transaction.
pub fn events_of<T: anchor_lang::Event>(logs: &[String]) -> Vec<T> {
    use base64::Engine;
    logs.iter()
        .filter_map(|l| l.strip_prefix("Program data: "))
        .filter_map(|b64| {
            base64::engine::general_purpose::STANDARD
                .decode(b64.trim())
                .ok()
        })
        .filter(|bytes| bytes.len() >= 8 && bytes[..8] == *T::DISCRIMINATOR)
        .map(|bytes| T::deserialize(&mut &bytes[8..]).expect("event decodes"))
        .collect()
}

/// Number of `Program data:` lines, that is, events of any type.
pub fn event_count(logs: &[String]) -> usize {
    logs.iter()
        .filter(|l| l.starts_with("Program data: "))
        .count()
}

/// Sends lamports to an address that a later `init` will create (an attacker pre-funding a PDA).
pub fn prefund(env: &mut Env, from: &Keypair, to: &Pubkey, lamports: u64) {
    let ix =
        anchor_lang::solana_program::system_instruction::transfer(&from.pubkey(), to, lamports);
    env.send(&[ix], &[from]).expect("pre-funding transfer");
}
