//! Tests for initialize_config, update_config, set_pause and create_event.
mod common;

use common::ix::*;
use common::*;
use prequel::{
    constants::*, error::PrequelError as E, Config, EventState, EventStatus, ExternalListing,
    Params,
};
use solana_signer::Signer;

fn kp(env: &Env) -> solana_keypair::Keypair {
    env.admin.insecure_clone()
}

// ---- initialize_config ---------------------------------------------------------------------------------------------

#[test]
fn initialize_config_stores_keys_and_defaults_and_emits_an_event() {
    let mut env = Env::new();
    let ix = initialize_config(&env.deployer.pubkey(), default_init_args(&env));
    let sent = env.send(&[ix], &[&env.deployer.insecure_clone()]).unwrap();
    assert!(
        sent.logs.iter().any(|l| l.starts_with("Program data: ")),
        "ConfigUpdated event: {:?}",
        sent.logs
    );

    let c: Config = env.load(&config_pda());
    assert_eq!(c.admin, env.admin.pubkey());
    assert_eq!(c.resolver, env.resolver.pubkey());
    assert_eq!(c.canonicalizer, env.canonicalizer.pubkey());
    assert_eq!(c.treasury, env.treasury);
    assert_eq!(c.params, Params::spec_defaults());
    assert!(
        c.params.require_attestation,
        "the beta runs with attestation on"
    );
    assert!(!c.pause_proposals && !c.pause_commits);
    assert_eq!((c.version, c.next_event_id), (1, 0));
}

#[test]
fn initialize_config_requires_the_upgrade_authority() {
    let mut env = Env::new();
    let stranger = env.wallet(10);
    let ix = initialize_config(&stranger.pubkey(), default_init_args(&env));
    assert_err!(env.send(&[ix], &[&stranger]), E::NotDeployer);
    assert!(!env.exists(&config_pda()));
}

#[test]
fn initialize_config_cannot_run_twice() {
    let mut env = Env::new();
    env.init_config();
    let ix = initialize_config(&env.deployer.pubkey(), default_init_args(&env));
    let err = env
        .send(&[ix], &[&env.deployer.insecure_clone()])
        .unwrap_err();
    assert!(
        err.logs.iter().any(|l| l.contains("already in use")),
        "{err:?}"
    );
}

#[test]
fn initialize_config_rejects_bad_settings_and_keys() {
    let mut env = Env::new();
    let mut run = |edit: &dyn Fn(&mut prequel::InitializeConfigArgs), e: E| {
        let mut args = default_init_args(&env);
        edit(&mut args);
        let ix = initialize_config(&env.deployer.pubkey(), args);
        assert_err!(env.send(&[ix], &[&env.deployer.insecure_clone()]), e);
    };
    run(
        &|a| a.params.success_fee_bps = MAX_SUCCESS_FEE_BPS + 1,
        E::SuccessFeeTooHigh,
    );
    run(&|a| a.params.split_proposer_bps += 1, E::FeeSplitInvalid);
    run(
        &|a| a.params.dispute_window = MIN_DISPUTE_WINDOW - 1,
        E::DisputeWindowOutOfRange,
    );
    run(
        &|a| a.params.dispute_bond = MAX_DISPUTE_BOND + 1,
        E::DisputeBondOutOfRange,
    );
    run(
        &|a| a.params.max_resolution_delay = MAX_RESOLUTION_DELAY + 1,
        E::ResolutionDelayOutOfRange,
    );
    run(
        &|a| a.params.launch_window = MIN_LAUNCH_WINDOW - 1,
        E::LaunchWindowOutOfRange,
    );
    run(&|a| a.params.proposal_fee = 0, E::ProposalFeeOutOfRange);
    run(&|a| a.params.min_commit = 1, E::MinCommitTooLow);
    run(
        &|a| a.params.min_raise = MAX_MIN_RAISE + 1,
        E::MinRaiseOutOfRange,
    );
    run(&|a| a.params.max_commit_per_wallet = 1, E::CommitCapTooLow);
    run(
        &|a| a.params.max_candidates_per_outcome = 33,
        E::SlateSizeOutOfRange,
    );
    run(
        &|a| a.admin = anchor_lang::prelude::Pubkey::default(),
        E::InvalidKey,
    );
    run(
        &|a| a.treasury = anchor_lang::prelude::Pubkey::default(),
        E::InvalidKey,
    );
    assert!(
        !env.exists(&config_pda()),
        "nothing was created by the failed attempts"
    );
}

// ---- update_config -------------------------------------------------------------------------------------------------

