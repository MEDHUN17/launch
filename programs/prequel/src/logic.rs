//! Pure decision logic, free of account handling so it can be unit-tested exhaustively
//! (docs/PHASE1_DESIGN.md section 3). Every time check uses the *effective* status (decision Q-04): the stored
//! status is only a cache that cranks update.

use crate::constants::*;
use crate::error::PrequelError;
use crate::state::{CandidateStatus, EventStatus, Params};

type Res<T> = core::result::Result<T, PrequelError>;

/// Settings must sit inside the hard caps of D-013.
pub fn validate_params(p: &Params) -> Res<()> {
    if p.success_fee_bps > MAX_SUCCESS_FEE_BPS {
        return Err(PrequelError::SuccessFeeTooHigh);
    }
    let split =
        p.split_proposer_bps as u32 + p.split_coprop_bps as u32 + p.split_treasury_bps as u32;
    if split != BPS_DENOMINATOR {
        return Err(PrequelError::FeeSplitInvalid);
    }
    if !(MIN_DISPUTE_WINDOW..=MAX_DISPUTE_WINDOW).contains(&p.dispute_window) {
        return Err(PrequelError::DisputeWindowOutOfRange);
    }
    if !(MIN_DISPUTE_BOND..=MAX_DISPUTE_BOND).contains(&p.dispute_bond) {
        return Err(PrequelError::DisputeBondOutOfRange);
    }
    if !(MIN_RESOLUTION_DELAY..=MAX_RESOLUTION_DELAY).contains(&p.max_resolution_delay) {
        return Err(PrequelError::ResolutionDelayOutOfRange);
    }
    if !(MIN_LAUNCH_WINDOW..=MAX_LAUNCH_WINDOW).contains(&p.launch_window) {
        return Err(PrequelError::LaunchWindowOutOfRange);
    }
    if !(MIN_PROPOSAL_FEE..=MAX_PROPOSAL_FEE).contains(&p.proposal_fee) {
        return Err(PrequelError::ProposalFeeOutOfRange);
    }
    if p.min_commit < MIN_COMMIT_FLOOR {
        return Err(PrequelError::MinCommitTooLow);
    }
    if p.min_raise < p.min_commit || !(MIN_MIN_RAISE..=MAX_MIN_RAISE).contains(&p.min_raise) {
        return Err(PrequelError::MinRaiseOutOfRange);
    }
    if !(0..=MAX_PROPOSAL_CUTOFF).contains(&p.proposal_cutoff) {
        return Err(PrequelError::ProposalCutoffOutOfRange);
    }
    if p.max_commit_per_wallet != 0 && p.max_commit_per_wallet < p.min_commit {
        return Err(PrequelError::CommitCapTooLow);
    }
    if !(MIN_SLATE..=MAX_SLATE).contains(&p.max_candidates_per_outcome) {
        return Err(PrequelError::SlateSizeOutOfRange);
    }
    // The reserved bytes will later hold settings. Whatever a client sends today would be frozen into every event
    // and read as a setting tomorrow, so only zero is accepted.
    if p.reserved != [0u8; 32] {
        return Err(PrequelError::ParamsReservedNotZero);
    }
    Ok(())
}

pub fn validate_outcome_count(n: u8) -> Res<()> {
    if (MIN_OUTCOMES..=MAX_OUTCOMES).contains(&n) {
        Ok(())
    } else {
        Err(PrequelError::InvalidOutcomeCount)
    }
}

