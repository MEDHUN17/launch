//! Tests added after the independent design review: account binding, the instructions sysvar, Ed25519
//! variants, packet size, pause matrix, caps across calls, pre-funded addresses, events.
mod common;

use anchor_lang::{prelude::Pubkey, Space};
use common::ix::*;
use common::*;
use prequel::{error::PrequelError as E, events, Candidate, Commitment, EventState, Vault};
use solana_keypair::Keypair;
use solana_signer::Signer;

fn admin(env: &Env) -> Keypair {
    env.admin.insecure_clone()
}

fn setup(attest: bool) -> Env {
    let mut env = Env::new();
    env.init_config_with(|p| p.require_attestation = attest);
    env.create_default_event("KXNBA-1");
    env
}

// ---- account binding (design review H1) ---------------------------------------------------------------------------

#[test]
fn swapping_any_account_in_commit_or_co_propose_is_refused_and_moves_no_money() {
    let mut env = setup(false);
    let (id1, _) = env.create_default_event("KXNBA-2");
    let proposer = env.wallet(100);
    let c_a = env
        .do_propose(&proposer, 0, &propose_args("AAA", 0, SOL))
        .unwrap();
    let c_b = env
        .do_propose(&proposer, id1, &propose_args("BBB", 0, SOL))
        .unwrap();
    let buyer = env.wallet(50);
    let stranger_commitment = commitment_pda(&c_a, &proposer.pubkey());
    let vault_b_before = env.lamports(&vault_pda(&c_b));
    let vault_a_before = env.lamports(&vault_pda(&c_a));

    // commit: wallet, config, event, candidate, vault, commitment, system_program
    let substitutions: [(usize, Pubkey); 7] = [
        (1, event_pda(0)),    // config replaced by an event
        (2, event_pda(id1)),  // the other event
        (3, c_b),             // another event's candidate
        (4, vault_pda(&c_b)), // another candidate's vault
        (4, config_pda()),
        (5, stranger_commitment), // someone else's commitment
        (5, commitment_pda(&c_b, &buyer.pubkey())), // own commitment, wrong candidate
    ];
    for (index, replacement) in substitutions {
        let mut ix = commit(&buyer.pubkey(), 0, &c_a, SOL);
        ix.accounts[index].pubkey = replacement;
        assert!(
            env.send(&[ix], &[&buyer]).is_err(),
            "commit with account #{index} swapped to {replacement} must fail"
        );
    }
    // co_propose: wallet, config, event, candidate, vault, commitment, co_proposer_share, treasury, system_program
    let stranger = env.wallet(1).pubkey();
    let share_of_other = coproposer_pda(&c_a, &proposer.pubkey());
    for (index, replacement) in [
        (2usize, event_pda(id1)),
        (3, c_b),
        (4, vault_pda(&c_b)),
        (5, stranger_commitment),
        (6, share_of_other),
        (7, stranger),
    ] {
        let mut ix = co_propose(&buyer.pubkey(), 0, &c_a, &env.treasury, SOL);
        ix.accounts[index].pubkey = replacement;
        assert!(
            env.send(&[ix], &[&buyer]).is_err(),
            "co_propose with account #{index} swapped must fail"
        );
    }
    // propose: proposer, config, event, candidate, vault, commitment, treasury, instructions, system_program
    let mut ix = propose(
        &buyer.pubkey(),
        0,
        &env.treasury,
        &propose_args("CCC", 0, SOL),
    );
    ix.accounts[6].pubkey = stranger;
    assert_err!(env.send(&[ix], &[&buyer]), E::AccountMismatch);

    assert_eq!(env.lamports(&vault_pda(&c_a)), vault_a_before);
    assert_eq!(env.lamports(&vault_pda(&c_b)), vault_b_before);
    assert_eq!(env.load::<Candidate>(&c_a).total_committed, SOL);
    assert_eq!(env.load::<Candidate>(&c_b).total_committed, SOL);
    assert!(!env.exists(&commitment_pda(&c_a, &buyer.pubkey())));
}

