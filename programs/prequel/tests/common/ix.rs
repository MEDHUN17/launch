//! Instruction and account builders over the Anchor-generated client types.
#![allow(dead_code)]

use super::*;
use anchor_lang::{system_program, InstructionData, ToAccountMetas};
use prequel::{CreateEventArgs, InitializeConfigArgs, Params, UpdateConfigArgs};

fn build(data: impl InstructionData, accounts: impl ToAccountMetas) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: accounts.to_account_metas(None),
        data: data.data(),
    }
}

pub fn initialize_config(deployer: &Pubkey, args: InitializeConfigArgs) -> Instruction {
    build(
        prequel::instruction::InitializeConfig { args },
        prequel::accounts::InitializeConfig {
            deployer: *deployer,
            config: config_pda(),
            program: program_id(),
            program_data: program_data_address(),
            system_program: system_program::ID,
        },
    )
}

pub fn update_config(admin: &Pubkey, args: UpdateConfigArgs) -> Instruction {
    build(
        prequel::instruction::UpdateConfig { args },
        prequel::accounts::UpdateConfig {
            admin: *admin,
            config: config_pda(),
        },
    )
}

pub fn set_pause(
    admin: &Pubkey,
    pause_proposals: Option<bool>,
    pause_commits: Option<bool>,
) -> Instruction {
    build(
        prequel::instruction::SetPause {
            pause_proposals,
            pause_commits,
        },
        prequel::accounts::SetPause {
            admin: *admin,
            config: config_pda(),
        },
    )
}

pub fn create_event(admin: &Pubkey, event_id: u64, args: CreateEventArgs) -> Instruction {
    build(
        prequel::instruction::CreateEvent { args: args.clone() },
        prequel::accounts::CreateEvent {
            admin: *admin,
            config: config_pda(),
            event: event_pda(event_id),
            external_listing: external_pda(&args.external_id_hash),
            system_program: system_program::ID,
        },
    )
}

pub fn default_init_args(env: &Env) -> InitializeConfigArgs {
    InitializeConfigArgs {
        admin: env.admin.pubkey(),
        resolver: env.resolver.pubkey(),
        canonicalizer: env.canonicalizer.pubkey(),
        treasury: env.treasury,
        params: Params::spec_defaults(),
    }
}

/// Default event: window open now, closes in 1 hour, starts in 2 hours, resolves 3 hours after the start.
pub fn default_event_args(now: i64, external_id: &str) -> CreateEventArgs {
    CreateEventArgs {
        external_id: external_id.to_string(),
        external_id_hash: sha256(external_id.as_bytes()),
        source: prequel::constants::SOURCE_KALSHI,
        outcome_count: 2,
        window_open: now,
        window_close: now + 3_600,
        event_start: now + 7_200,
        expected_resolution: now + 7_200 + 10_800,
    }
}

impl Env {
    pub fn init_config(&mut self) {
        let ix = initialize_config(&self.deployer.pubkey(), default_init_args(self));
        self.send(&[ix], &[&self.deployer.insecure_clone()])
            .expect("initialize_config");
    }

    /// Creates an event with default times and returns (event id, event address).
    pub fn create_default_event(&mut self, external_id: &str) -> (u64, Pubkey) {
        let id = self.load::<prequel::Config>(&config_pda()).next_event_id;
        let ix = create_event(
            &self.admin.pubkey(),
            id,
            default_event_args(self.now(), external_id),
        );
        self.send(&[ix], &[&self.admin.insecure_clone()])
            .expect("create_event");
        (id, event_pda(id))
    }
}

// ---- propose / commit / co_propose ---------------------------------------------------------------------------------

use prequel::{
    attestation::{content_hash, signed_digest, Attested},
    canon, ProposeArgs,
};

pub fn propose_args(ticker: &str, outcome: u8, amount: u64) -> ProposeArgs {
    ProposeArgs {
        outcome,
        canon_hash: canon::canonical_ticker_hash(ticker)
            .unwrap_or_else(|_| sha256(ticker.as_bytes())),
        name: format!("{ticker} Coin"),
        ticker: ticker.to_string(),
        uri: format!("ipfs://bafy{}", ticker.to_lowercase()),
        image_hash: sha256(format!("image:{ticker}").as_bytes()),
        amount,
        attestation_expiry: T0 + 600,
    }
}

pub fn candidate_for(event_id: u64, args: &ProposeArgs) -> Pubkey {
    candidate_pda(&event_pda(event_id), args.outcome, &args.canon_hash)
}

pub fn propose(
    proposer: &Pubkey,
    event_id: u64,
    treasury: &Pubkey,
    args: &ProposeArgs,
) -> Instruction {
    let candidate = candidate_for(event_id, args);
    build(
        prequel::instruction::ProposeCandidate { args: args.clone() },
        prequel::accounts::ProposeCandidate {
            proposer: *proposer,
            config: config_pda(),
            event: event_pda(event_id),
            candidate,
            vault: vault_pda(&candidate),
            commitment: commitment_pda(&candidate, proposer),
            treasury: *treasury,
            instructions: INSTRUCTIONS_SYSVAR,
            system_program: system_program::ID,
        },
    )
}

pub fn commit(wallet: &Pubkey, event_id: u64, candidate: &Pubkey, amount: u64) -> Instruction {
    build(
        prequel::instruction::Commit { amount },
        prequel::accounts::Commit {
            wallet: *wallet,
            config: config_pda(),
            event: event_pda(event_id),
            candidate: *candidate,
            vault: vault_pda(candidate),
            commitment: commitment_pda(candidate, wallet),
            system_program: system_program::ID,
        },
    )
}

