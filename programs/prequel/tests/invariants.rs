//! Invariant tests for Phase 1 (SPEC §5 invariants 1, 2, 4, 5, 8; invariant 6 is in lifecycle.rs).
//!
//! A seeded random walk applies every instruction (valid and invalid, in any order, across candidates, wallets
//! and time, including the exact seconds around each window close) and, after every step, checks the books.
//! A failed transaction must change nothing at all, and a failed proposal must leave nothing behind.
//!
//! | Spec invariant | Where it is checked |
//! | 1 sum of commitments = total committed | `check_books`: open commitments + refunded_total == total_committed, every step |
//! | 2 refunds return exactly each commit; the vault ends at its own balance | per refund in the walk (vault and wallet deltas), `check_books`, and the end of `run_walk` |
//! | 4 claimed or refunded, never both, at most once | a refunded wallet has no commitment and a second refund fails (every step) |
//! | 5 no proposal or commit after close; voided only refunds | every money-in attempt in the walk; late and voided attempts are counted so the check cannot be vacuous |
//! | 8 at most one candidate per (event, outcome, canon hash) | `one_candidate_per_canonical_ticker`, and duplicate detection in the walk |
//!
//! Also checked every step: the treasury holds exactly the proposal fees paid (nobody holds committers' SOL),
//! `candidate_count` matches the slate, statuses only move forward.
mod common;

use anchor_lang::prelude::Pubkey;
use anchor_lang::Space;
use common::ix::*;
use common::*;
use prequel::{
    canon, events, Candidate, CandidateStatus, Commitment, EventState, EventStatus, Vault,
};
use solana_keypair::Keypair;
use solana_signer::Signer;
use std::collections::{BTreeMap, BTreeSet};

/// xorshift64*: small, seeded, no dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

struct Cand {
    key: Pubkey,
    event: u64,
    vault_creation: u64,
    donations: u64,
    members: BTreeSet<usize>,
    refunded: BTreeSet<usize>,
    co_weights: u64,
}

struct World {
    env: Env,
    wallets: Vec<Keypair>,
    cands: Vec<Cand>,
    events: Vec<u64>,
    donor: Keypair,
    /// Proposal and co-proposal fees paid so far. The treasury must hold exactly this: fees, never commits.
    fees_paid: u64,
}

const TICKERS: [&str; 8] = ["BOB", "b0b", "PEPE", "pepe", "S5S", "sss", "WIF", "w1f"];
/// Largest transaction fee a refund may cost its payer in these tests (one signature, 5,000 lamports).
const FEE_ALLOWANCE: i128 = 20_000;

impl World {
    fn new() -> World {
        let mut env = Env::new();
        env.init_config_with(|p| p.max_candidates_per_outcome = 3);
        let mut events = vec![env.create_default_event("EV-0").0];
        // The second event opens 15 minutes later, so the two windows close at different seconds.
        let t = env.now();
        env.warp_to(t + 900);
        events.push(env.create_default_event("EV-1").0);
        let wallets = (0..6).map(|_| env.wallet(1_000)).collect();
        let donor = env.wallet(10);
        World {
            env,
            wallets,
            cands: vec![],
            events,
            donor,
            fees_paid: 0,
        }
    }

    fn event(&self, id: u64) -> EventState {
        self.env.load(&event_pda(id))
    }

    /// Every address whose bytes or lamports a failed transaction must leave untouched.
    fn snapshot(&self) -> Vec<(Pubkey, u64, Vec<u8>)> {
        let mut keys = vec![config_pda(), self.env.treasury];
        keys.extend(self.events.iter().map(|id| event_pda(*id)));
        for c in &self.cands {
            keys.push(c.key);
            keys.push(vault_pda(&c.key));
            for w in &self.wallets {
                keys.push(commitment_pda(&c.key, &w.pubkey()));
                keys.push(coproposer_pda(&c.key, &w.pubkey()));
            }
        }
        keys.iter()
            .map(|k| {
                self.env
                    .svm
                    .get_account(k)
                    .map_or((*k, 0, vec![]), |a| (*k, a.lamports, a.data))
            })
            .collect()
    }

