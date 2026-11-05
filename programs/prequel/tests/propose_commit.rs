//! Tests for propose_candidate, commit and co_propose: happy paths, every failure, attestation, boundaries.
mod common;

use anchor_lang::Space;
use common::ix::*;
use common::*;
use prequel::{
    error::PrequelError as E, Candidate, CandidateStatus, CoProposerShare, Commitment, EventState,
    Params, Vault,
};
use solana_signer::Signer;

/// Config with attestation off, one default event (window T0..T0+3600), returns the env.
fn setup() -> Env {
    let mut env = Env::new();
    env.init_config_with(|_| {});
    env.create_default_event("KXNBA-1");
    env
}

fn has_log(f: &Failed, needle: &str) -> bool {
    f.logs.iter().any(|l| l.contains(needle))
}

// ---- propose_candidate: the happy path --------------------------------------------------------------------------

#[test]
fn propose_creates_candidate_vault_and_commitment_and_pays_the_fee() {
    let mut env = setup();
    let proposer = env.wallet(10);
    let args = propose_args("BOB", 0, SOL);
    let treasury_before = env.lamports(&env.treasury);
    let candidate = env.do_propose(&proposer, 0, &args).unwrap();

    let c: Candidate = env.load(&candidate);
    assert_eq!(
        (c.event, c.outcome, c.proposer),
        (event_pda(0), 0, proposer.pubkey())
    );
    assert_eq!(
        (c.name.as_str(), c.ticker.as_str(), c.uri.as_str()),
        ("BOB Coin", "BOB", "ipfs://bafybob")
    );
    assert_eq!(c.canon_hash, args.canon_hash);
    assert_eq!(
        (
            c.total_committed,
            c.committer_count,
            c.co_proposer_total,
            c.co_proposer_count
        ),
        (SOL, 1, 0, 0)
    );
    assert_eq!(
        (c.status, c.refunded_total, c.created_at),
        (CandidateStatus::Pending, 0, T0)
    );
    let cm: Commitment = env.load(&commitment_pda(&candidate, &proposer.pubkey()));
    assert_eq!(
        (cm.amount, cm.first_commit_ts, cm.wallet, cm.candidate),
        (SOL, T0, proposer.pubkey(), candidate)
    );
    let v: Vault = env.load(&vault_pda(&candidate));
    assert_eq!(v.candidate, candidate);

    let rent = env.rent_minimum(8 + Vault::INIT_SPACE);
    assert_eq!(
        env.lamports(&vault_pda(&candidate)),
        rent + SOL,
        "the vault holds its rent plus exactly the commit"
    );
    assert_eq!(
        env.lamports(&env.treasury) - treasury_before,
        SOL / 50,
        "proposal fee goes to the treasury"
    );
    assert_eq!(
        env.load::<EventState>(&event_pda(0)).candidate_count,
        [1, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn display_ticker_is_uppercased_but_keeps_digits() {
    let mut env = setup();
    let w = env.wallet(10);
    let c = env.do_propose(&w, 0, &propose_args("b0b", 1, SOL)).unwrap();
    assert_eq!(env.load::<Candidate>(&c).ticker, "B0B");
}

// ---- Canon dedup (invariant 8) ----------------------------------------------------------------------------------

#[test]
fn duplicates_are_rejected_even_when_disguised() {
    let mut env = setup();
    let w = env.wallet(100);
    env.do_propose(&w, 0, &propose_args("BOB", 0, SOL)).unwrap();
    for disguise in ["BOB", "bob", "BoB", "B0B", "b0b"] {
        let err = env
            .do_propose(&w, 0, &propose_args(disguise, 0, SOL))
            .unwrap_err();
        assert!(has_log(&err, "already in use"), "{disguise}: {err:?}");
    }
    // the canon hash argument cannot be forged to dodge the seed: a different hash for the same ticker is refused
    let mut forged = propose_args("B0B", 0, SOL);
    forged.canon_hash = sha256(b"something else");
    assert_err!(env.do_propose(&w, 0, &forged), E::CanonHashMismatch);
    // different outcome, different event: allowed
    env.do_propose(&w, 0, &propose_args("BOB", 1, SOL)).unwrap();
    let (id1, _) = env.create_default_event("KXNBA-2");
    env.do_propose(&w, id1, &propose_args("BOB", 0, SOL))
        .unwrap();
}

#[test]
fn slate_cap_is_per_outcome_and_failed_proposals_do_not_use_slots() {
    let mut env = Env::new();
    env.init_config_with(|p| p.max_candidates_per_outcome = 2);
    env.create_default_event("KXNBA-1");
    let w = env.wallet(100);
    env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)).unwrap();
    assert_err!(
        env.do_propose(&w, 0, &propose_args("AAA", 1, 1)),
        E::CommitBelowMinimum
    ); // fails, no slot used
    env.do_propose(&w, 0, &propose_args("BBB", 0, SOL)).unwrap();
    assert_err!(
        env.do_propose(&w, 0, &propose_args("CCC", 0, SOL)),
        E::SlateFull
    );
    env.do_propose(&w, 0, &propose_args("CCC", 1, SOL)).unwrap(); // the other outcome has its own slate
    assert_eq!(
        env.load::<EventState>(&event_pda(0)).candidate_count[..2],
        [2, 1]
    );
}

// ---- propose_candidate: failures --------------------------------------------------------------------------------

#[test]
fn propose_rejects_bad_input_and_changes_nothing() {
    let mut env = setup();
    let w = env.wallet(100);
    let mut run = |edit: &dyn Fn(&mut prequel::ProposeArgs), e: E| {
        let mut a = propose_args("OKAY", 0, SOL);
        edit(&mut a);
        assert_err!(env.do_propose(&w, 0, &a), e);
    };
    run(&|a| a.outcome = 2, E::InvalidOutcome);
    run(&|a| a.name = String::new(), E::NameInvalid);
    run(&|a| a.name = "n".repeat(33), E::NameInvalid);
    run(&|a| a.uri = String::new(), E::UriInvalid);
    run(&|a| a.uri = "u".repeat(201), E::UriInvalid);
    run(
        &|a| {
            a.ticker = "A".into();
        },
        E::TickerLength,
    );
    run(
        &|a| {
            a.ticker = "ABCDEFGHIJK".into();
        },
        E::TickerLength,
    );
    run(
        &|a| {
            a.ticker = "AB-C".into();
        },
        E::TickerCharacters,
    );
    run(
        &|a| {
            a.ticker = "\u{00df}\u{00df}".into();
        },
        E::TickerCharacters,
    );
    run(&|a| a.amount = SOL / 20 - 1, E::CommitBelowMinimum);
    run(&|a| a.amount = 50 * SOL + 1, E::CommitAboveCap);
    let wrong_treasury = env.wallet(1).pubkey();
    let ix = propose(
        &w.pubkey(),
        0,
        &wrong_treasury,
        &propose_args("OKAY", 0, SOL),
    );
    assert_err!(env.send(&[ix], &[&w]), E::AccountMismatch);
    assert!(
        !env.exists(&candidate_for(0, &propose_args("OKAY", 0, SOL))),
        "nothing was created"
    );
    assert_eq!(
        env.load::<EventState>(&event_pda(0)).candidate_count,
        [0; 8]
    );
}

#[test]
fn propose_is_blocked_by_the_pause_flag_but_commit_is_not() {
    let mut env = setup();
    let w = env.wallet(100);
    let c = env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)).unwrap();
    env.send(
        &[set_pause(&env.admin.pubkey(), Some(true), None)],
        &[&env.admin.insecure_clone()],
    )
    .unwrap();
    assert_err!(
        env.do_propose(&w, 0, &propose_args("BBB", 0, SOL)),
        E::ProposalsPaused
    );
    env.do_commit(&w, 0, &c, SOL).unwrap(); // commits only pause with their own flag
}

