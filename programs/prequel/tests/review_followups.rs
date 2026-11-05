//! Tests added after the second independent code review (findings L1 to L11 and its surviving-mutation list).
//! Each test names the finding it closes. They are written to fail if the corresponding line of the program is
//! deleted or changed, which `scripts/mutation-sweep.sh` checks.
mod common;

use anchor_lang::{prelude::Pubkey, AccountSerialize};
use common::ix::*;
use common::*;
use prequel::{
    constants::MAX_PROPOSAL_CUTOFF, error::PrequelError as E, Commitment, Config, EventState,
    Params, UpdateConfigArgs,
};
use solana_keypair::Keypair;
use solana_signer::Signer;

fn no_change() -> UpdateConfigArgs {
    UpdateConfigArgs {
        admin: None,
        resolver: None,
        canonicalizer: None,
        treasury: None,
        params: None,
    }
}

fn update(env: &mut Env, args: UpdateConfigArgs) -> Result<Sent, Failed> {
    let admin = env.admin.insecure_clone();
    env.send(&[update_config(&admin.pubkey(), args)], &[&admin])
}

fn config_params(env: &Env) -> Params {
    env.load::<Config>(&config_pda()).params
}

/// An attested proposal: the canonicalizer's Ed25519 instruction right before `propose_candidate`, signed for an
/// expiry ten minutes from the (current) clock.
fn attested_propose(
    env: &mut Env,
    w: &Keypair,
    event: u64,
    ticker: &str,
    outcome: u8,
    amount: u64,
) -> Result<Sent, Failed> {
    let mut args = propose_args(ticker, outcome, amount);
    args.attestation_expiry = env.now() + 600;
    let att = attestation_ix(&env.canonicalizer, event, &args, &w.pubkey());
    let ix = propose(&w.pubkey(), event, &env.treasury, &args);
    env.send(&[att, ix], &[w])
}

/// Replaces fields of a stored event, keeping the account's size (test-only: the program has no such path).
fn patch_event(env: &mut Env, id: u64, f: impl FnOnce(&mut EventState)) {
    let key = event_pda(id);
    let mut acc = env.svm.get_account(&key).expect("event account");
    let mut ev: EventState = env.load(&key);
    f(&mut ev);
    let mut data = Vec::new();
    ev.try_serialize(&mut data).expect("serialize event");
    data.resize(acc.data.len(), 0);
    acc.data = data;
    env.svm.set_account(key, acc).unwrap();
}

// ---- L1: the settings an event froze are the ones every handler enforces ---------------------------------------------

