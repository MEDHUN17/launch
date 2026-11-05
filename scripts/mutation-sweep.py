#!/usr/bin/env python3
"""Mutation sweep for the Phase 1 program: breaks the program in one small way at a time and checks that a test fails.

A test suite that stays green after a deliberate bug is not testing that line. The list below is the second code
review's "mutations expected to survive" (all of which now have a killing test) plus the core rules of Phase 1.

    python3 -I scripts/mutation-sweep.py --check       only verify that every mutation applies exactly once
    python3 -I scripts/mutation-sweep.py               run all of them (about 40 minutes)
    python3 -I scripts/mutation-sweep.py M03 M25       run only these

Needs the Solana tools on PATH (see CLAUDE.md). Each mutation is applied to the working tree, the program is
rebuilt, the tests run, and the file is restored in a `finally`. Do not commit while it runs. Exit status is 1 if
any mutation survived or failed to apply.
"""
import os
import pathlib
import re
import signal
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "programs" / "prequel" / "src"
LOCK = ROOT / "target" / ".mutation-sweep.lock"

# (id, file under src/, old text (must occur exactly once), new text, what it breaks)
M = []


def mutation(mid, rel, old, new, note):
    M.append((mid, rel, old, new, note))


# ---- the review's list (mutation numbers refer to the review's section b) -------------------------------------------
mutation("M01", "instructions/config.rs", "program.programdata_address()? == Some(program_data.key())", "true",
         "1: initialize_config no longer ties program_data to this program")
mutation("M02a", "instructions/candidates.rs", "let max_slate = event.params.max_candidates_per_outcome;",
         "let max_slate = ctx.accounts.config.params.max_candidates_per_outcome;", "2: slate size read from config")
mutation("M02b", "instructions/candidates.rs", "if event.params.require_attestation {",
         "if ctx.accounts.config.params.require_attestation {", "2: attestation flag read from config")
mutation("M02c", "instructions/candidates.rs", "let proposal_fee = event.params.proposal_fee;",
         "let proposal_fee = ctx.accounts.config.params.proposal_fee;", "2: propose fee read from config")
mutation("M02d", "instructions/candidates.rs", "event.params.proposal_cutoff,",
         "ctx.accounts.config.params.proposal_cutoff,", "2: cutoff read from config")
mutation("M02e", "instructions/candidates.rs", "let amount = logic::check_commit_amount(&event.params, 0, args.amount)?;",
         "let amount = logic::check_commit_amount(&ctx.accounts.config.params, 0, args.amount)?;",
         "2: propose amount bounds read from config")
mutation("M02f", "instructions/candidates.rs", "let proposal_fee = ctx.accounts.event.params.proposal_fee;",
         "let proposal_fee = ctx.accounts.config.params.proposal_fee;", "2: co_propose fee read from config")
mutation("M02g", "instructions/candidates.rs", "        &event.params,\n        if fresh",
         "        &Params::spec_defaults(),\n        if fresh", "2: commit amount bounds ignore the event's settings")
mutation("M03", "instructions/lifecycle.rs",
         "logic::check_accepts_commits(event.status, event.window_open, event.window_close, now)?;",
         "logic::check_accepts_proposals(event.status, event.window_open, event.window_close, event.params.proposal_cutoff, now)?;",
         "3: rejection stops at the proposal cutoff")
mutation("M04a", "attestation.rs", "    m.extend_from_slice(a.config.as_ref());\n    m.extend_from_slice(a.event.as_ref());",
         "    m.extend_from_slice(a.event.as_ref());\n    m.extend_from_slice(a.config.as_ref());", "4: config and event swapped in the message")
mutation("M04b", "attestation.rs", "    m.extend_from_slice(a.image_hash);\n    m.extend_from_slice(a.proposer.as_ref());",
         "    m.extend_from_slice(a.proposer.as_ref());\n    m.extend_from_slice(a.image_hash);", "4: image hash and proposer swapped")
mutation("M04c", "attestation.rs", "        name.len() as u16,\n        display_ticker.len() as u16,\n        uri.len() as u16,",
         "        name.len() as u32,\n        display_ticker.len() as u32,\n        uri.len() as u32,", "4: u32 length prefixes")