    /// Refunds wallet `wi` on candidate `ci`. When it works, checks everything invariants 2 and 4 say about one
    /// refund: the vault pays exactly the commitment, the wallet receives it plus the rent of the accounts that
    /// were closed (less its own fee), the event carries the same numbers, and both accounts are gone. A refusal
    /// is returned for the caller to count (the walk) or to treat as a failure (the final phase).
    fn refund_checked(&mut self, ci: usize, wi: usize, desc: &str) -> Result<(), Failed> {
        let wallet = self.wallets[wi].insecure_clone();
        let (key, cev) = (self.cands[ci].key, self.cands[ci].event);
        let (ckey, skey, vkey) = (
            commitment_pda(&key, &wallet.pubkey()),
            coproposer_pda(&key, &wallet.pubkey()),
            vault_pda(&key),
        );
        let owed = self
            .env
            .exists(&ckey)
            .then(|| self.env.load::<Commitment>(&ckey).amount);
        let vault_before = self.env.lamports(&vkey);
        let wallet_before = self.env.lamports(&wallet.pubkey());
        let closed_rent = self.env.lamports(&ckey) + self.env.lamports(&skey);
        let sent = self.env.do_refund(&wallet, cev, &key)?;
        let owed =
            owed.unwrap_or_else(|| panic!("refund paid a wallet with no commitment ({desc})"));
        assert!(self.cands[ci].members.contains(&wi));
        assert!(
            self.cands[ci].refunded.insert(wi),
            "invariant 4: refunded twice ({desc})"
        );
        assert_eq!(
            vault_before - self.env.lamports(&vkey),
            owed,
            "invariant 2: the vault pays exactly the commitment ({desc})"
        );
        let gain = self.env.lamports(&wallet.pubkey()) as i128 - wallet_before as i128;
        let expect = (owed + closed_rent) as i128;
        assert!(
            gain <= expect && gain >= expect - FEE_ALLOWANCE,
            "invariant 2: the wallet got {gain}, expected {expect} less a fee ({desc})"
        );
        let e = &events_of::<events::Refunded>(&sent.logs)[0];
        assert_eq!(
            (e.amount, e.wallet, e.candidate),
            (owed, wallet.pubkey(), key)
        );
        assert!(
            !self.env.exists(&ckey) && !self.env.exists(&skey),
            "invariant 4: the commitment and share are gone after a refund ({desc})"
        );
        Ok(())
    }

