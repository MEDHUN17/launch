//! Tests for close_window, void_event, reject_candidate and refund (invariants 2, 4, 5 and 6 on the refund side).
mod common;

use anchor_lang::{prelude::Pubkey, Space};
use common::ix::*;
use common::*;
use prequel::{
    error::PrequelError as E, events, Candidate, CandidateStatus, EventState, EventStatus, Vault,
};
use solana_keypair::Keypair;
use solana_signer::Signer;

fn setup() -> Env {
    let mut env = Env::new();
    env.init_config_with(|_| {});
    env.create_default_event("KXNBA-1");
    env
}

fn vault_rent() -> usize {
    8 + Vault::INIT_SPACE
}

// ---- close_window ---------------------------------------------------------------------------------------------------

#[test]
fn close_window_follows_the_clock_and_a_repeat_fails_with_a_named_error() {
    let mut env = setup();
    env.warp_to(T0 + 3_599);
    assert_err!(env.do_close(0), E::WindowNotClosedYet);
    env.warp_to(T0 + 3_600);
    let sent = env.do_close(0).unwrap(); // anyone can crank it
    assert_eq!(
        events_of::<events::WindowClosed>(&sent.logs)[0].closed_at,
        T0 + 3_600
    );
    assert_eq!(
        env.load::<EventState>(&event_pda(0)).status,
        EventStatus::Closed
    );
    assert_err!(env.do_close(0), E::EventStateInvalid);
}

#[test]
fn a_voided_event_cannot_be_closed() {
    let mut env = setup();
    env.do_void(0).unwrap();
    env.warp_to(T0 + 4_000);
    assert_err!(env.do_close(0), E::EventStateInvalid);
}

// ---- void_event -------------------------------------------------------------------------------------------------------

#[test]
fn void_event_is_admin_only_works_from_open_and_closed_and_blocks_new_money() {
    let mut env = setup();
    let stranger = env.wallet(10);
    assert_err!(
        env.send(&[void_event(&stranger.pubkey(), 0)], &[&stranger]),
        E::NotAdmin
    );

    let w = env.wallet(100);
    let c = env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)).unwrap();
    env.warp_to(T0 + 3_600);
    env.do_close(0).unwrap();
    let sent = env.do_void(0).unwrap(); // from Closed
    assert_eq!(
        events_of::<events::EventVoided>(&sent.logs)[0].by,
        env.admin.pubkey()
    );
    let e: EventState = env.load(&event_pda(0));
    assert_eq!(
        (e.status, e.voided_at, e.void_reason),
        (EventStatus::Voided, T0 + 3_600, 1)
    );
    assert_err!(env.do_void(0), E::EventStateInvalid);

    // nothing can be added to a voided event
    assert_err!(env.do_commit(&w, 0, &c, SOL), E::EventStateInvalid);
    let late = env.wallet(10);
    assert_err!(env.do_co_propose(&late, 0, &c, SOL), E::EventStateInvalid);
    assert_err!(
        env.do_propose(&w, 0, &propose_args("BBB", 0, SOL)),
        E::EventStateInvalid
    );
}

#[test]
fn an_open_event_can_be_voided_too() {
    let mut env = setup();
    env.do_void(0).unwrap();
    assert_eq!(
        env.load::<EventState>(&event_pda(0)).status,
        EventStatus::Voided
    );
}

// ---- refund after a void (invariants 2 and 4) --------------------------------------------------------------------------

struct Scene {
    env: Env,
    candidate: Pubkey,
    creation_balance: u64,
    wallets: Vec<(Keypair, u64)>, // wallet and total committed
}

/// Candidate with a proposer (1 SOL), two plain committers (one commits twice) and one co-proposer, event voided.
fn voided_scene() -> Scene {
    let mut env = setup();
    let proposer = env.wallet(100);
    let c = env
        .do_propose(&proposer, 0, &propose_args("BOB", 0, SOL))
        .unwrap();
    let creation_balance = env.lamports(&vault_pda(&c)) - SOL;
    let (a, b, co) = (env.wallet(100), env.wallet(100), env.wallet(100));
    env.do_commit(&a, 0, &c, 2 * SOL).unwrap();
    env.do_commit(&a, 0, &c, 3 * SOL).unwrap();
    env.do_commit(&b, 0, &c, 4 * SOL).unwrap();
    env.do_co_propose(&co, 0, &c, 5 * SOL).unwrap();
    env.do_void(0).unwrap();
    Scene {
        env,
        candidate: c,
        creation_balance,
        wallets: vec![(proposer, SOL), (a, 5 * SOL), (b, 4 * SOL), (co, 5 * SOL)],
    }
}