#[test]
fn proposals_follow_the_window_to_the_second() {
    let mut env = Env::new();
    env.init_config_with(|_| {});
    // window opens at T0+100 and closes at T0+3600
    let mut a = default_event_args(T0, "KXNBA-W");
    a.window_open = T0 + 100;
    let ix = create_event(&env.admin.pubkey(), 0, a);
    env.send(&[ix], &[&env.admin.insecure_clone()]).unwrap();
    let w = env.wallet(100);
    env.warp_to(T0 + 99);
    assert_err!(
        env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)),
        E::WindowNotOpen
    );
    env.warp_to(T0 + 100);
    env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)).unwrap();
    env.warp_to(T0 + 3_599);
    env.do_propose(&w, 0, &propose_args("BBB", 0, SOL)).unwrap();
    env.warp_to(T0 + 3_600);
    assert_err!(
        env.do_propose(&w, 0, &propose_args("CCC", 0, SOL)),
        E::WindowClosed
    );
    env.warp_to(T0 + 3_601);
    assert_err!(
        env.do_propose(&w, 0, &propose_args("CCC", 0, SOL)),
        E::WindowClosed
    );
}

// ---- attestation (D-014) -----------------------------------------------------------------------------------------

fn attested_env() -> Env {
    let mut env = Env::new();
    env.init_config_with(|p| p.require_attestation = true);
    env.create_default_event("KXNBA-A");
    env
}