/// L1 / mutation 2. Changes every setting that a handler reads, in the stricter direction, after an event exists.
/// The old event must keep enforcing what it froze; an event created afterwards must enforce the new values.
/// If any handler read `config.params` instead of `event.params`, one of the steps below would fail.
#[test]
fn an_event_keeps_the_settings_it_was_created_with_whatever_the_config_says_later() {
    let mut env = Env::new();
    env.init_config_with(|p| {
        p.min_commit = SOL / 20;
        p.max_commit_per_wallet = 2 * SOL;
        p.max_candidates_per_outcome = 2;
        p.proposal_fee = SOL / 50;
        p.require_attestation = false;
        p.proposal_cutoff = 0;
    });
    let (a, _) = env.create_default_event("EV-A");
    let frozen = env.load::<EventState>(&event_pda(a)).params;
    assert_eq!(frozen, config_params(&env));

    let mut stricter = frozen;
    stricter.min_commit = SOL / 2;
    stricter.max_commit_per_wallet = SOL;
    stricter.max_candidates_per_outcome = 1;
    stricter.proposal_fee = SOL / 2;
    stricter.require_attestation = true;
    stricter.proposal_cutoff = 1_800;
    update(
        &mut env,
        UpdateConfigArgs {
            params: Some(stricter),
            ..no_change()
        },
    )
    .unwrap();
    assert_eq!(config_params(&env), stricter);
    assert_eq!(env.load::<EventState>(&event_pda(a)).params, frozen);

    let (w1, w2, w3) = (env.wallet(100), env.wallet(100), env.wallet(100));

    // minimum commit 0.05 (not 0.5), fee 0.02 (not 0.5), no attestation (though the config now demands one)
    let treasury_before = env.lamports(&env.treasury);
    let aaa = env
        .do_propose(&w1, a, &propose_args("AAA", 0, SOL / 10))
        .expect("event A: no attestation, minimum 0.05 SOL");
    assert_eq!(
        env.lamports(&env.treasury) - treasury_before,
        SOL / 50,
        "event A charges the frozen proposal fee"
    );

    // per-wallet cap 2 SOL (not 1 SOL)
    env.do_commit(&w2, a, &aaa, 3 * SOL / 2)
        .expect("1.5 SOL is under the frozen cap");
    assert_err!(env.do_commit(&w2, a, &aaa, 3 * SOL / 5), E::CommitAboveCap);

    // slate of 2 (not 1)
    env.do_propose(&w1, a, &propose_args("BBB", 0, SOL / 10))
        .expect("event A has two slots");
    assert_err!(
        env.do_propose(&w1, a, &propose_args("CCC", 0, SOL / 10)),
        E::SlateFull
    );

    // co_propose: the frozen fee and the frozen minimum
    let before = env.lamports(&env.treasury);
    env.do_co_propose(&w3, a, &aaa, SOL / 10)
        .expect("0.1 SOL is above the frozen minimum");
    assert_eq!(env.lamports(&env.treasury) - before, SOL / 50);

    // no cutoff on this event: proposals run until close even though the config now stops them 30 minutes early
    env.warp_to(T0 + 3_000);
    env.do_propose(&w1, a, &propose_args("DDD", 1, SOL / 10))
        .expect("event A has no proposal cutoff");

    // an event created now takes the new settings
    let (b, _) = env.create_default_event("EV-B");
    assert_eq!(env.load::<EventState>(&event_pda(b)).params, stricter);
    let w4 = env.wallet(100);
    let w5 = env.wallet(100);
    let mut bare = propose_args("EEE", 0, SOL);
    bare.attestation_expiry = env.now() + 600; // fresh, so that the missing instruction is what fails
    assert_err!(env.do_propose(&w4, b, &bare), E::AttestationMissing);
    assert_err!(
        attested_propose(&mut env, &w4, b, "EEE", 0, SOL / 10),
        E::CommitBelowMinimum
    );
    let before = env.lamports(&env.treasury);
    attested_propose(&mut env, &w4, b, "EEE", 0, SOL).expect("meets the new minimum");
    assert_eq!(env.lamports(&env.treasury) - before, SOL / 2, "the new fee");
    assert_err!(
        attested_propose(&mut env, &w4, b, "FFF", 0, SOL),
        E::SlateFull
    );
    let eee = candidate_for(b, &propose_args("EEE", 0, SOL));
    assert_err!(env.do_commit(&w4, b, &eee, SOL / 2), E::CommitAboveCap);

    // the new cutoff: proposals stop 1,800 seconds before close, at that exact second; commits go on
    let close_b = env.load::<EventState>(&event_pda(b)).window_close;
    env.warp_to(close_b - 1_801);
    attested_propose(&mut env, &w4, b, "GGG", 1, SOL).expect("one second before the cutoff");
    env.warp_to(close_b - 1_800);
    assert_err!(
        attested_propose(&mut env, &w4, b, "HHH", 1, SOL),
        E::ProposalCutoffPassed
    );
    env.do_commit(&w5, b, &eee, SOL / 2)
        .expect("commits are not stopped by the cutoff");
}

// ---- L2: the ProgramData binding -----------------------------------------------------------------------------------

/// L2 / mutation 1. Anyone can deploy a program of their own and be its upgrade authority. Without the check that
/// `program_data` belongs to THIS program, they could pass their own ProgramData and take the config.
#[test]
fn initialize_config_refuses_another_programs_program_data() {
    let mut env = Env::new();
    let attacker = env.wallet(10);
    let other_program = Pubkey::new_unique();
    env.svm.add_program(other_program, PROGRAM_BYTES).unwrap();
    let other_data =
        Pubkey::find_program_address(&[other_program.as_ref()], &BPF_LOADER_UPGRADEABLE).0;
    set_upgrade_authority_of(&mut env.svm, &other_data, &attacker.pubkey());

    let mut ix = initialize_config(&attacker.pubkey(), default_init_args(&env));
    for meta in ix.accounts.iter_mut() {
        if meta.pubkey == program_data_address() {
            meta.pubkey = other_data;
        }
    }
    assert_err!(env.send(&[ix], &[&attacker]), E::NotDeployer);
    assert!(
        !env.exists(&config_pda()),
        "the attacker did not get a config"
    );

    // and the real deployer still can
    env.init_config_with(|_| {});
    assert!(env.exists(&config_pda()));
}

// ---- L4: reserved settings bytes ---------------------------------------------------------------------------------