#[test]
fn a_voided_event_refunds_every_commit_exactly_and_the_vault_ends_at_its_creation_balance() {
    let mut s = voided_scene();
    let total: u64 = s.wallets.iter().map(|(_, a)| a).sum();
    assert_eq!(s.env.load::<Candidate>(&s.candidate).total_committed, total);
    assert_eq!(
        s.env.lamports(&vault_pda(&s.candidate)),
        s.creation_balance + total
    );

    let mut paid = 0;
    for (i, (wallet, amount)) in s.wallets.iter().enumerate() {
        let vault_before = s.env.lamports(&vault_pda(&s.candidate));
        let sent = s.env.do_refund(wallet, 0, &s.candidate).unwrap();
        let vault_after = s.env.lamports(&vault_pda(&s.candidate));
        assert_eq!(
            vault_before - vault_after,
            *amount,
            "wallet {i} gets exactly its commitment, no fee"
        );
        let ev = &events_of::<events::Refunded>(&sent.logs)[0];
        assert_eq!((ev.amount, ev.wallet), (*amount, wallet.pubkey()));
        assert_eq!(
            ev.candidate_status,
            CandidateStatus::Refunding,
            "the first refund flips Pending to Refunding"
        );
        paid += amount;
        assert_eq!(ev.refunded_total, paid);
        // the commitment (and the co-proposer share) are gone: a second refund is impossible
        assert!(!s
            .env
            .exists(&commitment_pda(&s.candidate, &wallet.pubkey())));
        assert!(!s
            .env
            .exists(&coproposer_pda(&s.candidate, &wallet.pubkey())));
        let again = s.env.do_refund(wallet, 0, &s.candidate);
        assert!(again.is_err(), "wallet {i} cannot refund twice");
        assert_eq!(
            s.env.lamports(&vault_pda(&s.candidate)),
            vault_after,
            "the failed second refund moved nothing"
        );
    }
    let c: Candidate = s.env.load(&s.candidate);
    assert_eq!(
        (c.refunded_total, c.status),
        (total, CandidateStatus::Refunding)
    );
    assert_eq!(
        s.env.lamports(&vault_pda(&s.candidate)),
        s.creation_balance,
        "invariant 2: only the vault's own balance is left"
    );
}

#[test]
fn a_dust_donation_to_the_vault_cannot_block_the_last_refund() {
    let mut s = voided_scene();
    let donor = s.env.wallet(10);
    let before = s.env.lamports(&vault_pda(&s.candidate));
    // 1 lamport: harmless on a program-owned vault (it would strand the last refund on a system-owned one)
    let ix = anchor_lang::solana_program::system_instruction::transfer(
        &donor.pubkey(),
        &vault_pda(&s.candidate),
        1,
    );
    s.env.send(&[ix], &[&donor]).unwrap();
    assert_eq!(s.env.lamports(&vault_pda(&s.candidate)), before + 1);
    let wallets: Vec<_> = s.wallets.iter().map(|(w, _)| w.insecure_clone()).collect();
    for w in &wallets {
        s.env.do_refund(w, 0, &s.candidate).unwrap();
    }
    assert_eq!(
        s.env.lamports(&vault_pda(&s.candidate)),
        s.creation_balance + 1,
        "the donation is simply left over"
    );
}

#[test]
fn refunds_still_work_after_the_rent_rate_falls() {
    let mut s = voided_scene();
    let mut rent = s.env.svm.get_sysvar::<solana_rent::Rent>();
    rent.lamports_per_byte /= 2; // SIMD-0437 lowers rent in steps; vaults created earlier hold more than the new minimum
    s.env.svm.set_sysvar(&rent);
    let wallets: Vec<_> = s.wallets.iter().map(|(w, _)| w.insecure_clone()).collect();
    for w in &wallets {
        s.env.do_refund(w, 0, &s.candidate).unwrap();
    }
    assert_eq!(s.env.lamports(&vault_pda(&s.candidate)), s.creation_balance);
    let _ = vault_rent();
}

#[test]
fn only_the_committer_can_refund_and_nobody_can_use_another_account() {
    let mut s = voided_scene();
    let stranger = s.env.wallet(10);
    assert_account_rejected(
        s.env.send(
            &[refund(&stranger.pubkey(), 0, &s.candidate, false)],
            &[&stranger],
        ),
        "a wallet with no commitment",
    );
    // presenting someone else's commitment fails the seeds check
    let victim = s.wallets[1].0.pubkey();
    let mut ix = refund(&stranger.pubkey(), 0, &s.candidate, false);
    ix.accounts[4].pubkey = commitment_pda(&s.candidate, &victim);
    assert_account_rejected(s.env.send(&[ix], &[&stranger]), "someone else's commitment");
    // an unrelated vault is rejected
    let other = s.env.wallet(100);
    let (id1, _) = s.env.create_default_event("KXNBA-OTHER");
    let c2 = s
        .env
        .do_propose(&other, id1, &propose_args("ZZZ", 0, SOL))
        .unwrap();
    let mut ix = refund(&s.wallets[0].0.pubkey(), 0, &s.candidate, false);
    ix.accounts[3].pubkey = vault_pda(&c2);
    assert_account_rejected(
        s.env.send(&[ix], &[&s.wallets[0].0.insecure_clone()]),
        "another candidate's vault",
    );
    assert_eq!(s.env.load::<Candidate>(&s.candidate).refunded_total, 0);
}