/// `window_open <= window_close <= event_start - 5 minutes`, the window closes in the future and lasts at most
/// 60 days, and `event_start <= expected_resolution <= event_start + 90 days`.
pub fn validate_event_times(
    now: i64,
    window_open: i64,
    window_close: i64,
    event_start: i64,
    expected_resolution: i64,
) -> Res<()> {
    let latest_resolution = event_start
        .checked_add(MAX_RESOLUTION_AFTER_START)
        .ok_or(PrequelError::MathOverflow)?;
    let latest_close = event_start
        .checked_sub(MIN_CLOSE_TO_START_GAP)
        .ok_or(PrequelError::MathOverflow)?;
    let length = window_close
        .checked_sub(window_open)
        .ok_or(PrequelError::MathOverflow)?;
    if window_open <= window_close
        && window_close > now
        && window_close <= latest_close
        && length <= MAX_WINDOW_LENGTH
        && expected_resolution >= event_start
        && expected_resolution <= latest_resolution
    {
        Ok(())
    } else {
        Err(PrequelError::InvalidEventTimes)
    }
}

/// A stored `Open` reads as `Closed` once the close time has passed, whether or not anyone cranked
/// `close_window`. Every other status is returned as stored.
pub fn effective_event_status(stored: EventStatus, window_close: i64, now: i64) -> EventStatus {
    if stored == EventStatus::Open && now >= window_close {
        EventStatus::Closed
    } else {
        stored
    }
}

/// Commits and proposals are accepted only while `window_open <= now < window_close` and the event is open.
/// At `now == window_close` the window is closed.
pub fn check_accepts_commits(
    stored: EventStatus,
    window_open: i64,
    window_close: i64,
    now: i64,
) -> Res<()> {
    match effective_event_status(stored, window_close, now) {
        EventStatus::Open => {
            if now < window_open {
                Err(PrequelError::WindowNotOpen)
            } else {
                Ok(())
            }
        }
        EventStatus::Closed => Err(PrequelError::WindowClosed),
        _ => Err(PrequelError::EventStateInvalid),
    }
}

/// Proposals follow the commit window and, when `proposal_cutoff` is set, also stop that many seconds before close
/// so the admin has time to reject a bad one. Commits are unaffected.
pub fn check_accepts_proposals(
    stored: EventStatus,
    window_open: i64,
    window_close: i64,
    proposal_cutoff: i64,
    now: i64,
) -> Res<()> {
    check_accepts_commits(stored, window_open, window_close, now)?;
    if proposal_cutoff > 0 && now >= window_close.saturating_sub(proposal_cutoff) {
        return Err(PrequelError::ProposalCutoffPassed);
    }
    Ok(())
}

/// Validates a deposit against `min_commit` and the per-wallet cap and returns the wallet's new running total.
pub fn check_commit_amount(p: &Params, existing: u64, add: u64) -> Res<u64> {
    if add < p.min_commit {
        return Err(PrequelError::CommitBelowMinimum);
    }
    let total = existing
        .checked_add(add)
        .ok_or(PrequelError::MathOverflow)?;
    if p.max_commit_per_wallet != 0 && total > p.max_commit_per_wallet {
        return Err(PrequelError::CommitAboveCap);
    }
    Ok(total)
}

/// Phase 1 refund eligibility: rejected or refunding candidates, and pending ones once the event is voided.
/// Phases 2 and 3 add the losing-side and below-`min_raise` cases.
pub fn is_refundable(event: EventStatus, candidate: CandidateStatus) -> bool {
    match candidate {
        CandidateStatus::Rejected | CandidateStatus::Refunding => true,
        CandidateStatus::Pending => event == EventStatus::Voided,
        CandidateStatus::Born => false,
    }
}

pub fn validate_name(name: &str) -> Res<()> {
    if name.trim().is_empty()
        || name.len() > MAX_NAME_LEN
        || name.chars().any(|c| c.is_control() || is_format_char(c))
    {
        return Err(PrequelError::NameInvalid);
    }
    Ok(())
}