/// L4 and the missing `ProposalCutoffOutOfRange` integration test.
#[test]
fn reserved_settings_bytes_must_be_zero_and_the_cutoff_is_bounded_at_init_and_update() {
    let mut env = Env::new();
    let deployer = env.deployer.insecure_clone();
    let init = |env: &mut Env, edit: &dyn Fn(&mut Params)| {
        let mut args = default_init_args(env);
        edit(&mut args.params);
        env.send(&[initialize_config(&deployer.pubkey(), args)], &[&deployer])
    };
    assert_err!(
        init(&mut env, &|p| p.reserved[3] = 7),
        E::ParamsReservedNotZero
    );
    assert_err!(
        init(&mut env, &|p| p.proposal_cutoff = MAX_PROPOSAL_CUTOFF + 1),
        E::ProposalCutoffOutOfRange
    );
    assert!(!env.exists(&config_pda()), "failed inits created nothing");

    env.init_config_with(|_| {});
    let before = config_params(&env);
    let mut p = before;
    p.reserved[31] = 1;
    assert_err!(
        update(
            &mut env,
            UpdateConfigArgs {
                params: Some(p),
                ..no_change()
            }
        ),
        E::ParamsReservedNotZero
    );
    let mut p = before;
    p.proposal_cutoff = MAX_PROPOSAL_CUTOFF + 1;
    assert_err!(
        update(
            &mut env,
            UpdateConfigArgs {
                params: Some(p),
                ..no_change()
            }
        ),
        E::ProposalCutoffOutOfRange
    );
    assert_eq!(
        config_params(&env),
        before,
        "refused updates change nothing"
    );
}

// ---- L10 / mutation 6: every key is validated, rotation takes effect ----------------------------------------------

#[test]
fn every_key_refuses_the_zero_address_at_init_and_update() {
    let default = Pubkey::default();
    for which in 0..4 {
        let mut env = Env::new();
        let mut args = default_init_args(&env);
        match which {
            0 => args.admin = default,
            1 => args.resolver = default,
            2 => args.canonicalizer = default,
            _ => args.treasury = default,
        }
        let deployer = env.deployer.insecure_clone();
        let r = env.send(&[initialize_config(&deployer.pubkey(), args)], &[&deployer]);
        assert!(
            matches!(&r, Err(f) if f.is(E::InvalidKey)),
            "init key #{which}: {r:?}"
        );
        assert!(!env.exists(&config_pda()));
    }

    let mut env = Env::new();
    env.init_config_with(|_| {});
    let snapshot = env.svm.get_account(&config_pda()).unwrap().data;
    for (name, args) in [
        (
            "admin",
            UpdateConfigArgs {
                admin: Some(default),
                ..no_change()
            },
        ),
        (
            "resolver",
            UpdateConfigArgs {
                resolver: Some(default),
                ..no_change()
            },
        ),
        (
            "canonicalizer",
            UpdateConfigArgs {
                canonicalizer: Some(default),
                ..no_change()
            },
        ),
        (
            "treasury",
            UpdateConfigArgs {
                treasury: Some(default),
                ..no_change()
            },
        ),
    ] {
        let r = update(&mut env, args);
        assert!(
            matches!(&r, Err(f) if f.is(E::InvalidKey)),
            "update {name}: {r:?}"
        );
    }
    assert_eq!(
        env.svm.get_account(&config_pda()).unwrap().data,
        snapshot,
        "refused updates change nothing"
    );
}

#[test]
fn rotating_every_key_takes_effect_at_once() {
    let mut env = Env::new();
    env.init_config_with(|_| {});
    let new_admin = env.wallet(10);
    let (new_resolver, new_canon, new_treasury) = (
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );
    let old_admin = env.admin.insecure_clone();
    let old_treasury = env.treasury;
    update(
        &mut env,
        UpdateConfigArgs {
            admin: Some(new_admin.pubkey()),
            resolver: Some(new_resolver),
            canonicalizer: Some(new_canon),
            treasury: Some(new_treasury),
            params: None,
        },
    )
    .unwrap();
    let c: Config = env.load(&config_pda());
    assert_eq!(
        (c.admin, c.resolver, c.canonicalizer, c.treasury),
        (new_admin.pubkey(), new_resolver, new_canon, new_treasury)
    );

    // the old admin is out, the new one is in
    assert_err!(
        env.send(
            &[set_pause(&old_admin.pubkey(), Some(true), None)],
            &[&old_admin]
        ),
        E::NotAdmin
    );
    env.send(
        &[set_pause(&new_admin.pubkey(), Some(true), None)],
        &[&new_admin],
    )
    .unwrap();
    env.send(
        &[set_pause(&new_admin.pubkey(), Some(false), None)],
        &[&new_admin],
    )
    .unwrap();

    // fees go to the new treasury; presenting the old one is refused
    env.admin = new_admin.insecure_clone();
    let (ev, _) = env.create_default_event("EV-ROT");
    let w = env.wallet(10);
    let args = propose_args("TRE", 0, SOL / 10);
    let stale = propose(&w.pubkey(), ev, &old_treasury, &args);
    assert_err!(env.send(&[stale], &[&w]), E::AccountMismatch);
    env.treasury = new_treasury;
    env.do_propose(&w, ev, &args).unwrap();
    assert_eq!(env.lamports(&new_treasury), SOL / 50);
    assert_eq!(env.lamports(&old_treasury), 0);
}