    /// Spec invariants 1, 2 and 4 (and the reworded forms of Q-05), for every candidate, plus the slate and treasury.
    fn check_books(&self) {
        for c in &self.cands {
            let cand: Candidate = self.env.load(&c.key);
            let mut open = 0u64;
            for (i, w) in self.wallets.iter().enumerate() {
                let key = commitment_pda(&c.key, &w.pubkey());
                if self.env.exists(&key) {
                    assert!(
                        !c.refunded.contains(&i),
                        "invariant 4: wallet {i} was refunded but still has a commitment"
                    );
                    assert!(c.members.contains(&i));
                    open += self.env.load::<Commitment>(&key).amount;
                } else {
                    assert!(
                        !c.members.contains(&i) || c.refunded.contains(&i),
                        "wallet {i}'s commitment vanished without a refund"
                    );
                }
            }
            assert_eq!(
                open + cand.refunded_total,
                cand.total_committed,
                "invariant 1: open + refunded = total"
            );
            if cand.refunded_total == 0 {
                assert_eq!(
                    open, cand.total_committed,
                    "invariant 1 as the spec words it: before any refund the commitments add up to the total"
                );
            }
            assert_eq!(
                self.env.lamports(&vault_pda(&c.key)),
                c.vault_creation + cand.total_committed - cand.refunded_total + c.donations,
                "invariant 2: vault = creation balance + what is still owed + donations"
            );
            assert_eq!(
                cand.committer_count as usize,
                c.members.len(),
                "committer_count counts each wallet that ever joined, once"
            );
            assert_eq!(
                cand.co_proposer_total, c.co_weights,
                "co-proposer weights add up"
            );
            let ev = self.event(c.event);
            if ev.status == EventStatus::Voided {
                assert_ne!(cand.status, CandidateStatus::Born);
            }
            if cand.refunded_total > 0 {
                assert!(matches!(
                    cand.status,
                    CandidateStatus::Refunding | CandidateStatus::Rejected
                ));
            }
        }
        // The slate: candidate_count[o] is the number of candidates on outcome o that were not rejected, and never
        // exceeds the cap.
        for id in &self.events {
            let ev = self.event(*id);
            for o in 0..8u8 {
                let live = self
                    .cands
                    .iter()
                    .filter(|c| c.event == *id)
                    .filter(|c| {
                        let cand: Candidate = self.env.load(&c.key);
                        cand.outcome == o && cand.status != CandidateStatus::Rejected
                    })
                    .count();
                assert_eq!(
                    ev.candidate_count[o as usize] as usize, live,
                    "event {id} outcome {o}: candidate_count matches the live candidates"
                );
                assert!(ev.candidate_count[o as usize] <= ev.params.max_candidates_per_outcome);
            }
        }
        assert_eq!(
            self.env.lamports(&self.env.treasury),
            self.fees_paid,
            "the treasury holds exactly the proposal fees: it never touches committers' SOL"
        );
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Op {
    Propose,
    Commit,
    CoPropose,
    Refund,
    Reject,
    Void,
    Close,
    Donate,
    Warp,
}

fn pick_op(rng: &mut Rng, have_candidates: bool) -> Op {
    let op = match rng.below(100) {
        0..=19 => Op::Propose,
        20..=37 => Op::Commit,
        38..=47 => Op::CoPropose,
        48..=59 => Op::Refund,
        60..=65 => Op::Reject,
        66..=67 => Op::Void,
        68..=73 => Op::Close,
        74..=79 => Op::Donate,
        _ => Op::Warp,
    };
    if !have_candidates && !matches!(op, Op::Void | Op::Close | Op::Warp) {
        Op::Propose
    } else {
        op
    }
}

/// How much of the instruction set a walk really exercised. A walk that only ever failed would pass every
/// invariant vacuously, so the top-level test asserts minimum coverage.
#[derive(Default, Debug, Clone, Copy)]
struct Stats {
    proposed: u32,
    committed: u32,
    co_proposed: u32,
    refunded: u32,
    /// Refunds in the closing phase, where every wallet in every candidate is paid back after a void.
    refunded_at_end: u32,
    rejected: u32,
    voided: u32,
    closed: u32,
    donated: u32,
    /// Steps that failed and were checked to have changed nothing.
    failed_unchanged: u32,
    /// Propose / commit / co_propose attempts made at or after the window close (all must fail).
    late_money_in: u32,
    /// Money-in attempts on a voided event (all must fail).
    money_in_on_voided: u32,
    /// Money-in attempts within one second of a window close (T-1, T, T+1).
    at_boundary: u32,
    /// Refund attempts that were refused (nothing owed yet, not refundable, already refunded, not a member).
    refunds_refused: u32,
    /// Proposals refused because the (event, outcome, canon hash) already had a candidate.
    duplicates_refused: u32,
}

impl Stats {
    fn add(&mut self, o: &Stats) {
        self.proposed += o.proposed;
        self.committed += o.committed;
        self.co_proposed += o.co_proposed;
        self.refunded += o.refunded;
        self.refunded_at_end += o.refunded_at_end;
        self.rejected += o.rejected;
        self.voided += o.voided;
        self.closed += o.closed;
        self.donated += o.donated;
        self.failed_unchanged += o.failed_unchanged;
        self.late_money_in += o.late_money_in;
        self.money_in_on_voided += o.money_in_on_voided;
        self.at_boundary += o.at_boundary;
        self.refunds_refused += o.refunds_refused;
        self.duplicates_refused += o.duplicates_refused;
    }

    /// Records what the event looked like when a money-in instruction was attempted.
    fn note_money_in(&mut self, ev: &EventState, now: i64) {
        if now >= ev.window_close {
            self.late_money_in += 1;
        }
        if ev.status == EventStatus::Voided {
            self.money_in_on_voided += 1;
        }
        if (now - ev.window_close).abs() <= 1 {
            self.at_boundary += 1;
        }
    }
}

/// Invariant 5, for a money-in instruction that succeeded: the window was open at this exact second and the stored
/// status was Open (a voided event takes no money; a stale `Open` after close is not an excuse).
fn assert_money_in_allowed(pre: &EventState, now: i64, desc: &str) {
    assert_eq!(
        pre.status,
        EventStatus::Open,
        "invariant 5: money went in while the event was not Open ({desc})"
    );
    assert!(
        pre.window_open <= now && now < pre.window_close,
        "invariant 5: money went in outside the window [{}, {}) at {now} ({desc})",
        pre.window_open,
        pre.window_close
    );
}

fn run_walk(seed: u64, steps: usize) -> Stats {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut w = World::new();
    let mut st = Stats::default();
    let mut last_rank: BTreeMap<u64, u8> = BTreeMap::new();
    let mut last_candidate: BTreeMap<Pubkey, CandidateStatus> = BTreeMap::new();

    for step in 0..steps {
        let before = w.snapshot();
        let now = w.env.now();
        let wi = rng.below(w.wallets.len());
        let wallet = w.wallets[wi].insecure_clone();
        let ev = w.events[rng.below(w.events.len())];
        let amount = SOL / 20 + (rng.next() % (3 * SOL));
        let op = pick_op(&mut rng, !w.cands.is_empty());
        let mut desc = format!(
            "seed {seed} step {step} {op:?} wallet {wi} event {ev} t={}",
            now - T0
        );
        let mut changed = false;

        match op {
            Op::Propose => {
                let ticker = TICKERS[rng.below(TICKERS.len())];
                let outcome = rng.below(2) as u8;
                let mut args = propose_args(ticker, outcome, amount);
                args.attestation_expiry = now + 600;
                desc += &format!(" {ticker}/{outcome} amount {amount}");
                let key = candidate_for(ev, &args);
                let known = w.cands.iter().any(|c| c.key == key);
                let pre = w.event(ev);
                st.note_money_in(&pre, now);
                match w.env.do_propose(&wallet, ev, &args) {
                    Ok(_) => {
                        changed = true;
                        st.proposed += 1;
                        assert_money_in_allowed(&pre, now, &desc);
                        assert!(
                            !known,
                            "invariant 8: a second candidate was created for the same (event, outcome, canon) ({desc})"
                        );
                        let vault_creation = w.env.lamports(&vault_pda(&key)) - amount;
                        assert_eq!(
                            vault_creation,
                            w.env.rent_minimum(8 + Vault::INIT_SPACE),
                            "a new vault holds exactly its rent plus the first commit"
                        );
                        w.fees_paid += pre.params.proposal_fee;
                        w.cands.push(Cand {
                            key,
                            event: ev,
                            vault_creation,
                            donations: 0,
                            members: BTreeSet::from([wi]),
                            refunded: BTreeSet::new(),
                            co_weights: 0,
                        });
                    }
                    Err(_) => {
                        if known {
                            st.duplicates_refused += 1;
                        } else {
                            // a failed creation leaves nothing behind
                            for k in [key, vault_pda(&key), commitment_pda(&key, &wallet.pubkey())]
                            {
                                assert!(
                                    !w.env.exists(&k),
                                    "a failed proposal left {k} behind ({desc})"
                                );
                            }
                        }
                    }
                }
            }
            Op::Commit | Op::CoPropose => {
                let ci = rng.below(w.cands.len());
                let (key, cev) = (w.cands[ci].key, w.cands[ci].event);
                desc += &format!(" (candidate {ci} of event {cev})");
                let pre = w.event(cev);
                st.note_money_in(&pre, now);
                if op == Op::Commit {
                    if w.env.do_commit(&wallet, cev, &key, amount).is_ok() {
                        changed = true;
                        st.committed += 1;
                        assert_money_in_allowed(&pre, now, &desc);
                        w.cands[ci].members.insert(wi);
                    }
                } else if w.env.do_co_propose(&wallet, cev, &key, amount).is_ok() {
                    changed = true;
                    st.co_proposed += 1;
                    assert_money_in_allowed(&pre, now, &desc);
                    w.cands[ci].members.insert(wi);
                    w.cands[ci].co_weights += amount;
                    w.fees_paid += pre.params.proposal_fee;
                }
            }
            Op::Refund => {
                let ci = rng.below(w.cands.len());
                desc += &format!(" (candidate {ci} of event {})", w.cands[ci].event);
                match w.refund_checked(ci, wi, &desc) {
                    Ok(()) => {
                        changed = true;
                        st.refunded += 1;
                    }
                    Err(_) => st.refunds_refused += 1,
                }
            }
            Op::Reject => {
                let ci = rng.below(w.cands.len());
                let (key, cev) = (w.cands[ci].key, w.cands[ci].event);
                desc += &format!(" (candidate {ci} of event {cev})");
                let pre = w.event(cev);
                if w.env.do_reject(cev, &key).is_ok() {
                    changed = true;
                    st.rejected += 1;
                    assert!(
                        pre.status == EventStatus::Open && now < pre.window_close,
                        "a candidate was rejected after the window closed ({desc})"
                    );
                    let cand: Candidate = w.env.load(&key);
                    assert_eq!(cand.status, CandidateStatus::Rejected);
                }
            }
            Op::Void => {
                if w.env.do_void(ev).is_ok() {
                    changed = true;
                    st.voided += 1;
                    assert_eq!(w.event(ev).status, EventStatus::Voided);
                }
            }
            Op::Close => {
                let pre = w.event(ev);
                if w.env.do_close(ev).is_ok() {
                    changed = true;
                    st.closed += 1;
                    assert!(
                        pre.status == EventStatus::Open && now >= pre.window_close,
                        "close_window worked before the window closed ({desc})"
                    );
                    assert_eq!(w.event(ev).status, EventStatus::Closed);
                }
            }
            Op::Donate => {
                // anyone can send lamports to a vault; it must not change what is owed
                let ci = rng.below(w.cands.len());
                let dust = 1 + rng.next() % 5_000_000;
                let ix = anchor_lang::solana_program::system_instruction::transfer(
                    &w.donor.pubkey(),
                    &vault_pda(&w.cands[ci].key),
                    dust,
                );
                let donor = w.donor.insecure_clone();
                if w.env.send(&[ix], &[&donor]).is_ok() {
                    changed = true;
                    st.donated += 1;
                    w.cands[ci].donations += dust;
                }
            }
            Op::Warp => {
                // one time in three, land within a second of a window close (T-1, T, T+1)
                let target = if rng.below(3) == 0 {
                    let close = w.event(w.events[rng.below(w.events.len())]).window_close;
                    close + [-1i64, 0, 1][rng.below(3)]
                } else {
                    now + [1i64, 30, 120, 600, 900][rng.below(5)]
                };
                w.env.warp_to(target.max(now));
                changed = true;
            }
        }

        if !changed {
            st.failed_unchanged += 1;
            assert_eq!(
                w.snapshot(),
                before,
                "a failed or no-op step must change nothing ({desc})"
            );
        }
        w.check_books();
        // event status only moves forward: Open(0) -> Closed(1) -> Voided(5)
        for id in &w.events {
            let rank = match w.event(*id).status {
                EventStatus::Open => 0,
                EventStatus::Closed => 1,
                _ => 5,
            };
            let prev = last_rank.insert(*id, rank).unwrap_or(0);
            assert!(rank >= prev, "event {id} status went backwards ({desc})");
        }
        // candidate status: Pending may become Rejected or Refunding; those two never change again
        for c in &w.cands {
            let now_status = w.env.load::<Candidate>(&c.key).status;
            let before = last_candidate
                .insert(c.key, now_status)
                .unwrap_or(CandidateStatus::Pending);
            let allowed = before == now_status
                || (before == CandidateStatus::Pending
                    && matches!(
                        now_status,
                        CandidateStatus::Rejected | CandidateStatus::Refunding
                    ));
            assert!(
                allowed,
                "candidate {} went {before:?} -> {now_status:?} ({desc})",
                c.key
            );
        }
    }

    // Finally: void every event and refund everybody. Every wallet gets exactly what it put in.
    for id in w.events.clone() {
        let _ = w.env.do_void(id);
    }
    for ci in 0..w.cands.len() {
        for wi in 0..w.wallets.len() {
            if w.cands[ci].members.contains(&wi) && !w.cands[ci].refunded.contains(&wi) {
                w.refund_checked(ci, wi, &format!("seed {seed} final refund"))
                    .unwrap_or_else(|f| panic!("final refund failed, seed {seed}: {f:?}"));
                st.refunded_at_end += 1;
            }
        }
    }
    w.check_books();
    for c in &w.cands {
        let cand: Candidate = w.env.load(&c.key);
        assert_eq!(
            cand.refunded_total, cand.total_committed,
            "everything was returned"
        );
        assert_eq!(
            c.vault_creation,
            w.env.rent_minimum(8 + Vault::INIT_SPACE),
            "the vault's own balance is the rent-exempt minimum"
        );
        assert_eq!(
            w.env.lamports(&vault_pda(&c.key)),
            c.vault_creation + c.donations,
            "invariant 2: the vault ends at the rent-exempt minimum, plus whatever was donated"
        );
    }
    st
}

#[test]
fn random_walks_keep_the_books_balanced_and_failures_change_nothing() {
    let mut total = Stats::default();
    for seed in 1..=16 {
        total.add(&run_walk(seed, 80));
    }
    eprintln!("coverage over 16 walks of 80 steps: {total:#?}");
    // A walk that never succeeded at anything would pass every invariant. These floors keep the test honest.
    let floors = [
        ("proposed", total.proposed, 40),
        ("committed", total.committed, 40),
        ("co_proposed", total.co_proposed, 15),
        ("refunded", total.refunded, 12),
        ("refunded_at_end", total.refunded_at_end, 60),
        ("rejected", total.rejected, 8),
        ("voided", total.voided, 3),
        ("closed", total.closed, 8),
        ("donated", total.donated, 20),
        ("failed_unchanged", total.failed_unchanged, 200),
        ("late_money_in", total.late_money_in, 40),
        ("money_in_on_voided", total.money_in_on_voided, 3),
        ("at_boundary", total.at_boundary, 8),
        ("refunds_refused", total.refunds_refused, 30),
        ("duplicates_refused", total.duplicates_refused, 5),
    ];
    for (name, got, floor) in floors {
        assert!(
            got >= floor,
            "coverage: {name} = {got}, wanted at least {floor}"
        );
    }
}

#[test]
fn one_candidate_per_canonical_ticker() {
    // Invariant 8 as an address property: every spelling that canonicalizes alike derives the same candidate address,
    // so at most one can ever be created; different canon, outcome or event give different addresses.
    let mut env = Env::new();
    env.init_config_with(|_| {});
    env.create_default_event("EV-8");
    env.create_default_event("EV-9");
    let w = env.wallet(1_000);
    let families: [&[&str]; 3] = [
        &["BOB", "bob", "BoB", "B0B", "b0b", "B0b"],
        &["BO8"],
        &["SOS", "s05", "5O5", "s0S"],
    ];
    let mut addresses = BTreeSet::new();
    for family in families {
        let key = candidate_for(0, &propose_args(family[0], 0, SOL));
        assert!(addresses.insert(key), "two families share an address");
        let mut created = 0;
        for s in family {
            assert_eq!(
                candidate_for(0, &propose_args(s, 0, SOL)),
                key,
                "{s:?} derives the family's address"
            );
            created += usize::from(env.do_propose(&w, 0, &propose_args(s, 0, SOL)).is_ok());
        }
        assert_eq!(created, 1, "exactly one spelling of {} wins", family[0]);
        assert!(env.exists(&key));
        // the same coin on the other outcome, and in the other event, is a different candidate
        let other_outcome = candidate_for(0, &propose_args(family[0], 1, SOL));
        let other_event = candidate_for(1, &propose_args(family[0], 0, SOL));
        assert!(addresses.insert(other_outcome) && addresses.insert(other_event));
        env.do_propose(&w, 0, &propose_args(family[1 % family.len()], 1, SOL))
            .expect("the same ticker on the other outcome is allowed");
        env.do_propose(&w, 1, &propose_args(family[0], 0, SOL))
            .expect("the same ticker in another event is allowed");
    }
    // spellings that are not valid tickers never create anything
    for bad in ["SOS ", "B", "B0B0B0B0B0B", "BÖB"] {
        assert!(
            canon::canonical_ticker(bad).is_err(),
            "{bad:?} must not canonicalize"
        );
        assert!(
            env.do_propose(&w, 0, &propose_args(bad, 0, SOL)).is_err(),
            "{bad:?} must not create a candidate"
        );
    }
}