#[test]
fn a_pending_candidate_is_not_refundable_while_its_event_is_live() {
    let mut env = setup();
    let w = env.wallet(100);
    let c = env.do_propose(&w, 0, &propose_args("BOB", 0, SOL)).unwrap();
    assert_err!(env.do_refund(&w, 0, &c), E::NotRefundable); // open
    env.warp_to(T0 + 3_600);
    assert_err!(env.do_refund(&w, 0, &c), E::NotRefundable); // closed by the clock, stored status still Open
    env.do_close(0).unwrap();
    assert_err!(env.do_refund(&w, 0, &c), E::NotRefundable); // closed and awaiting resolution (Phase 2)
}

// ---- reject_candidate ---------------------------------------------------------------------------------------------------

#[test]
fn rejecting_a_candidate_frees_the_slot_blocks_new_money_and_refunds_everyone() {
    let mut env = Env::new();
    env.init_config_with(|p| p.max_candidates_per_outcome = 2);
    env.create_default_event("KXNBA-1");
    let (p1, p2, a, b) = (
        env.wallet(100),
        env.wallet(100),
        env.wallet(100),
        env.wallet(100),
    );
    let c1 = env
        .do_propose(&p1, 0, &propose_args("AAA", 0, SOL))
        .unwrap();
    env.do_propose(&p2, 0, &propose_args("BBB", 0, SOL))
        .unwrap();
    env.do_commit(&a, 0, &c1, 2 * SOL).unwrap();
    env.do_co_propose(&b, 0, &c1, 3 * SOL).unwrap();
    assert_err!(
        env.do_propose(&p1, 0, &propose_args("CCC", 0, SOL)),
        E::SlateFull
    );

    let stranger = env.wallet(10);
    assert_err!(
        env.send(
            &[reject_candidate(&stranger.pubkey(), 0, &c1)],
            &[&stranger]
        ),
        E::NotAdmin
    );
    let sent = env.do_reject(0, &c1).unwrap();
    assert_eq!(
        events_of::<events::CandidateRejected>(&sent.logs)[0].candidate,
        c1
    );
    assert_eq!(env.load::<Candidate>(&c1).status, CandidateStatus::Rejected);
    assert_eq!(
        env.load::<EventState>(&event_pda(0)).candidate_count[0],
        1,
        "the slot is free again"
    );
    assert_err!(env.do_reject(0, &c1), E::CandidateNotRejectable);

    // a new candidate takes the freed slot; the rejected ticker stays burned
    env.do_propose(&p1, 0, &propose_args("CCC", 0, SOL))
        .unwrap();
    let err = env
        .do_propose(&p1, 0, &propose_args("AAA", 0, SOL))
        .unwrap_err();
    assert!(
        err.logs.iter().any(|l| l.contains("already in use")),
        "{err:?}"
    );
    // no new money into the rejected candidate
    assert_err!(env.do_commit(&a, 0, &c1, SOL), E::CandidateNotPending);
    let late = env.wallet(10);
    assert_err!(
        env.do_co_propose(&late, 0, &c1, SOL),
        E::CandidateNotPending
    );

    // everyone gets 100% back right away, while the event is still open; the proposal fee stays
    let total = env.load::<Candidate>(&c1).total_committed;
    assert_eq!(total, 6 * SOL);
    let treasury_before = env.lamports(&env.treasury);
    for w in [&p1, &a, &b] {
        env.do_refund(w, 0, &c1).unwrap();
    }
    assert_eq!(env.load::<Candidate>(&c1).refunded_total, total);
    assert_eq!(
        env.lamports(&env.treasury),
        treasury_before,
        "refunds take nothing from the treasury and give nothing back"
    );
}

#[test]
fn rejection_is_only_possible_before_the_window_closes() {
    let mut env = setup();
    let w = env.wallet(100);
    let c = env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)).unwrap();
    env.warp_to(T0 + 3_599);
    env.do_reject(0, &c)
        .unwrap_or_else(|f| panic!("T-1 should be allowed: {f:?}"));
    let c2 = env.do_propose(&w, 0, &propose_args("BBB", 0, SOL));
    assert!(c2.is_ok());
    env.warp_to(T0 + 3_600);
    assert_err!(env.do_reject(0, &c2.unwrap()), E::WindowClosed);
}

// ---- pause never blocks exits (invariant 6) ---------------------------------------------------------------------------

#[test]
fn neither_pause_flag_blocks_refund_reject_void_or_close() {
    let mut env = setup();
    let (w, v) = (env.wallet(100), env.wallet(100));
    let c1 = env.do_propose(&w, 0, &propose_args("AAA", 0, SOL)).unwrap();
    let c2 = env.do_propose(&v, 0, &propose_args("BBB", 0, SOL)).unwrap();
    env.send(
        &[set_pause(&env.admin.pubkey(), Some(true), Some(true))],
        &[&env.admin.insecure_clone()],
    )
    .unwrap();

    env.do_reject(0, &c1).unwrap();
    env.do_refund(&w, 0, &c1).unwrap();
    env.warp_to(T0 + 3_600);
    env.do_close(0).unwrap();
    env.do_void(0).unwrap();
    env.do_refund(&v, 0, &c2).unwrap();
    assert_eq!(env.load::<Candidate>(&c2).refunded_total, SOL);
}