#[test]
fn update_config_is_admin_only_validated_and_partial() {
    let mut env = Env::new();
    env.init_config();
    let none = || prequel::UpdateConfigArgs {
        admin: None,
        resolver: None,
        canonicalizer: None,
        treasury: None,
        params: None,
    };

    // a stranger cannot
    let stranger = env.wallet(10);
    let ix = update_config(
        &stranger.pubkey(),
        prequel::UpdateConfigArgs {
            resolver: Some(stranger.pubkey()),
            ..none()
        },
    );
    assert_err!(env.send(&[ix], &[&stranger]), E::NotAdmin);

    // out-of-range settings are rejected, the default key is rejected
    let mut bad = Params::spec_defaults();
    bad.success_fee_bps = 10_000;
    let ix = update_config(
        &env.admin.pubkey(),
        prequel::UpdateConfigArgs {
            params: Some(bad),
            ..none()
        },
    );
    assert_err!(env.send(&[ix], &[&kp(&env)]), E::SuccessFeeTooHigh);
    let ix = update_config(
        &env.admin.pubkey(),
        prequel::UpdateConfigArgs {
            treasury: Some(Default::default()),
            ..none()
        },
    );
    assert_err!(env.send(&[ix], &[&kp(&env)]), E::InvalidKey);

    // a partial update changes only what is given; rotating the admin hands over the role
    let new_resolver = env.wallet(1);
    let mut p = Params::spec_defaults();
    p.min_commit = SOL / 10;
    let ix = update_config(
        &env.admin.pubkey(),
        prequel::UpdateConfigArgs {
            resolver: Some(new_resolver.pubkey()),
            params: Some(p),
            ..none()
        },
    );
    env.send(&[ix], &[&kp(&env)]).unwrap();
    let c: Config = env.load(&config_pda());
    assert_eq!(c.resolver, new_resolver.pubkey());
    assert_eq!(c.admin, env.admin.pubkey());
    assert_eq!(c.params.min_commit, SOL / 10);

    let new_admin = env.wallet(1);
    let ix = update_config(
        &env.admin.pubkey(),
        prequel::UpdateConfigArgs {
            admin: Some(new_admin.pubkey()),
            ..none()
        },
    );
    env.send(&[ix], &[&kp(&env)]).unwrap();
    let ix = update_config(&env.admin.pubkey(), none());
    assert_err!(env.send(&[ix], &[&kp(&env)]), E::NotAdmin); // the old admin is out
    let ix = update_config(&new_admin.pubkey(), none());
    env.send(&[ix], &[&new_admin]).unwrap();
}

#[test]
fn update_config_changes_future_events_only() {
    let mut env = Env::new();
    env.init_config();
    let (_, first) = env.create_default_event("MARKET-A");
    let mut p = Params::spec_defaults();
    p.success_fee_bps = 250;
    p.min_commit = SOL / 10;
    p.require_attestation = false;
    let ix = update_config(
        &env.admin.pubkey(),
        prequel::UpdateConfigArgs {
            admin: None,
            resolver: None,
            canonicalizer: None,
            treasury: None,
            params: Some(p),
        },
    );
    env.send(&[ix], &[&kp(&env)]).unwrap();
    let (_, second) = env.create_default_event("MARKET-B");

    let e0: EventState = env.load(&first);
    let e1: EventState = env.load(&second);
    assert_eq!(
        e0.params,
        Params::spec_defaults(),
        "the live event keeps the settings it was created with"
    );
    assert_eq!(e1.params, p, "a new event gets the new settings");
}

// ---- set_pause -----------------------------------------------------------------------------------------------------

#[test]
fn set_pause_is_admin_only_and_flags_are_independent() {
    let mut env = Env::new();
    env.init_config();
    let stranger = env.wallet(10);
    assert_err!(
        env.send(
            &[set_pause(&stranger.pubkey(), Some(true), None)],
            &[&stranger]
        ),
        E::NotAdmin
    );

    env.send(
        &[set_pause(&env.admin.pubkey(), Some(true), None)],
        &[&kp(&env)],
    )
    .unwrap();
    let c: Config = env.load(&config_pda());
    assert!(c.pause_proposals && !c.pause_commits);
    env.send(
        &[set_pause(&env.admin.pubkey(), None, Some(true))],
        &[&kp(&env)],
    )
    .unwrap();
    let c: Config = env.load(&config_pda());
    assert!(
        c.pause_proposals && c.pause_commits,
        "None leaves a flag unchanged"
    );
    env.send(
        &[set_pause(&env.admin.pubkey(), Some(false), Some(false))],
        &[&kp(&env)],
    )
    .unwrap();
    let c: Config = env.load(&config_pda());
    assert!(!c.pause_proposals && !c.pause_commits);
}