// ---- the instructions sysvar (H2) and Ed25519 variants ------------------------------------------------------------

fn send_with(
    env: &mut Env,
    w: &Keypair,
    pre: Vec<anchor_lang::solana_program::instruction::Instruction>,
    args: &prequel::ProposeArgs,
) -> Result<Sent, Failed> {
    let mut ixs = pre;
    ixs.push(propose(&w.pubkey(), 0, &env.treasury, args));
    env.send(&ixs, &[w])
}

#[test]
fn a_fake_instructions_sysvar_is_refused_even_when_attestation_is_off() {
    for attest in [false, true] {
        let mut env = setup(attest);
        let w = env.wallet(10);
        let args = propose_args("BOB", 0, SOL);
        let mut ix = propose(&w.pubkey(), 0, &env.treasury, &args);
        ix.accounts[7].pubkey = Pubkey::new_unique();
        assert_err!(env.send(&[ix], &[&w]), E::AccountMismatch);
    }
}

#[test]
fn ed25519_variants_that_must_not_pass() {
    let mut env = setup(true);
    let w = env.wallet(100);
    let args = propose_args("BOB", 0, SOL);
    let digest = attestation_message(0, &args, &w.pubkey());
    let key = env.canonicalizer.pubkey();
    let canonicalizer = env.canonicalizer.insecure_clone();
    let signed = |msg: &[u8]| canonicalizer.sign_message(msg);

    // count 0 in a 2-byte instruction: the precompile accepts it, we must not
    let empty = anchor_lang::solana_program::instruction::Instruction {
        program_id: ED25519_PROGRAM,
        accounts: vec![],
        data: vec![0, 0],
    };
    assert_err!(
        send_with(&mut env, &w, vec![empty], &args),
        E::AttestationInvalid
    );

    // the right signer, a message one byte longer or shorter than the digest
    for tweak in [1i32, -1] {
        let msg: Vec<u8> = if tweak > 0 {
            [digest.as_slice(), &[0u8]].concat()
        } else {
            digest[..digest.len() - 1].to_vec()
        };
        let ix = ed25519_ix_raw(key.as_ref(), signed(&msg).as_ref(), &msg, [u16::MAX; 3]);
        assert_err!(
            send_with(&mut env, &w, vec![ix], &args),
            E::AttestationInvalid
        );
    }

    // a signature over the digest of a different outcome, event or proposer
    let mut other_outcome = args.clone();
    other_outcome.outcome = 1;
    let ix = attestation_ix(&env.canonicalizer, 0, &other_outcome, &w.pubkey());
    assert_err!(
        send_with(&mut env, &w, vec![ix], &args),
        E::AttestationInvalid
    );

    // ordering: a compute-budget instruction before the Ed25519 one is fine, after it is not
    let att = attestation_ix(&env.canonicalizer, 0, &args, &w.pubkey());
    let mut after = vec![att.clone()];
    after.extend(compute_budget_ixs());
    assert_err!(send_with(&mut env, &w, after, &args), E::AttestationMissing);
    // the Ed25519 instruction after the proposal
    let propose_first = [propose(&w.pubkey(), 0, &env.treasury, &args), att.clone()];
    assert_err!(env.send(&propose_first, &[&w]), E::AttestationMissing);

    // one attestation cannot serve two proposals in the same transaction
    let other = propose_args("CAT", 0, SOL);
    let two = [
        att.clone(),
        propose(&w.pubkey(), 0, &env.treasury, &args),
        propose(&w.pubkey(), 0, &env.treasury, &other),
    ];
    assert_err!(env.send(&two, &[&w]), E::AttestationMissing);
    let replay = [
        att.clone(),
        propose(&w.pubkey(), 0, &env.treasury, &args),
        att.clone(),
        propose(&w.pubkey(), 0, &env.treasury, &other),
    ];
    assert_err!(env.send(&replay, &[&w]), E::AttestationInvalid);
    assert!(
        !env.exists(&candidate_for(0, &args)) && !env.exists(&candidate_for(0, &other)),
        "failed transactions leave nothing"
    );

    // and the legitimate order works
    let mut good = compute_budget_ixs();
    good.push(att);
    send_with(&mut env, &w, good, &args).unwrap();
}