// ---- mutation 7: the cutoff must leave room in the window -------------------------------------------------------------

#[test]
fn a_cutoff_must_be_shorter_than_the_window() {
    let mut env = Env::new();
    env.init_config_with(|p| p.proposal_cutoff = 3_600);
    let admin = env.admin.insecure_clone();
    let args = default_event_args(env.now(), "EV-CUT"); // a 3,600 second window
    assert_err!(
        env.send(&[create_event(&admin.pubkey(), 0, args.clone())], &[&admin]),
        E::InvalidEventTimes
    );
    assert_eq!(env.load::<Config>(&config_pda()).next_event_id, 0);
    assert!(!env.exists(&event_pda(0)) && !env.exists(&external_pda(&args.external_id_hash)));

    let mut p = config_params(&env);
    p.proposal_cutoff = 3_599;
    update(
        &mut env,
        UpdateConfigArgs {
            params: Some(p),
            ..no_change()
        },
    )
    .unwrap();
    env.send(&[create_event(&admin.pubkey(), 0, args)], &[&admin])
        .expect("one second shorter than the window is allowed");
}

// ---- mutation 3: rejection and co-proposing outlive the proposal cutoff -----------------------------------------------

/// The cutoff exists to give the admin time to review new candidates, so rejection must keep working after
/// proposals have stopped, until the close itself. Co-proposing joins an existing candidate and is not stopped by it.
#[test]
fn rejection_and_co_proposing_continue_after_proposals_have_stopped() {
    let mut env = Env::new();
    env.init_config_with(|p| p.proposal_cutoff = 1_800);
    let (ev, _) = env.create_default_event("EV-REJ"); // window T0..T0+3600, proposals stop at T0+1800
    let w = env.wallet(20);
    let co = env.wallet(20);
    let c1 = env
        .do_propose(&w, ev, &propose_args("ONE", 0, SOL / 10))
        .unwrap();
    let c2 = env
        .do_propose(&w, ev, &propose_args("TWO", 0, SOL / 10))
        .unwrap();
    let c3 = env
        .do_propose(&w, ev, &propose_args("THR", 0, SOL / 10))
        .unwrap();

    env.warp_to(T0 + 1_800);
    assert_err!(
        env.do_propose(&w, ev, &propose_args("NEW", 0, SOL / 10)),
        E::ProposalCutoffPassed
    );
    env.do_co_propose(&co, ev, &c1, SOL / 10)
        .expect("co-proposing is a commit, not a new proposal");
    env.do_reject(ev, &c1)
        .expect("rejection runs until close, not until the cutoff");

    env.warp_to(T0 + 3_599);
    env.do_reject(ev, &c2)
        .expect("the last second before close");
    env.warp_to(T0 + 3_600);
    assert_err!(env.do_reject(ev, &c3), E::WindowClosed);
}

// ---- L5: the ticker table version ----------------------------------------------------------------------------------

#[test]
fn proposals_on_an_event_from_another_table_version_are_refused_but_commits_and_refunds_are_not() {
    let mut env = Env::new();
    env.init_config_with(|_| {});
    let (ev, _) = env.create_default_event("EV-V");
    let w = env.wallet(10);
    patch_event(&mut env, ev, |e| e.canon_version = 2);
    assert_err!(
        env.do_propose(&w, ev, &propose_args("BOB", 0, SOL / 10)),
        E::CanonVersionMismatch
    );
    assert!(!env.exists(&candidate_for(ev, &propose_args("BOB", 0, SOL / 10))));

    patch_event(&mut env, ev, |e| e.canon_version = 1);
    let c = env
        .do_propose(&w, ev, &propose_args("BOB", 0, SOL / 10))
        .unwrap();
    patch_event(&mut env, ev, |e| e.canon_version = 2);
    env.do_commit(&w, ev, &c, SOL / 10)
        .expect("commits do not depend on the ticker table");
    env.do_void(ev).unwrap();
    env.do_refund(&w, ev, &c).expect("neither do refunds");
}