pub fn co_propose(
    wallet: &Pubkey,
    event_id: u64,
    candidate: &Pubkey,
    treasury: &Pubkey,
    amount: u64,
) -> Instruction {
    build(
        prequel::instruction::CoPropose { amount },
        prequel::accounts::CoPropose {
            wallet: *wallet,
            config: config_pda(),
            event: event_pda(event_id),
            candidate: *candidate,
            vault: vault_pda(candidate),
            commitment: commitment_pda(candidate, wallet),
            co_proposer_share: coproposer_pda(candidate, wallet),
            treasury: *treasury,
            system_program: system_program::ID,
        },
    )
}

/// The bytes the canonicalizer signs for `args` (decision D-014), and the Ed25519 instruction carrying them.
pub fn attestation_message(event_id: u64, args: &ProposeArgs, proposer: &Pubkey) -> Vec<u8> {
    let display = args.ticker.to_uppercase();
    signed_digest(&Attested {
        program_id: &program_id(),
        config: &config_pda(),
        event: &event_pda(event_id),
        outcome: args.outcome,
        canon_hash: &args.canon_hash,
        content_hash: &content_hash(&args.name, &display, &args.uri),
        image_hash: &args.image_hash,
        proposer,
        expiry: args.attestation_expiry,
    })
    .to_vec()
}

pub fn attestation_ix(
    signer: &solana_keypair::Keypair,
    event_id: u64,
    args: &ProposeArgs,
    proposer: &Pubkey,
) -> Instruction {
    ed25519_ix(signer, &attestation_message(event_id, args, proposer))
}

impl Env {
    /// Initializes the config with the SPEC defaults after `edit`, attestation off unless the edit turns it on.
    pub fn init_config_with(&mut self, edit: impl FnOnce(&mut Params)) {
        let mut args = default_init_args(self);
        args.params.require_attestation = false;
        edit(&mut args.params);
        let ix = initialize_config(&self.deployer.pubkey(), args);
        self.send(&[ix], &[&self.deployer.insecure_clone()])
            .expect("initialize_config");
    }

    pub fn do_propose(
        &mut self,
        proposer: &solana_keypair::Keypair,
        event_id: u64,
        args: &ProposeArgs,
    ) -> Result<Pubkey, Failed> {
        let ix = propose(&proposer.pubkey(), event_id, &self.treasury, args);
        self.send(&[ix], &[proposer])
            .map(|_| candidate_for(event_id, args))
    }

    pub fn do_commit(
        &mut self,
        wallet: &solana_keypair::Keypair,
        event_id: u64,
        candidate: &Pubkey,
        amount: u64,
    ) -> Result<Sent, Failed> {
        let ix = commit(&wallet.pubkey(), event_id, candidate, amount);
        self.send(&[ix], &[wallet])
    }

    pub fn do_co_propose(
        &mut self,
        wallet: &solana_keypair::Keypair,
        event_id: u64,
        candidate: &Pubkey,
        amount: u64,
    ) -> Result<Sent, Failed> {
        let ix = co_propose(
            &wallet.pubkey(),
            event_id,
            candidate,
            &self.treasury,
            amount,
        );
        self.send(&[ix], &[wallet])
    }
}

// ---- close / void / reject / refund --------------------------------------------------------------------------------

pub fn close_window(caller: &Pubkey, event_id: u64) -> Instruction {
    build(
        prequel::instruction::CloseWindow {},
        prequel::accounts::CloseWindow {
            caller: *caller,
            event: event_pda(event_id),
        },
    )
}

pub fn void_event(admin: &Pubkey, event_id: u64) -> Instruction {
    build(
        prequel::instruction::VoidEvent {},
        prequel::accounts::VoidEvent {
            admin: *admin,
            config: config_pda(),
            event: event_pda(event_id),
        },
    )
}

pub fn reject_candidate(admin: &Pubkey, event_id: u64, candidate: &Pubkey) -> Instruction {
    build(
        prequel::instruction::RejectCandidate {},
        prequel::accounts::RejectCandidate {
            admin: *admin,
            config: config_pda(),
            event: event_pda(event_id),
            candidate: *candidate,
        },
    )
}

/// `with_share`: also close the wallet's co-proposer share (pass `false` to omit the account).
pub fn refund(wallet: &Pubkey, event_id: u64, candidate: &Pubkey, with_share: bool) -> Instruction {
    build(
        prequel::instruction::Refund {},
        prequel::accounts::Refund {
            wallet: *wallet,
            event: event_pda(event_id),
            candidate: *candidate,
            vault: vault_pda(candidate),
            commitment: commitment_pda(candidate, wallet),
            co_proposer_share: with_share.then(|| coproposer_pda(candidate, wallet)),
        },
    )
}

impl Env {
    pub fn do_refund(
        &mut self,
        wallet: &solana_keypair::Keypair,
        event_id: u64,
        candidate: &Pubkey,
    ) -> Result<Sent, Failed> {
        let with_share = self.exists(&coproposer_pda(candidate, &wallet.pubkey()));
        self.send(
            &[refund(&wallet.pubkey(), event_id, candidate, with_share)],
            &[wallet],
        )
    }
    pub fn do_void(&mut self, event_id: u64) -> Result<Sent, Failed> {
        let admin = self.admin.insecure_clone();
        self.send(&[void_event(&admin.pubkey(), event_id)], &[&admin])
    }
    pub fn do_reject(&mut self, event_id: u64, candidate: &Pubkey) -> Result<Sent, Failed> {
        let admin = self.admin.insecure_clone();
        self.send(
            &[reject_candidate(&admin.pubkey(), event_id, candidate)],
            &[&admin],
        )
    }
    pub fn do_close(&mut self, event_id: u64) -> Result<Sent, Failed> {
        let caller = self.wallet(1);
        self.send(&[close_window(&caller.pubkey(), event_id)], &[&caller])
    }
}