#[test]
fn attestation_expiry_boundary_and_key_rotation() {
    let mut env = setup(true);
    let w = env.wallet(100);
    let mut args = propose_args("BOB", 0, SOL);
    args.attestation_expiry = T0 + 100;
    let ok_at = |env: &mut Env, now: i64, args: &prequel::ProposeArgs, w: &Keypair| {
        env.warp_to(now);
        let att = attestation_ix(&env.canonicalizer.insecure_clone(), 0, args, &w.pubkey());
        send_with(env, w, vec![att], args)
    };
    assert_err!(ok_at(&mut env, T0 + 101, &args, &w), E::AttestationExpired); // T+1
    ok_at(&mut env, T0 + 100, &args, &w).unwrap(); // T: still valid
                                                   // a second candidate signed with the new key after a rotation; the old key no longer works
    let new_key = Keypair::new();
    let ix = update_config(
        &env.admin.pubkey(),
        prequel::UpdateConfigArgs {
            admin: None,
            resolver: None,
            canonicalizer: Some(new_key.pubkey()),
            treasury: None,
            params: None,
        },
    );
    env.send(&[ix], &[&admin(&env)]).unwrap();
    let mut second = propose_args("CAT", 0, SOL);
    second.attestation_expiry = T0 + 200;
    let old = attestation_ix(&env.canonicalizer, 0, &second, &w.pubkey());
    assert_err!(
        send_with(&mut env, &w, vec![old], &second),
        E::AttestationInvalid
    );
    let new = attestation_ix(&new_key, 0, &second, &w.pubkey());
    send_with(&mut env, &w, vec![new], &second).unwrap();
}

// ---- packet size (H3) ---------------------------------------------------------------------------------------------

#[test]
fn a_maximum_length_attested_proposal_fits_in_one_packet_with_a_priority_fee() {
    let mut env = setup(true);
    let w = env.wallet(100);
    let mut args = propose_args("ABCDEFGHIJ", 0, SOL); // 10-character ticker
    args.name = "n".repeat(32);
    args.uri = format!("ipfs://{}", "b".repeat(128 - 7));
    assert_eq!(
        (args.name.len(), args.ticker.len(), args.uri.len()),
        (32, 10, 128)
    );
    let mut ixs = compute_budget_ixs();
    ixs.push(attestation_ix(&env.canonicalizer, 0, &args, &w.pubkey()));
    ixs.push(propose(&w.pubkey(), 0, &env.treasury, &args));
    let size = tx_size(&env, &ixs, &w.pubkey(), 1);
    println!("PACKET maximum-length attested proposal with 2 compute-budget instructions: {size} bytes (limit 1232)");
    assert!(
        size <= 1232 - 100,
        "{size} bytes leaves no room for a wallet to add instructions"
    );
    env.send(&ixs, &[&w]).unwrap();
}

// ---- pauses ---------------------------------------------------------------------------------------------------------

#[test]
fn propose_is_stopped_by_either_pause_flag() {
    let mut env = setup(false);
    let w = env.wallet(100);
    env.send(
        &[set_pause(&env.admin.pubkey(), None, Some(true))],
        &[&admin(&env)],
    )
    .unwrap();
    assert_err!(
        env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)),
        E::CommitsPaused
    );
    env.send(
        &[set_pause(&env.admin.pubkey(), Some(true), Some(false))],
        &[&admin(&env)],
    )
    .unwrap();
    assert_err!(
        env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)),
        E::ProposalsPaused
    );
    env.send(
        &[set_pause(&env.admin.pubkey(), Some(false), None)],
        &[&admin(&env)],
    )
    .unwrap();
    env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)).unwrap();
}