fn send_with(
    env: &mut Env,
    w: &solana_keypair::Keypair,
    pre: Vec<anchor_lang::solana_program::instruction::Instruction>,
    args: &prequel::ProposeArgs,
) -> Result<Sent, Failed> {
    let mut ixs = pre;
    ixs.push(propose(&w.pubkey(), 0, &env.treasury, args));
    env.send(&ixs, &[w])
}

#[test]
fn an_attested_proposal_succeeds() {
    let mut env = attested_env();
    let w = env.wallet(10);
    let args = propose_args("BOB", 0, SOL);
    let att = attestation_ix(&env.canonicalizer, 0, &args, &w.pubkey());
    send_with(&mut env, &w, vec![att], &args).unwrap();
    assert!(env.exists(&candidate_for(0, &args)));
}

#[test]
fn attestation_failures() {
    let mut env = attested_env();
    let w = env.wallet(100);
    let args = propose_args("BOB", 0, SOL);
    let good =
        |env: &Env, a: &prequel::ProposeArgs| attestation_ix(&env.canonicalizer, 0, a, &w.pubkey());

    // required but absent, or not the instruction right before
    assert_err!(
        send_with(&mut env, &w, vec![], &args),
        E::AttestationMissing
    );
    let filler =
        anchor_lang::solana_program::system_instruction::transfer(&w.pubkey(), &w.pubkey(), 1);
    let att = good(&env, &args);
    assert_err!(
        send_with(&mut env, &w, vec![att, filler], &args),
        E::AttestationMissing
    );

    // signed by the wrong key
    let imposter = env.wallet(1);
    let wrong_key = attestation_ix(&imposter, 0, &args, &w.pubkey());
    assert_err!(
        send_with(&mut env, &w, vec![wrong_key], &args),
        E::AttestationInvalid
    );

    // signed for different content, proposer, outcome or expiry than what is submitted
    let mut other = args.clone();
    other.name = "Different Name".into();
    let att = good(&env, &other);
    assert_err!(
        send_with(&mut env, &w, vec![att], &args),
        E::AttestationInvalid
    );
    let someone_else = env.wallet(1);
    let att = attestation_ix(&env.canonicalizer, 0, &args, &someone_else.pubkey());
    assert_err!(
        send_with(&mut env, &w, vec![att], &args),
        E::AttestationInvalid
    );
    let mut other = args.clone();
    other.image_hash = sha256(b"another image");
    let att = good(&env, &other);
    assert_err!(
        send_with(&mut env, &w, vec![att], &args),
        E::AttestationInvalid
    );
    let mut later = args.clone();
    later.attestation_expiry += 1;
    let att = good(&env, &args);
    assert_err!(
        send_with(&mut env, &w, vec![att], &later),
        E::AttestationInvalid
    );

    // a message for another event cannot be replayed here
    let att = attestation_ix(&env.canonicalizer, 7, &args, &w.pubkey());
    assert_err!(
        send_with(&mut env, &w, vec![att], &args),
        E::AttestationInvalid
    );

    // malformed precompile instructions: offsets pointing at other instructions, two signatures
    let msg = attestation_message(0, &args, &w.pubkey());
    let sig = env.canonicalizer.sign_message(&msg);
    for idx in [
        [0u16, u16::MAX, u16::MAX],
        [u16::MAX, 0, u16::MAX],
        [u16::MAX, u16::MAX, 0],
    ] {
        let raw = ed25519_ix_raw(env.canonicalizer.pubkey().as_ref(), sig.as_ref(), &msg, idx);
        let r = send_with(&mut env, &w, vec![raw], &args);
        assert!(r.is_err(), "index fields {idx:?} must not be accepted");
    }
    let mut two = ed25519_ix_raw(
        env.canonicalizer.pubkey().as_ref(),
        sig.as_ref(),
        &msg,
        [u16::MAX; 3],
    );
    two.data[0] = 2;
    assert!(send_with(&mut env, &w, vec![two], &args).is_err());

    // expired: valid yesterday, not today
    env.warp_to(args.attestation_expiry + 1);
    let att = good(&env, &args);
    assert_err!(
        send_with(&mut env, &w, vec![att], &args),
        E::AttestationExpired
    );

    // and nothing above created the candidate
    assert!(!env.exists(&candidate_for(0, &args)));
}