/// Unicode format characters (category Cf) plus the line and paragraph separators. They are invisible, and some
/// reorder the text around them (the bidi overrides U+202A to U+202E and isolates U+2066 to U+2069), so a name
/// containing one can display as something else. Zero-width joiners go too, which costs a few multi-part emoji;
/// the Canon service applies fuller checks (normalization, homoglyphs) before it signs anything.
fn is_format_char(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

pub fn validate_uri(uri: &str) -> Res<()> {
    // Printable ASCII only (no spaces, no control characters): anything else must be percent-encoded. The scheme
    // must be one of URI_SCHEMES and something must follow it.
    let printable = uri.bytes().all(|b| (0x21..=0x7e).contains(&b));
    let scheme_ok = URI_SCHEMES
        .iter()
        .any(|s| uri.len() > s.len() && uri.starts_with(s));
    if uri.len() > MAX_URI_LEN || !printable || !scheme_ok {
        return Err(PrequelError::UriInvalid);
    }
    Ok(())
}

pub fn validate_external_id(id: &str) -> Res<()> {
    if id.is_empty() || id.len() > MAX_EXTERNAL_ID_LEN {
        return Err(PrequelError::ExternalIdInvalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use EventStatus::*;

    fn code(e: PrequelError) -> u32 {
        e.into()
    }
    fn err<T: core::fmt::Debug>(r: Res<T>) -> u32 {
        code(r.unwrap_err())
    }

    #[test]
    fn spec_defaults_are_inside_the_caps() {
        assert!(validate_params(&Params::spec_defaults()).is_ok());
    }

    #[test]
    fn each_cap_is_enforced_at_its_edges() {
        let d = Params::spec_defaults();
        let bad = |f: fn(&mut Params), e: PrequelError| {
            let mut p = d;
            f(&mut p);
            assert_eq!(err(validate_params(&p)), code(e));
        };
        bad(
            |p| p.success_fee_bps = MAX_SUCCESS_FEE_BPS + 1,
            PrequelError::SuccessFeeTooHigh,
        );
        bad(|p| p.split_treasury_bps += 1, PrequelError::FeeSplitInvalid);
        bad(
            |p| p.dispute_window = MIN_DISPUTE_WINDOW - 1,
            PrequelError::DisputeWindowOutOfRange,
        );
        bad(
            |p| p.dispute_window = MAX_DISPUTE_WINDOW + 1,
            PrequelError::DisputeWindowOutOfRange,
        );
        bad(
            |p| p.dispute_bond = MIN_DISPUTE_BOND - 1,
            PrequelError::DisputeBondOutOfRange,
        );
        bad(
            |p| p.dispute_bond = MAX_DISPUTE_BOND + 1,
            PrequelError::DisputeBondOutOfRange,
        );
        bad(
            |p| p.max_resolution_delay = MIN_RESOLUTION_DELAY - 1,
            PrequelError::ResolutionDelayOutOfRange,
        );
        bad(
            |p| p.max_resolution_delay = MAX_RESOLUTION_DELAY + 1,
            PrequelError::ResolutionDelayOutOfRange,
        );
        bad(
            |p| p.launch_window = MIN_LAUNCH_WINDOW - 1,
            PrequelError::LaunchWindowOutOfRange,
        );
        bad(
            |p| p.launch_window = MAX_LAUNCH_WINDOW + 1,
            PrequelError::LaunchWindowOutOfRange,
        );
        bad(
            |p| p.proposal_fee = MIN_PROPOSAL_FEE - 1,
            PrequelError::ProposalFeeOutOfRange,
        );
        bad(
            |p| p.proposal_fee = MAX_PROPOSAL_FEE + 1,
            PrequelError::ProposalFeeOutOfRange,
        );
        bad(
            |p| p.min_commit = MIN_COMMIT_FLOOR - 1,
            PrequelError::MinCommitTooLow,
        );
        bad(
            |p| p.min_raise = p.min_commit - 1,
            PrequelError::MinRaiseOutOfRange,
        );
        bad(
            |p| p.min_raise = MAX_MIN_RAISE + 1,
            PrequelError::MinRaiseOutOfRange,
        );
        bad(
            |p| p.max_commit_per_wallet = p.min_commit - 1,
            PrequelError::CommitCapTooLow,
        );
        bad(
            |p| p.max_candidates_per_outcome = 0,
            PrequelError::SlateSizeOutOfRange,
        );
        bad(
            |p| p.max_candidates_per_outcome = MAX_SLATE + 1,
            PrequelError::SlateSizeOutOfRange,
        );
        // the edges themselves are allowed
        let mut p = d;
        p.success_fee_bps = MAX_SUCCESS_FEE_BPS;
        p.dispute_window = MIN_DISPUTE_WINDOW;
        p.max_resolution_delay = MAX_RESOLUTION_DELAY;
        p.max_commit_per_wallet = 0;
        p.max_candidates_per_outcome = MAX_SLATE;
        assert!(validate_params(&p).is_ok());
    }

    #[test]
    fn event_times() {
        let now = 1_000;
        let ok = |o, c, s, r| validate_event_times(now, o, c, s, r);
        assert!(ok(0, 2_000, 5_000, 6_000).is_ok());
        assert!(
            ok(2_000, 2_000, 5_000, 5_000).is_ok(),
            "open == close and resolution == start are allowed"
        );
        assert!(ok(3_000, 2_000, 5_000, 6_000).is_err(), "open after close");
        assert!(
            ok(0, now, 5_000, 6_000).is_err(),
            "close must be in the future"
        );
        assert!(
            ok(0, 5_000, 5_000, 6_000).is_err(),
            "close must be before the start"
        );
        assert!(
            ok(0, 2_000, 5_000, 4_999).is_err(),
            "resolution before start"
        );
        assert!(ok(0, 2_000, 5_000, 5_000 + MAX_RESOLUTION_AFTER_START).is_ok());
        assert!(ok(0, 2_000, 5_000, 5_000 + MAX_RESOLUTION_AFTER_START + 1).is_err());
        assert!(
            validate_event_times(now, 0, 2_000, i64::MAX, i64::MAX).is_err(),
            "overflow is an error, not a panic"
        );
    }

    #[test]
    fn effective_status_reads_open_as_closed_at_the_boundary() {
        assert_eq!(effective_event_status(Open, 100, 99), Open);
        assert_eq!(effective_event_status(Open, 100, 100), Closed);
        assert_eq!(effective_event_status(Open, 100, 101), Closed);
        for s in [Closed, ResolutionProposed, Disputed, Resolved, Voided] {
            assert_eq!(
                effective_event_status(s, 100, 1_000),
                s,
                "only Open is promoted"
            );
        }
    }

    #[test]
    fn commits_are_accepted_strictly_inside_the_window() {
        let (open, close) = (100, 200);
        assert_eq!(
            err(check_accepts_commits(Open, open, close, 99)),
            code(PrequelError::WindowNotOpen)
        );
        assert!(check_accepts_commits(Open, open, close, 100).is_ok());
        assert!(check_accepts_commits(Open, open, close, 199).is_ok());
        assert_eq!(
            err(check_accepts_commits(Open, open, close, 200)),
            code(PrequelError::WindowClosed)
        );
        assert_eq!(
            err(check_accepts_commits(Open, open, close, 201)),
            code(PrequelError::WindowClosed)
        );
        assert_eq!(
            err(check_accepts_commits(Closed, open, close, 150)),
            code(PrequelError::WindowClosed)
        );
        for s in [ResolutionProposed, Disputed, Resolved, Voided] {
            assert_eq!(
                err(check_accepts_commits(s, open, close, 150)),
                code(PrequelError::EventStateInvalid)
            );
        }
    }

    #[test]
    fn commit_amounts_and_caps() {
        let p = Params {
            min_commit: 50,
            max_commit_per_wallet: 200,
            ..Params::spec_defaults()
        };
        assert_eq!(
            err(check_commit_amount(&p, 0, 49)),
            code(PrequelError::CommitBelowMinimum)
        );
        assert_eq!(check_commit_amount(&p, 0, 50).unwrap(), 50);
        assert_eq!(check_commit_amount(&p, 150, 50).unwrap(), 200);
        assert_eq!(
            err(check_commit_amount(&p, 150, 51)),
            code(PrequelError::CommitAboveCap)
        );
        assert_eq!(
            err(check_commit_amount(&p, u64::MAX, 50)),
            code(PrequelError::MathOverflow)
        );
        let uncapped = Params {
            max_commit_per_wallet: 0,
            ..p
        };
        assert_eq!(
            check_commit_amount(&uncapped, u64::MAX - 50, 50).unwrap(),
            u64::MAX
        );
    }

    #[test]
    fn refundability() {
        use CandidateStatus::*;
        assert!(is_refundable(Voided, Pending));
        assert!(!is_refundable(Open, Pending));
        assert!(!is_refundable(Closed, Pending));
        assert!(
            !is_refundable(Resolved, Pending),
            "Phase 2/3 decide the losing side"
        );
        assert!(is_refundable(Open, Rejected));
        assert!(is_refundable(Closed, Refunding));
        assert!(!is_refundable(Voided, Born));
    }

    #[test]
    fn text_fields() {
        assert!(validate_name("Victory Coin").is_ok());
        assert!(validate_name("").is_err() && validate_name("   ").is_err());
        assert!(validate_name(&"a".repeat(32)).is_ok() && validate_name(&"a".repeat(33)).is_err());
        assert!(
            validate_name(&"\u{00e9}".repeat(16)).is_ok(),
            "32 bytes of 2-byte characters"
        );
        assert!(
            validate_name(&"\u{00e9}".repeat(17)).is_err(),
            "limits are in bytes"
        );
        assert!(validate_name("a\nb").is_err() && validate_name("a\u{0}b").is_err());
        assert!(validate_uri("ipfs://bafy").is_ok() && validate_uri("").is_err());
        assert!(
            validate_uri(&format!("ipfs://{}", "u".repeat(121))).is_ok()
                && validate_uri(&format!("ipfs://{}", "u".repeat(122))).is_err(),
            "128 bytes including the scheme"
        );
        assert!(validate_external_id("KXNBA-1").is_ok() && validate_external_id("").is_err());
        assert!(validate_external_id(&"x".repeat(64)).is_ok());
        assert!(validate_external_id(&"x".repeat(65)).is_err());
        assert!(validate_outcome_count(1).is_err() && validate_outcome_count(2).is_ok());
        assert!(validate_outcome_count(8).is_ok() && validate_outcome_count(9).is_err());
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    use EventStatus::*;

    fn code(e: PrequelError) -> u32 {
        e.into()
    }

    #[test]
    fn min_raise_has_a_floor_and_the_cutoff_is_bounded() {
        let mut p = Params::spec_defaults();
        p.min_raise = MIN_MIN_RAISE - 1;
        assert_eq!(
            code(validate_params(&p).unwrap_err()),
            code(PrequelError::MinRaiseOutOfRange)
        );
        p.min_raise = MIN_MIN_RAISE;
        assert!(validate_params(&p).is_ok());
        p.proposal_cutoff = -1;
        assert_eq!(
            code(validate_params(&p).unwrap_err()),
            code(PrequelError::ProposalCutoffOutOfRange)
        );
        p.proposal_cutoff = MAX_PROPOSAL_CUTOFF + 1;
        assert_eq!(
            code(validate_params(&p).unwrap_err()),
            code(PrequelError::ProposalCutoffOutOfRange)
        );
        p.proposal_cutoff = MAX_PROPOSAL_CUTOFF;
        assert!(validate_params(&p).is_ok());
    }

    #[test]
    fn close_to_start_gap_and_window_length() {
        let now = 1_000;
        let start = 100_000;
        assert!(
            validate_event_times(now, 0, start - MIN_CLOSE_TO_START_GAP, start, start).is_ok(),
            "exactly the gap"
        );
        assert!(
            validate_event_times(now, 0, start - MIN_CLOSE_TO_START_GAP + 1, start, start).is_err(),
            "one second short"
        );
        let long_start = 10 * DAY * 100;
        assert!(validate_event_times(
            now,
            2_000,
            2_000 + MAX_WINDOW_LENGTH,
            long_start,
            long_start
        )
        .is_ok());
        assert!(
            validate_event_times(
                now,
                2_000,
                2_001 + MAX_WINDOW_LENGTH,
                long_start,
                long_start
            )
            .is_err(),
            "window over 60 days"
        );
        assert!(
            validate_event_times(now, 0, 2_000, i64::MIN, i64::MAX).is_err(),
            "extreme values are errors, not panics"
        );
    }

    #[test]
    fn proposals_stop_before_commits_when_a_cutoff_is_set() {
        let (open, close) = (100, 1_000);
        assert!(
            check_accepts_proposals(Open, open, close, 0, 999).is_ok(),
            "no cutoff: runs until close"
        );
        assert!(check_accepts_proposals(Open, open, close, 200, 799).is_ok());
        assert_eq!(
            code(check_accepts_proposals(Open, open, close, 200, 800).unwrap_err()),
            code(PrequelError::ProposalCutoffPassed)
        );
        assert!(
            check_accepts_commits(Open, open, close, 800).is_ok(),
            "commits are still open"
        );
        assert_eq!(
            code(check_accepts_proposals(Open, open, close, 200, 1_000).unwrap_err()),
            code(PrequelError::WindowClosed)
        );
        assert_eq!(
            code(check_accepts_proposals(Open, open, close, 200, 99).unwrap_err()),
            code(PrequelError::WindowNotOpen)
        );
    }

    #[test]
    fn uris_are_printable_ascii_only() {
        assert!(
            validate_uri("ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
                .is_ok()
        );
        for bad in [
            "ipfs://a b",
            "ipfs://caf\u{00e9}",
            "a\u{0}b",
            "a\nb",
            "\u{7f}x",
        ] {
            assert!(validate_uri(bad).is_err(), "{bad:?}");
        }
    }
}

/// Added after the second review: both sides of every bound, the reserved bytes, and the text rules.
#[cfg(test)]
mod followup_tests {
    use super::*;

    fn code(e: PrequelError) -> u32 {
        e.into()
    }

    fn accepts(label: &str, f: impl FnOnce(&mut Params)) {
        let mut p = Params::spec_defaults();
        f(&mut p);
        assert!(
            validate_params(&p).is_ok(),
            "{label} should be accepted: {:?}",
            validate_params(&p)
        );
    }

    fn rejects(label: &str, want: PrequelError, f: impl FnOnce(&mut Params)) {
        let mut p = Params::spec_defaults();
        f(&mut p);
        match validate_params(&p) {
            Err(e) => assert_eq!(code(e), code(want), "{label}"),
            Ok(()) => panic!("{label} should be rejected"),
        }
    }

    #[test]
    fn every_bound_is_inclusive_and_one_step_outside_is_refused() {
        use PrequelError as E;
        // success fee and fee split
        accepts("fee 0", |p| p.success_fee_bps = 0);
        accepts("fee at cap", |p| p.success_fee_bps = MAX_SUCCESS_FEE_BPS);
        rejects("fee over cap", E::SuccessFeeTooHigh, |p| {
            p.success_fee_bps = MAX_SUCCESS_FEE_BPS + 1
        });
        rejects("split one short", E::FeeSplitInvalid, |p| {
            p.split_treasury_bps -= 1
        });
        rejects("split one over", E::FeeSplitInvalid, |p| {
            p.split_treasury_bps += 1
        });
        rejects("split cannot wrap", E::FeeSplitInvalid, |p| {
            p.split_proposer_bps = u16::MAX;
            p.split_coprop_bps = u16::MAX;
            p.split_treasury_bps = u16::MAX;
        });
        accepts("split moved between slices", |p| {
            p.split_proposer_bps = 10_000;
            p.split_coprop_bps = 0;
            p.split_treasury_bps = 0;
        });
        // dispute window, resolution delay, launch window
        accepts("dispute window min", |p| {
            p.dispute_window = MIN_DISPUTE_WINDOW
        });
        accepts("dispute window max", |p| {
            p.dispute_window = MAX_DISPUTE_WINDOW
        });
        rejects("dispute window below", E::DisputeWindowOutOfRange, |p| {
            p.dispute_window = MIN_DISPUTE_WINDOW - 1
        });
        rejects("dispute window above", E::DisputeWindowOutOfRange, |p| {
            p.dispute_window = MAX_DISPUTE_WINDOW + 1
        });
        accepts("resolution delay min", |p| {
            p.max_resolution_delay = MIN_RESOLUTION_DELAY
        });
        accepts("resolution delay max", |p| {
            p.max_resolution_delay = MAX_RESOLUTION_DELAY
        });
        rejects(
            "resolution delay below",
            E::ResolutionDelayOutOfRange,
            |p| p.max_resolution_delay = MIN_RESOLUTION_DELAY - 1,
        );
        rejects(
            "resolution delay above",
            E::ResolutionDelayOutOfRange,
            |p| p.max_resolution_delay = MAX_RESOLUTION_DELAY + 1,
        );
        accepts("launch window min", |p| p.launch_window = MIN_LAUNCH_WINDOW);
        accepts("launch window max", |p| p.launch_window = MAX_LAUNCH_WINDOW);
        rejects("launch window below", E::LaunchWindowOutOfRange, |p| {
            p.launch_window = MIN_LAUNCH_WINDOW - 1
        });
        rejects("launch window above", E::LaunchWindowOutOfRange, |p| {
            p.launch_window = MAX_LAUNCH_WINDOW + 1
        });
        // bond and fee
        accepts("bond min", |p| p.dispute_bond = MIN_DISPUTE_BOND);
        accepts("bond max", |p| p.dispute_bond = MAX_DISPUTE_BOND);
        rejects("bond below", E::DisputeBondOutOfRange, |p| {
            p.dispute_bond = MIN_DISPUTE_BOND - 1
        });
        rejects("bond above", E::DisputeBondOutOfRange, |p| {
            p.dispute_bond = MAX_DISPUTE_BOND + 1
        });
        accepts("proposal fee min", |p| p.proposal_fee = MIN_PROPOSAL_FEE);
        accepts("proposal fee max", |p| p.proposal_fee = MAX_PROPOSAL_FEE);
        rejects("proposal fee below", E::ProposalFeeOutOfRange, |p| {
            p.proposal_fee = MIN_PROPOSAL_FEE - 1
        });
        rejects("proposal fee above", E::ProposalFeeOutOfRange, |p| {
            p.proposal_fee = MAX_PROPOSAL_FEE + 1
        });
        // min_commit, min_raise, per-wallet cap
        accepts("min_commit at the floor", |p| {
            p.min_commit = MIN_COMMIT_FLOOR
        });
        rejects("min_commit below the floor", E::MinCommitTooLow, |p| {
            p.min_commit = MIN_COMMIT_FLOOR - 1
        });
        accepts("min_raise at its floor", |p| p.min_raise = MIN_MIN_RAISE);
        accepts("min_raise at its ceiling", |p| p.min_raise = MAX_MIN_RAISE);
        rejects("min_raise below its floor", E::MinRaiseOutOfRange, |p| {
            p.min_raise = MIN_MIN_RAISE - 1
        });
        rejects("min_raise above its ceiling", E::MinRaiseOutOfRange, |p| {
            p.min_raise = MAX_MIN_RAISE + 1
        });
        accepts("min_raise equal to min_commit", |p| {
            p.min_commit = MIN_MIN_RAISE;
            p.min_raise = MIN_MIN_RAISE;
        });
        rejects("min_raise under min_commit", E::MinRaiseOutOfRange, |p| {
            p.min_commit = 2 * MIN_MIN_RAISE;
            p.min_raise = 2 * MIN_MIN_RAISE - 1;
        });
        accepts("no wallet cap", |p| p.max_commit_per_wallet = 0);
        accepts("wallet cap equal to min_commit", |p| {
            p.max_commit_per_wallet = p.min_commit
        });
        rejects("wallet cap under min_commit", E::CommitCapTooLow, |p| {
            p.max_commit_per_wallet = p.min_commit - 1
        });
        // slate and cutoff
        accepts("slate of 1", |p| p.max_candidates_per_outcome = MIN_SLATE);
        accepts("slate at the maximum", |p| {
            p.max_candidates_per_outcome = MAX_SLATE
        });
        rejects("slate of 0", E::SlateSizeOutOfRange, |p| {
            p.max_candidates_per_outcome = 0
        });
        rejects("slate over the maximum", E::SlateSizeOutOfRange, |p| {
            p.max_candidates_per_outcome = MAX_SLATE + 1
        });
        accepts("no cutoff", |p| p.proposal_cutoff = 0);
        accepts("cutoff at the maximum", |p| {
            p.proposal_cutoff = MAX_PROPOSAL_CUTOFF
        });
        rejects(
            "cutoff over the maximum",
            E::ProposalCutoffOutOfRange,
            |p| p.proposal_cutoff = MAX_PROPOSAL_CUTOFF + 1,
        );
        rejects("negative cutoff", E::ProposalCutoffOutOfRange, |p| {
            p.proposal_cutoff = -1
        });
    }

    #[test]
    fn reserved_settings_bytes_must_be_zero() {
        for i in [0usize, 15, 31] {
            rejects(
                "non-zero reserved byte",
                PrequelError::ParamsReservedNotZero,
                |p| p.reserved[i] = 1,
            );
        }
        accepts("all-zero reserved", |p| p.reserved = [0; 32]);
    }

    #[test]
    fn names_reject_invisible_and_reordering_characters() {
        for bad in [
            "a\u{200B}b",     // zero-width space
            "a\u{200D}b",     // zero-width joiner
            "\u{202E}evil",   // right-to-left override
            "x\u{2066}y",     // left-to-right isolate
            "ab\u{FEFF}",     // byte order mark
            "soft\u{00AD}hy", // soft hyphen
            "a\u{2028}b",     // line separator
            "\u{061C}x",      // Arabic letter mark
            "tag\u{E0067}",   // tag character
        ] {
            assert_eq!(
                validate_name(bad).map_err(code),
                Err(code(PrequelError::NameInvalid)),
                "{bad:?}"
            );
        }
        for good in [
            "Pepe",
            "P\u{e9}p\u{e9}",
            "\u{65e5}\u{672c}\u{8a9e}",
            "Coin \u{1f438}",
            "A-B_C.1",
            "\u{dc}n\u{ef}code",
        ] {
            assert!(validate_name(good).is_ok(), "{good:?}");
        }
    }

    #[test]
    fn uris_need_an_allowed_scheme_and_something_after_it() {
        let cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        assert!(validate_uri(&format!("ipfs://{cid}")).is_ok());
        assert!(validate_uri(&format!("https://gateway.example/ipfs/{cid}")).is_ok());
        for bad in [
            "",
            "ipfs://",
            "https://",
            "http://example.com/x",
            "javascript:alert(1)",
            "data:text/plain;base64,AAAA",
            "IPFS://x",
            "ftp://x",
            "//example.com/x",
            "ipfs:/x",
            "bafybei",
        ] {
            assert_eq!(
                validate_uri(bad).map_err(code),
                Err(code(PrequelError::UriInvalid)),
                "{bad:?}"
            );
        }
        assert!(validate_uri(&format!("ipfs://{}", "u".repeat(121))).is_ok());
        assert!(validate_uri(&format!("ipfs://{}", "u".repeat(122))).is_err());
    }
}