mutation("M05", "instructions/candidates.rs", "attestation::content_hash(&args.name, &display, &args.uri)",
         "attestation::content_hash(&args.name, &args.ticker, &args.uri)", "5: the raw ticker is hashed instead of the stored one")
mutation("M06a", "instructions/config.rs", "        &args.resolver,\n", "", "6: initialize_config accepts a zero resolver")
mutation("M06b", "instructions/config.rs", "        &args.canonicalizer,\n", "", "6: initialize_config accepts a zero canonicalizer")
mutation("M06c", "instructions/config.rs", "        check_keys(&[&k])?;\n        config.admin = k;", "        config.admin = k;",
         "6: update_config accepts a zero admin")
mutation("M06d", "instructions/config.rs", "        check_keys(&[&k])?;\n        config.resolver = k;", "        config.resolver = k;",
         "6: update_config accepts a zero resolver")
mutation("M06e", "instructions/config.rs", "        check_keys(&[&k])?;\n        config.canonicalizer = k;", "        config.canonicalizer = k;",
         "6: update_config accepts a zero canonicalizer")
mutation("M06f", "instructions/config.rs", "        check_keys(&[&k])?;\n        config.treasury = k;", "        config.treasury = k;",
         "6: update_config accepts a zero treasury")
mutation("M06g", "instructions/config.rs", "        config.treasury = k;", "        let _ = k;", "6: treasury rotation is ignored")
mutation("M07", "instructions/events.rs", "cutoff == 0 || cutoff < args.window_close - args.window_open,", "true,",
         "7: a cutoff may swallow the whole window")
mutation("M08a", "instructions/candidates.rs", "candidate.image_hash = args.image_hash;", "candidate.image_hash = [0; 32];", "8: image hash not stored")
mutation("M08i", "instructions/candidates.rs", "candidate.layout_version = 1;", "candidate.layout_version = 0;", "8: candidate layout version not stored")
mutation("M08b", "instructions/candidates.rs", "        commitment.wallet = wallet_key;\n        commitment.first_commit_ts = now;",
         "        commitment.wallet = wallet_key;\n        commitment.first_commit_ts = 0;", "8: first commit time not stored (commit path)")
mutation("M08c", "instructions/candidates.rs", "share.candidate = candidate_key;", "share.candidate = Pubkey::default();", "8: share candidate not stored")
mutation("M08d", "instructions/candidates.rs", "share.wallet = wallet_key;", "share.wallet = Pubkey::default();", "8: share wallet not stored")
mutation("M08e", "instructions/events.rs", "event.external_id_hash = args.external_id_hash;", "event.external_id_hash = [0; 32];", "8: event hash not stored")
mutation("M08f", "instructions/events.rs", "event.layout_version = 1;", "event.layout_version = 0;", "8: event layout version not stored")
mutation("M08g", "instructions/events.rs", "event.external_id = args.external_id.clone();", "event.external_id = String::new();", "8: external id not stored")
mutation("M08h", "instructions/events.rs", "listing.external_id_hash = args.external_id_hash;", "listing.external_id_hash = [0; 32];", "8: listing hash not stored")