#[test]
fn a_proposal_cutoff_stops_proposals_but_not_commits() {
    let mut env = Env::new();
    env.init_config_with(|p| p.proposal_cutoff = 900);
    env.create_default_event("KXNBA-1"); // window closes at T0 + 3600, so proposals stop at T0 + 2700
    let w = env.wallet(100);
    env.warp_to(T0 + 2_699);
    let c = env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)).unwrap();
    env.warp_to(T0 + 2_700);
    assert_err!(
        env.do_propose(&w, 0, &propose_args("BBB", 0, SOL)),
        E::ProposalCutoffPassed
    );
    env.do_commit(&w, 0, &c, SOL).unwrap();
    let late = env.wallet(10);
    env.do_co_propose(&late, 0, &c, SOL).unwrap();
}

// ---- per-wallet cap across calls --------------------------------------------------------------------------------

#[test]
fn the_per_wallet_cap_counts_propose_commit_and_co_propose_together() {
    let mut env = setup(false);
    let a = env.wallet(200);
    let c = env
        .do_propose(&a, 0, &propose_args("AAA", 0, 30 * SOL))
        .unwrap();
    env.do_commit(&a, 0, &c, 20 * SOL).unwrap(); // 50 SOL: exactly the cap
    assert_err!(env.do_commit(&a, 0, &c, SOL / 20), E::CommitAboveCap);
    let b = env.wallet(200);
    env.do_commit(&b, 0, &c, 40 * SOL).unwrap();
    assert_err!(
        env.do_co_propose(&b, 0, &c, 10 * SOL + 1),
        E::CommitAboveCap
    );
    env.do_co_propose(&b, 0, &c, 10 * SOL).unwrap();
    assert_eq!(
        env.load::<Commitment>(&commitment_pda(&c, &b.pubkey()))
            .amount,
        50 * SOL
    );
    assert_eq!(env.load::<Candidate>(&c).total_committed, 100 * SOL);
}

// ---- pre-funded addresses -----------------------------------------------------------------------------------------

#[test]
fn pre_funding_every_address_the_program_creates_does_not_block_it() {
    let mut env = Env::new();
    let attacker = env.wallet(50);
    let rent0 = env.rent_minimum(0);
    // Config, EventState #0 and the ExternalListing are known before anything exists
    prefund(&mut env, &attacker, &config_pda(), rent0);
    env.init_config_with(|_| {});
    let args = default_event_args(env.now(), "KXNBA-PRE");
    prefund(&mut env, &attacker, &event_pda(0), rent0);
    prefund(
        &mut env,
        &attacker,
        &external_pda(&args.external_id_hash),
        5 * SOL,
    );
    env.send(
        &[create_event(&env.admin.pubkey(), 0, args)],
        &[&admin(&env)],
    )
    .unwrap();

    // candidate, vault, commitment and co-proposer share addresses are derivable from public data
    let w = env.wallet(100);
    let p = propose_args("BOB", 0, SOL);
    let candidate = candidate_for(0, &p);
    for target in [
        candidate,
        vault_pda(&candidate),
        commitment_pda(&candidate, &w.pubkey()),
    ] {
        prefund(&mut env, &attacker, &target, rent0 + 1);
    }
    env.do_propose(&w, 0, &p).unwrap();
    let co = env.wallet(100);
    for target in [
        coproposer_pda(&candidate, &co.pubkey()),
        commitment_pda(&candidate, &co.pubkey()),
    ] {
        prefund(&mut env, &attacker, &target, 2 * SOL);
    }
    env.do_co_propose(&co, 0, &candidate, SOL).unwrap();
    // the surplus is simply surplus: accounting still comes from state
    let cand: Candidate = env.load(&candidate);
    assert_eq!(cand.total_committed, 2 * SOL);
    assert!(
        env.lamports(&vault_pda(&candidate)) >= env.rent_minimum(8 + Vault::INIT_SPACE) + 2 * SOL
    );
}