#[test]
fn a_tampered_signature_is_rejected_by_the_runtime() {
    let mut env = attested_env();
    let w = env.wallet(10);
    let args = propose_args("BOB", 0, SOL);
    let msg = attestation_message(0, &args, &w.pubkey());
    let mut sig = env.canonicalizer.sign_message(&msg).as_ref().to_vec();
    sig[0] ^= 0xff;
    let bad = ed25519_ix_raw(
        env.canonicalizer.pubkey().as_ref(),
        &sig,
        &msg,
        [u16::MAX; 3],
    );
    let f = send_with(&mut env, &w, vec![bad], &args).unwrap_err();
    println!("tampered signature: {}", f.debug);
    // the failure comes from the precompile (instruction 0), before our program (instruction 1) ever runs
    assert!(
        f.debug.contains("InstructionError(0,"),
        "unexpected failure: {f:?}"
    );
    assert!(
        f.code().is_none_or(|c| c < 6000),
        "must not be one of our errors: {f:?}"
    );
    assert!(!env.exists(&candidate_for(0, &args)));
}

// ---- commit -----------------------------------------------------------------------------------------------------

fn with_candidate() -> (Env, Pubkey) {
    let mut env = setup();
    let proposer = env.wallet(100);
    let c = env
        .do_propose(&proposer, 0, &propose_args("BOB", 0, SOL))
        .unwrap();
    (env, c)
}
use anchor_lang::prelude::Pubkey;

#[test]
fn commits_accumulate_per_wallet_and_count_each_wallet_once() {
    let (mut env, c) = with_candidate();
    let (a, b) = (env.wallet(100), env.wallet(100));
    env.do_commit(&a, 0, &c, 2 * SOL).unwrap();
    env.do_commit(&a, 0, &c, 3 * SOL).unwrap();
    env.do_commit(&b, 0, &c, SOL).unwrap();
    let cand: Candidate = env.load(&c);
    assert_eq!((cand.total_committed, cand.committer_count), (7 * SOL, 3));
    assert_eq!(
        env.load::<Commitment>(&commitment_pda(&c, &a.pubkey()))
            .amount,
        5 * SOL
    );
    assert_eq!(
        env.load::<Commitment>(&commitment_pda(&c, &b.pubkey()))
            .amount,
        SOL
    );
    let rent = env.rent_minimum(8 + Vault::INIT_SPACE);
    assert_eq!(env.lamports(&vault_pda(&c)), rent + 7 * SOL);
}