# 9: off-by-one at bounds that must be accepted (or must be refused)
for mid, old, new, note in [
    ("M09a", "(MIN_DISPUTE_WINDOW..=MAX_DISPUTE_WINDOW)", "(MIN_DISPUTE_WINDOW..MAX_DISPUTE_WINDOW)", "max dispute window excluded"),
    ("M09b", "(MIN_DISPUTE_BOND..=MAX_DISPUTE_BOND)", "(MIN_DISPUTE_BOND..MAX_DISPUTE_BOND)", "max bond excluded"),
    ("M09c", "(MIN_LAUNCH_WINDOW..=MAX_LAUNCH_WINDOW)", "(MIN_LAUNCH_WINDOW..MAX_LAUNCH_WINDOW)", "max launch window excluded"),
    ("M09d", "(MIN_RESOLUTION_DELAY..=MAX_RESOLUTION_DELAY)", "(MIN_RESOLUTION_DELAY..MAX_RESOLUTION_DELAY)", "max delay excluded"),
    ("M09e", "(MIN_PROPOSAL_FEE..=MAX_PROPOSAL_FEE)", "(MIN_PROPOSAL_FEE..MAX_PROPOSAL_FEE)", "max proposal fee excluded"),
    ("M09f", "!(MIN_MIN_RAISE..=MAX_MIN_RAISE)", "!(MIN_MIN_RAISE..MAX_MIN_RAISE)", "max min_raise excluded"),
    ("M09g", "if p.min_commit < MIN_COMMIT_FLOOR {", "if p.min_commit <= MIN_COMMIT_FLOOR {", "min_commit floor excluded"),
    ("M09h", "if p.min_raise < p.min_commit ||", "if p.min_raise <= p.min_commit ||", "min_raise equal to min_commit refused"),
    ("M09i", "p.max_commit_per_wallet < p.min_commit {", "p.max_commit_per_wallet <= p.min_commit {", "cap equal to min_commit refused"),
    ("M09j", "if p.success_fee_bps > MAX_SUCCESS_FEE_BPS {", "if p.success_fee_bps >= MAX_SUCCESS_FEE_BPS {", "fee at the cap refused"),
    ("M09k", "id.len() > MAX_EXTERNAL_ID_LEN", "id.len() >= MAX_EXTERNAL_ID_LEN", "64-byte external id refused"),
]:
    mutation(mid, "logic.rs", old, new, "9: " + note)

mutation("M10a", "state.rs", "    pub outcome: u8,\n    pub layout_version: u8,\n    pub bump: u8,\n    pub vault_bump: u8,",
         "    pub layout_version: u8,\n    pub outcome: u8,\n    pub bump: u8,\n    pub vault_bump: u8,", "10: Candidate outcome and layout_version swapped")
mutation("M10b", "state.rs", "    pub window_open: i64,\n    pub window_close: i64,", "    pub window_close: i64,\n    pub window_open: i64,",
         "10: EventState window times swapped")
mutation("M10c", "state.rs", "    pub amount: u64,\n    pub first_commit_ts: i64,", "    pub first_commit_ts: i64,\n    pub amount: u64,",
         "10: Commitment amount and first_commit_ts swapped")
mutation("M10d", "state.rs", "    pub admin: Pubkey,\n    pub resolver: Pubkey,", "    pub resolver: Pubkey,\n    pub admin: Pubkey,", "10: Config admin and resolver swapped")

# ---- the core rules of Phase 1 ----------------------------------------------------------------------------------------
mutation("C01", "logic.rs", "if stored == EventStatus::Open && now >= window_close {", "if stored == EventStatus::Open && now > window_close {",
         "window closes one second late")
mutation("C02", "instructions/lifecycle.rs", "candidate.refunded_total = refunded;", "let _ = refunded;", "refund skips refunded_total")
mutation("C03", "instructions/lifecycle.rs", "ctx.accounts.vault.sub_lamports(amount)?;", "let _ = amount;", "refund never debits the vault")
mutation("C04", "instructions/lifecycle.rs", "!ctx.accounts.commitment.has_share || ctx.accounts.co_proposer_share.is_some()", "true",
         "refund no longer requires the share")
mutation("C05", "instructions/candidates.rs", "ctx.accounts.commitment.has_share = true;", "let _ = 0;", "co_propose does not set has_share")
mutation("C06", "instructions/candidates.rs", "event.canon_version == canon::CANON_VERSION,", "true,", "ticker-table version not checked")
mutation("C07", "logic.rs", "if p.reserved != [0u8; 32] {", "if false {", "reserved settings bytes not checked")
mutation("C08", "instructions/candidates.rs", "wallet_key != ctx.accounts.candidate.proposer,", "true,", "the proposer may co-propose")
mutation("C09", "instructions/candidates.rs", "canon_hash == args.canon_hash,", "true,", "canon hash taken from the caller")
mutation("C10", "instructions/candidates.rs", "require!(*slot < max_slate, PrequelError::SlateFull);", "require!(true, PrequelError::SlateFull);", "slate cap removed")
mutation("C11", "instructions/lifecycle.rs", "matches!(event.status, EventStatus::Open | EventStatus::Closed),", "true,", "void from any status")
mutation("C12", "instructions/lifecycle.rs", "require!(now >= event.window_close, PrequelError::WindowNotClosedYet);",
         "require!(true, PrequelError::WindowNotClosedYet);", "close_window before the close time")