// ---- create_event --------------------------------------------------------------------------------------------------

#[test]
fn create_event_freezes_settings_and_lists_the_market() {
    let mut env = Env::new();
    env.init_config();
    let args = default_event_args(env.now(), "KXNBA-26NOV05-LAL");
    let ix = create_event(&env.admin.pubkey(), 0, args.clone());
    let sent = env.send(&[ix], &[&kp(&env)]).unwrap();
    assert!(
        sent.logs.iter().any(|l| l.starts_with("Program data: ")),
        "EventCreated event"
    );

    let e: EventState = env.load(&event_pda(0));
    assert_eq!((e.id, e.source, e.outcome_count), (0, SOURCE_KALSHI, 2));
    assert_eq!(e.status, EventStatus::Open);
    assert_eq!(
        (
            e.window_open,
            e.window_close,
            e.event_start,
            e.expected_resolution
        ),
        (
            args.window_open,
            args.window_close,
            args.event_start,
            args.expected_resolution
        )
    );
    assert_eq!(
        (e.proposed_outcome, e.resolved_outcome),
        (NO_OUTCOME, NO_OUTCOME)
    );
    assert_eq!(e.candidate_count, [0; 8]);
    assert_eq!(e.params, Params::spec_defaults());
    assert_eq!(e.canon_version, 1);
    assert_eq!(e.created_at, T0);

    let l: ExternalListing = env.load(&external_pda(&args.external_id_hash));
    assert_eq!(l.event, event_pda(0));
    assert_eq!(env.load::<Config>(&config_pda()).next_event_id, 1);
}

#[test]
fn create_event_rejects_bad_input_and_changes_nothing() {
    let mut env = Env::new();
    env.init_config();
    let now = env.now();
    let mut run = |edit: &dyn Fn(&mut prequel::CreateEventArgs), e: E| {
        let mut args = default_event_args(now, "KXNBA-X");
        edit(&mut args);
        let ix = create_event(&env.admin.pubkey(), 0, args);
        assert_err!(env.send(&[ix], &[&env.admin.insecure_clone()]), e);
    };
    run(&|a| a.source = 1, E::UnsupportedSource);
    run(&|a| a.outcome_count = 1, E::InvalidOutcomeCount);
    run(&|a| a.outcome_count = 9, E::InvalidOutcomeCount);
    run(
        &|a| {
            a.external_id = String::new();
            a.external_id_hash = sha256(b"");
        },
        E::ExternalIdInvalid,
    );
    run(
        &|a| {
            a.external_id = "x".repeat(65);
            a.external_id_hash = sha256(a.external_id.as_bytes());
        },
        E::ExternalIdInvalid,
    );
    run(&|a| a.external_id_hash = [7; 32], E::ExternalIdHashMismatch);
    run(&|a| a.window_close = now, E::InvalidEventTimes); // must close in the future
    run(&|a| a.window_close = a.event_start, E::InvalidEventTimes);
    run(
        &|a| a.window_open = a.window_close + 1,
        E::InvalidEventTimes,
    );
    run(
        &|a| a.expected_resolution = a.event_start - 1,
        E::InvalidEventTimes,
    );
    run(
        &|a| a.expected_resolution = a.event_start + MAX_RESOLUTION_AFTER_START + 1,
        E::InvalidEventTimes,
    );
    let stranger = env.wallet(10);
    let ix = create_event(&stranger.pubkey(), 0, default_event_args(now, "KXNBA-X"));
    assert_err!(env.send(&[ix], &[&stranger]), E::NotAdmin);

    assert!(!env.exists(&event_pda(0)));
    assert_eq!(
        env.load::<Config>(&config_pda()).next_event_id,
        0,
        "failed attempts do not consume ids"
    );
}

#[test]
fn the_same_market_cannot_be_listed_twice_and_ids_increment() {
    let mut env = Env::new();
    env.init_config();
    let (id0, _) = env.create_default_event("MARKET-1");
    let (id1, _) = env.create_default_event("MARKET-2");
    assert_eq!((id0, id1), (0, 1));

    let ix = create_event(
        &env.admin.pubkey(),
        2,
        default_event_args(env.now(), "MARKET-1"),
    );
    let err = env.send(&[ix], &[&kp(&env)]).unwrap_err();
    assert!(
        err.logs.iter().any(|l| l.contains("already in use")),
        "{err:?}"
    );
    assert_eq!(env.load::<Config>(&config_pda()).next_event_id, 2);
    assert!(!env.exists(&event_pda(2)));
}
