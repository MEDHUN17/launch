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