mutation("C13", "instructions/lifecycle.rs", "*slot = slot.checked_sub(1).ok_or(PrequelError::MathOverflow)?;", "let _ = slot;",
         "rejection does not free the slot")
mutation("C14", "instructions/lifecycle.rs",
         "pub struct VoidEvent<'info> {\n    pub admin: Signer<'info>,\n    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = admin @ PrequelError::NotAdmin)]",
         "pub struct VoidEvent<'info> {\n    pub admin: Signer<'info>,\n    #[account(seeds = [CONFIG_SEED], bump = config.bump)]", "anyone may void")
mutation("C15", "instructions/candidates.rs",
         "pub fn handle_commit(ctx: Context<Commit>, amount: u64) -> Result<()> {\n    let now = Clock::get()?.unix_timestamp;\n    require!(\n        !ctx.accounts.config.pause_commits,\n        PrequelError::CommitsPaused\n    );",
         "pub fn handle_commit(ctx: Context<Commit>, amount: u64) -> Result<()> {\n    let now = Clock::get()?.unix_timestamp;", "commit ignores the pause flag")
mutation("C16", "attestation.rs", "require!(now <= attested.expiry, PrequelError::AttestationExpired);", "require!(now < attested.expiry, PrequelError::AttestationExpired);",
         "attestation expires one second early")
mutation("C17", "attestation.rs", "if sig_ix != SAME_INSTRUCTION || pk_ix != SAME_INSTRUCTION || msg_ix != SAME_INSTRUCTION {",
         "if pk_ix != SAME_INSTRUCTION || msg_ix != SAME_INSTRUCTION {", "signature index field not checked")
mutation("C18", "attestation.rs", "if sig_ix != SAME_INSTRUCTION || pk_ix != SAME_INSTRUCTION || msg_ix != SAME_INSTRUCTION {",
         "if sig_ix != SAME_INSTRUCTION || msg_ix != SAME_INSTRUCTION {", "public key index field not checked")
mutation("C19", "attestation.rs", "if sig_ix != SAME_INSTRUCTION || pk_ix != SAME_INSTRUCTION || msg_ix != SAME_INSTRUCTION {",
         "if sig_ix != SAME_INSTRUCTION || pk_ix != SAME_INSTRUCTION {", "message index field not checked")
mutation("C20", "attestation.rs", "if data.len() < HEADER_LEN || data[0] != 1 {", "if data.len() < HEADER_LEN || data[0] < 1 {",
         "more than one signature accepted")
mutation("C21", "attestation.rs", "parsed.pubkey == canonicalizer.as_ref(),", "true,", "any signer accepted")
mutation("C22", "attestation.rs", "previous.program_id == ED25519_PROGRAM_ID,", "true,", "any previous instruction accepted")
mutation("C23", "logic.rs", "if add < p.min_commit {", "if add <= p.min_commit {", "a commit of exactly min_commit refused")
mutation("C24", "logic.rs", "if p.max_commit_per_wallet != 0 && total > p.max_commit_per_wallet {", "if p.max_commit_per_wallet != 0 && total >= p.max_commit_per_wallet {",
         "a total of exactly the cap refused")
mutation("C25", "logic.rs", "if now < window_open {", "if now <= window_open {", "a commit at exactly window_open refused")
mutation("C26", "logic.rs", "now >= window_close.saturating_sub(proposal_cutoff)", "now > window_close.saturating_sub(proposal_cutoff)",
         "proposals allowed at the cutoff second")


# Files whose rules have unit tests: a mutation there is tried against `cargo test --lib` first (seconds).
UNIT_TESTED = {"logic.rs", "attestation.rs", "canon.rs"}
# Integration test binaries, fastest and most targeted first; the first one that fails ends the run.
TEST_BINARIES = [
    "review_followups", "layout", "attestation_vectors", "config_events", "lifecycle",
    "propose_commit", "review_hardening", "harness_smoke", "invariants",
]