#[test]
fn commit_rejects_bad_amounts_pauses_and_non_pending_candidates() {
    let (mut env, c) = with_candidate();
    let w = env.wallet(200);
    assert_err!(
        env.do_commit(&w, 0, &c, SOL / 20 - 1),
        E::CommitBelowMinimum
    );
    env.do_commit(&w, 0, &c, 49 * SOL).unwrap();
    assert_err!(env.do_commit(&w, 0, &c, SOL + 1), E::CommitAboveCap); // 50 SOL cap on the running total
    env.do_commit(&w, 0, &c, SOL).unwrap(); // exactly the cap
    assert_err!(env.do_commit(&w, 0, &c, SOL / 20), E::CommitAboveCap);

    env.send(
        &[set_pause(&env.admin.pubkey(), None, Some(true))],
        &[&env.admin.insecure_clone()],
    )
    .unwrap();
    let other = env.wallet(10);
    assert_err!(env.do_commit(&other, 0, &c, SOL), E::CommitsPaused);
    env.send(
        &[set_pause(&env.admin.pubkey(), None, Some(false))],
        &[&env.admin.insecure_clone()],
    )
    .unwrap();
    env.do_commit(&other, 0, &c, SOL).unwrap();
}

#[test]
fn commits_follow_the_window_to_the_second_even_when_nobody_cranks_close() {
    let (mut env, c) = with_candidate();
    let w = env.wallet(100);
    env.warp_to(T0 + 3_599);
    env.do_commit(&w, 0, &c, SOL).unwrap();
    env.warp_to(T0 + 3_600); // stored status is still Open: only the clock says closed
    assert_eq!(
        env.load::<EventState>(&event_pda(0)).status,
        prequel::EventStatus::Open
    );
    assert_err!(env.do_commit(&w, 0, &c, SOL), E::WindowClosed);
    env.warp_to(T0 + 10_000);
    assert_err!(env.do_commit(&w, 0, &c, SOL), E::WindowClosed);
}

#[test]
fn a_commit_to_a_candidate_of_another_event_is_refused() {
    let (mut env, c) = with_candidate();
    let (id1, _) = env.create_default_event("KXNBA-2");
    let w = env.wallet(10);
    // the right candidate, but presented under event 1: the has_one / seeds constraints refuse it
    let ix = commit(&w.pubkey(), id1, &c, SOL);
    assert!(env.send(&[ix], &[&w]).is_err());
}

// ---- co_propose -------------------------------------------------------------------------------------------------

#[test]
fn co_propose_pays_the_fee_and_freezes_the_weight() {
    let (mut env, c) = with_candidate();
    let w = env.wallet(100);
    let treasury_before = env.lamports(&env.treasury);
    env.do_co_propose(&w, 0, &c, 2 * SOL).unwrap();
    assert_eq!(
        env.lamports(&env.treasury) - treasury_before,
        SOL / 50,
        "same non-refundable fee as a proposal"
    );
    let share: CoProposerShare = env.load(&coproposer_pda(&c, &w.pubkey()));
    assert_eq!(
        (share.weight, share.claimed, share.joined_at),
        (2 * SOL, 0, T0)
    );
    let cand: Candidate = env.load(&c);
    assert_eq!(
        (
            cand.co_proposer_total,
            cand.co_proposer_count,
            cand.total_committed,
            cand.committer_count
        ),
        (2 * SOL, 1, 3 * SOL, 2)
    );

    // later commits raise the commitment but not the reward weight
    env.do_commit(&w, 0, &c, 4 * SOL).unwrap();
    assert_eq!(
        env.load::<CoProposerShare>(&coproposer_pda(&c, &w.pubkey()))
            .weight,
        2 * SOL
    );
    assert_eq!(
        env.load::<Commitment>(&commitment_pda(&c, &w.pubkey()))
            .amount,
        6 * SOL
    );
    assert_eq!(env.load::<Candidate>(&c).co_proposer_total, 2 * SOL);
}