#[test]
fn initialize_config_refuses_the_wrong_program_accounts() {
    let mut env = Env::new();
    let args = default_init_args(&env);
    let deployer = env.deployer.insecure_clone();
    let mut run = |index: usize, replacement: Pubkey| {
        let mut ix = initialize_config(&deployer.pubkey(), args.clone());
        ix.accounts[index].pubkey = replacement;
        assert!(
            env.send(&[ix], &[&deployer]).is_err(),
            "account #{index} replaced"
        );
    };
    // deployer, config, program, program_data, system_program
    run(2, Pubkey::new_unique());
    run(3, Pubkey::new_unique());
    run(3, config_pda());
    assert!(!env.exists(&config_pda()));
}

// ---- events -------------------------------------------------------------------------------------------------------------

#[test]
fn each_instruction_logs_the_events_the_indexer_relies_on() {
    let mut env = Env::new();
    let mut init = default_init_args(&env);
    init.params.require_attestation = false;
    let sent = env
        .send(
            &[initialize_config(&env.deployer.pubkey(), init)],
            &[&env.deployer.insecure_clone()],
        )
        .unwrap();
    assert_eq!(event_count(&sent.logs), 1);
    assert_eq!(
        events_of::<events::ConfigUpdated>(&sent.logs)[0].admin,
        env.admin.pubkey()
    );

    let args = default_event_args(env.now(), "KXNBA-EV");
    let sent = env
        .send(
            &[create_event(&env.admin.pubkey(), 0, args.clone())],
            &[&admin(&env)],
        )
        .unwrap();
    assert_eq!(event_count(&sent.logs), 1);
    let created = &events_of::<events::EventCreated>(&sent.logs)[0];
    assert_eq!(
        (
            created.id,
            created.external_id.as_str(),
            created.outcome_count
        ),
        (0, "KXNBA-EV", 2)
    );

    let sent = env
        .send(
            &[set_pause(&env.admin.pubkey(), Some(true), None)],
            &[&admin(&env)],
        )
        .unwrap();
    let paused = &events_of::<events::PauseChanged>(&sent.logs)[0];
    assert!(paused.pause_proposals && !paused.pause_commits);
    env.send(
        &[set_pause(&env.admin.pubkey(), Some(false), None)],
        &[&admin(&env)],
    )
    .unwrap();

    let w = env.wallet(100);
    let p = propose_args("BOB", 0, SOL);
    let sent = env
        .send(&[propose(&w.pubkey(), 0, &env.treasury, &p)], &[&w])
        .unwrap();
    assert_eq!(
        event_count(&sent.logs),
        2,
        "CandidateProposed and Committed"
    );
    let proposed = &events_of::<events::CandidateProposed>(&sent.logs)[0];
    assert_eq!(
        (proposed.ticker.as_str(), proposed.amount, proposed.proposer),
        ("BOB", SOL, w.pubkey())
    );
    let committed = &events_of::<events::Committed>(&sent.logs)[0];
    assert_eq!(
        (
            committed.total_committed,
            committed.committer_count,
            committed.wallet_total
        ),
        (SOL, 1, SOL)
    );

    let candidate = candidate_for(0, &p);
    let buyer = env.wallet(10);
    let sent = env
        .send(
            &[commit(&buyer.pubkey(), 0, &candidate, 2 * SOL)],
            &[&buyer],
        )
        .unwrap();
    assert_eq!(event_count(&sent.logs), 1);
    let committed = &events_of::<events::Committed>(&sent.logs)[0];
    assert_eq!(
        (committed.total_committed, committed.committer_count),
        (3 * SOL, 2)
    );

    let co = env.wallet(10);
    let sent = env
        .send(
            &[co_propose(&co.pubkey(), 0, &candidate, &env.treasury, SOL)],
            &[&co],
        )
        .unwrap();
    assert_eq!(event_count(&sent.logs), 2, "CoProposed and Committed");
    assert_eq!(events_of::<events::CoProposed>(&sent.logs)[0].weight, SOL);
    assert_eq!(env.load::<EventState>(&event_pda(0)).candidate_count[0], 1);
}