def run(cmd, timeout=1800):
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=timeout)


def failing_tests(output):
    return sorted(set(re.findall(r"^test (\S+) \.\.\. FAILED$", output, flags=re.M)))


def did_not_compile(proc):
    return "error[" in proc.stderr or "could not compile" in proc.stderr or "error: " in proc.stderr and proc.returncode == 101 and "test result" not in proc.stdout


def judge(rel):
    """Returns (verdict, outcome) for the mutation currently applied; outcome is killed, survived or broken."""
    if rel in UNIT_TESTED:
        unit = run(["cargo", "test", "-p", "prequel", "--lib"])
        if did_not_compile(unit):
            return "DOES NOT COMPILE", "broken"
        if unit.returncode != 0:
            return "killed by unit test " + ", ".join(failing_tests(unit.stdout)[:2]), "killed"
    # the plain SBF build is enough to run the tests (anchor build also generates the IDL, which they do not use)
    build = run(["cargo", "build-sbf", "--manifest-path", "programs/prequel/Cargo.toml"])
    if build.returncode != 0:
        return "DOES NOT BUILD", "broken"
    for binary in TEST_BINARIES:
        result = run(["cargo", "test", "-p", "prequel", "--test", binary])
        if did_not_compile(result):
            return "DOES NOT COMPILE", "broken"
        if result.returncode != 0:
            names = failing_tests(result.stdout)
            return f"killed in {binary}: " + (", ".join(names[:2]) if names else "the binary failed to run"), "killed"
    return "SURVIVED", "survived"


def guard():
    """Refuses to start if another sweep is running or the program sources are not exactly what is committed."""
    if LOCK.exists():
        other = LOCK.read_text().strip()
        if other.isdigit() and pathlib.Path(f"/proc/{other}").exists():
            sys.exit(f"another sweep is running (pid {other}); two sweeps would mutate the same files")
    dirty = run(["git", "status", "--porcelain", "--", "programs/prequel/src"]).stdout.strip()
    if dirty:
        sys.exit("programs/prequel/src has uncommitted changes; commit or stash them first (the sweep restores files from memory):\n" + dirty)
    LOCK.parent.mkdir(exist_ok=True)
    LOCK.write_text(str(os.getpid()))


def main():
    args = sys.argv[1:]
    check_only = "--check" in args
    only = [a for a in args if not a.startswith("--")]
    survived, broken, killed = [], [], []
    started = time.time()
    # SIGTERM must restore the mutated file too, so turn it into the same exception as Ctrl-C
    signal.signal(signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt()))
    if not check_only:
        guard()
        # With no mutation every test must pass, otherwise "killed" would only mean "the baseline is broken".
        verdict, outcome = judge("")
        print(f"baseline (no mutation): {'green' if outcome == 'survived' else verdict}", flush=True)
        if outcome != "survived":
            LOCK.unlink(missing_ok=True)
            sys.exit("the baseline is not green, so no verdict would mean anything")
    for mid, rel, old, new, note in M:
        if only and mid not in only:
            continue
        path = SRC / rel
        original = path.read_text()
        count = original.count(old)
        if count != 1:
            print(f"{mid:6} NOT APPLIED   ({count} matches in {rel}) {note}", flush=True)
            broken.append(mid)
            continue
        if check_only:
            print(f"{mid:6} applies       {note}")
            continue
        t0 = time.time()
        path.write_text(original.replace(old, new))
        try:
            verdict, outcome = judge(rel)
        finally:
            path.write_text(original)
        {"killed": killed, "survived": survived, "broken": broken}[outcome].append(mid)
        print(f"{mid:6} {verdict:78.78} {time.time() - t0:4.0f}s  {note}", flush=True)
    if not check_only:
        LOCK.unlink(missing_ok=True)
        run(["anchor", "build"])  # leave the real binary in place
        print(f"\nfinished in {(time.time() - started) / 60:.1f} min: {len(killed)} killed, survived: {survived or 'none'}, broken: {broken or 'none'}")
    sys.exit(1 if survived or broken else 0)


if __name__ == "__main__":
    main()