// ---- L6: a co-proposer's share is closed with its commitment ------------------------------------------------------------

#[test]
fn a_co_proposer_cannot_refund_without_closing_its_share() {
    let mut env = Env::new();
    env.init_config_with(|_| {});
    let (ev, _) = env.create_default_event("EV-S");
    let (proposer, co, plain) = (env.wallet(10), env.wallet(10), env.wallet(10));
    let c = env
        .do_propose(&proposer, ev, &propose_args("SHR", 0, SOL / 10))
        .unwrap();
    env.do_commit(&plain, ev, &c, SOL / 10).unwrap();
    env.do_co_propose(&co, ev, &c, SOL / 10).unwrap();
    let has_share = |env: &Env, w: &Keypair| {
        env.load::<Commitment>(&commitment_pda(&c, &w.pubkey()))
            .has_share
    };
    assert!(has_share(&env, &co));
    assert!(!has_share(&env, &plain) && !has_share(&env, &proposer));
    env.do_void(ev).unwrap();

    // leaving the share out is refused, and nothing moves
    let vault_before = env.lamports(&vault_pda(&c));
    assert_err!(
        env.send(&[refund(&co.pubkey(), ev, &c, false)], &[&co]),
        E::CoProposerShareRequired
    );
    assert_eq!(env.lamports(&vault_pda(&c)), vault_before);
    assert!(env.exists(&commitment_pda(&c, &co.pubkey())));
    assert!(env.exists(&coproposer_pda(&c, &co.pubkey())));

    // with the share, both accounts close and the wallet gets both rents back with the refund
    let rents = env.lamports(&commitment_pda(&c, &co.pubkey()))
        + env.lamports(&coproposer_pda(&c, &co.pubkey()));
    let wallet_before = env.lamports(&co.pubkey());
    env.send(&[refund(&co.pubkey(), ev, &c, true)], &[&co])
        .unwrap();
    assert!(!env.exists(&commitment_pda(&c, &co.pubkey())));
    assert!(!env.exists(&coproposer_pda(&c, &co.pubkey())));
    let gain = env.lamports(&co.pubkey()) as i128 - wallet_before as i128;
    let expect = (SOL / 10 + rents) as i128;
    assert!(
        gain <= expect && gain >= expect - 20_000,
        "got {gain}, expected {expect} less a fee"
    );

    // a wallet that never co-proposed may leave the share out: there is none
    env.send(&[refund(&plain.pubkey(), ev, &c, false)], &[&plain])
        .unwrap();
    env.send(&[refund(&proposer.pubkey(), ev, &c, false)], &[&proposer])
        .unwrap();
}

// ---- L9: names and URIs ----------------------------------------------------------------------------------------------------

#[test]
fn propose_refuses_invisible_name_characters_and_unsafe_uris() {
    let mut env = Env::new();
    env.init_config_with(|_| {});
    let (ev, _) = env.create_default_event("EV-TXT");
    let w = env.wallet(10);
    for name in ["Pepe\u{202E}", "Pe\u{200B}pe", "\u{2066}Pepe"] {
        let mut a = propose_args("TXT", 0, SOL / 10);
        a.name = name.to_string();
        assert_err!(env.do_propose(&w, ev, &a), E::NameInvalid);
    }
    for uri in [
        "javascript:alert(1)",
        "data:text/html;base64,AAAA",
        "http://example.com/x.json",
        "ipfs://",
        "ftp://x",
    ] {
        let mut a = propose_args("TXT", 0, SOL / 10);
        a.uri = uri.to_string();
        assert_err!(env.do_propose(&w, ev, &a), E::UriInvalid);
    }
    // the refused attempts left nothing behind, and the allowed forms work
    let mut a = propose_args("TXT", 0, SOL / 10);
    assert!(!env.exists(&candidate_for(ev, &a)));
    a.uri =
        "https://gateway.pinata.cloud/ipfs/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .to_string();
    a.name = "Caf\u{e9} \u{1f438}".to_string();
    env.do_propose(&w, ev, &a)
        .expect("https URL, accented name and emoji");
}