#[test]
fn a_wallet_that_already_committed_can_still_co_propose() {
    let (mut env, c) = with_candidate();
    let w = env.wallet(100);
    env.do_commit(&w, 0, &c, 3 * SOL).unwrap();
    env.do_co_propose(&w, 0, &c, SOL).unwrap();
    assert_eq!(
        env.load::<Commitment>(&commitment_pda(&c, &w.pubkey()))
            .amount,
        4 * SOL
    );
    assert_eq!(
        env.load::<CoProposerShare>(&coproposer_pda(&c, &w.pubkey()))
            .weight,
        SOL,
        "weight is only the co-propose amount"
    );
    let cand: Candidate = env.load(&c);
    assert_eq!(cand.committer_count, 2, "the wallet is counted once");
}

#[test]
fn co_propose_failures() {
    let (mut env, c) = with_candidate();
    let w = env.wallet(100);
    // the proposer is already in
    let proposer_key = env.load::<Candidate>(&c).proposer;
    assert_eq!(proposer_key, env.load::<Candidate>(&c).proposer);
    // bad amounts
    assert_err!(
        env.do_co_propose(&w, 0, &c, SOL / 20 - 1),
        E::CommitBelowMinimum
    );
    assert_err!(env.do_co_propose(&w, 0, &c, 51 * SOL), E::CommitAboveCap);
    // both pause flags stop it
    for (p, m, e) in [
        (Some(true), None, E::ProposalsPaused),
        (Some(false), Some(true), E::CommitsPaused),
    ] {
        env.send(
            &[set_pause(&env.admin.pubkey(), p, m)],
            &[&env.admin.insecure_clone()],
        )
        .unwrap();
        assert_err!(env.do_co_propose(&w, 0, &c, SOL), e);
    }
    env.send(
        &[set_pause(&env.admin.pubkey(), Some(false), Some(false))],
        &[&env.admin.insecure_clone()],
    )
    .unwrap();
    // works once, never twice
    env.do_co_propose(&w, 0, &c, SOL).unwrap();
    let err = env.do_co_propose(&w, 0, &c, SOL).unwrap_err();
    assert!(has_log(&err, "already in use"), "{err:?}");
    // closed window
    let late = env.wallet(10);
    env.warp_to(T0 + 3_600);
    assert_err!(env.do_co_propose(&late, 0, &c, SOL), E::WindowClosed);
    assert_eq!(
        env.load::<Candidate>(&c).co_proposer_count,
        1,
        "failed attempts changed nothing"
    );
}

#[test]
fn the_proposer_cannot_co_propose_their_own_candidate() {
    let mut env = setup();
    let proposer = env.wallet(100);
    let c = env
        .do_propose(&proposer, 0, &propose_args("BOB", 0, SOL))
        .unwrap();
    assert_err!(
        env.do_co_propose(&proposer, 0, &c, SOL),
        E::ProposerCannotCoPropose
    );
}

// ---- conservation -----------------------------------------------------------------------------------------------

#[test]
fn the_vault_always_holds_its_rent_plus_exactly_the_committed_total() {
    let (mut env, c) = with_candidate();
    let rent = env.rent_minimum(8 + Vault::INIT_SPACE);
    let wallets: Vec<_> = (0..5).map(|_| env.wallet(100)).collect();
    let mut expected_commitments = vec![SOL]; // the proposer's
    for (i, w) in wallets.iter().enumerate() {
        env.do_commit(w, 0, &c, (i as u64 + 1) * SOL / 2).unwrap();
        let co = env.wallet(100);
        env.do_co_propose(&co, 0, &c, SOL).unwrap();
        expected_commitments.push((i as u64 + 1) * SOL / 2);
        expected_commitments.push(SOL);
        let cand: Candidate = env.load(&c);
        assert_eq!(
            cand.total_committed,
            expected_commitments.iter().sum::<u64>()
        );
        assert_eq!(
            env.lamports(&vault_pda(&c)),
            rent + cand.total_committed,
            "invariant 1: vault matches the books"
        );
    }
    let _ = Params::spec_defaults();
}
