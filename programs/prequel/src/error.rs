use anchor_lang::prelude::*;

#[error_code]
pub enum PrequelError {
    // authority
    #[msg("Only the admin can do this")]
    NotAdmin,
    #[msg("Only the program's upgrade authority can initialize the config")]
    NotDeployer,
    #[msg("A required key is the default (all-zero) address")]
    InvalidKey,
    // settings (D-013 caps)
    #[msg("success_fee_bps is above the hard cap")]
    SuccessFeeTooHigh,
    #[msg("Fee split must sum to 10,000 bps")]
    FeeSplitInvalid,
    #[msg("dispute_window is outside its allowed range")]
    DisputeWindowOutOfRange,
    #[msg("dispute_bond is outside its allowed range")]
    DisputeBondOutOfRange,
    #[msg("max_resolution_delay is outside its allowed range")]
    ResolutionDelayOutOfRange,
    #[msg("launch_window is outside its allowed range")]
    LaunchWindowOutOfRange,
    #[msg("proposal_fee is outside its allowed range")]
    ProposalFeeOutOfRange,
    #[msg("min_commit is below the floor")]
    MinCommitTooLow,
    #[msg("min_raise is outside its allowed range")]
    MinRaiseOutOfRange,
    #[msg("max_commit_per_wallet is below min_commit")]
    CommitCapTooLow,
    #[msg("proposal_cutoff is outside its allowed range")]
    ProposalCutoffOutOfRange,
    #[msg("max_candidates_per_outcome is outside its allowed range")]
    SlateSizeOutOfRange,
    // events
    #[msg("Event times are inconsistent or in the past")]
    InvalidEventTimes,
    #[msg("Only source 0 (Kalshi) is supported")]
    UnsupportedSource,
    #[msg("outcome_count must be between 2 and 8")]
    InvalidOutcomeCount,
    #[msg("external id is empty or too long")]
    ExternalIdInvalid,
    #[msg("external_id_hash does not match the external id")]
    ExternalIdHashMismatch,
    // gating
    #[msg("New proposals are paused")]
    ProposalsPaused,
    #[msg("New commits are paused")]
    CommitsPaused,
    #[msg("The commit window is not open yet")]
    WindowNotOpen,
    #[msg("The commit window is closed")]
    WindowClosed,
    #[msg("New proposals have stopped for this event; commits are still open")]
    ProposalCutoffPassed,
    #[msg("The commit window has not closed yet")]
    WindowNotClosedYet,
    #[msg("The event is not in a state that allows this")]
    EventStateInvalid,
    // candidates
    #[msg("Outcome index is out of range for this event")]
    InvalidOutcome,
    #[msg("Name is empty, too long, or contains control or invisible formatting characters")]
    NameInvalid,
    #[msg("Metadata URI must start with ipfs:// or https://, be printable ASCII without spaces, and fit 128 bytes")]
    UriInvalid,
    #[msg("Ticker must be 2 to 10 characters")]
    TickerLength,
    #[msg("Ticker may only contain ASCII letters and digits")]
    TickerCharacters,
    #[msg("canon_hash does not match the ticker")]
    CanonHashMismatch,
    #[msg("This outcome already has the maximum number of candidates")]
    SlateFull,
    #[msg("The candidate is not pending")]
    CandidateNotPending,
    #[msg("The candidate cannot be rejected now")]
    CandidateNotRejectable,
    #[msg("The proposer is already in; use commit instead")]
    ProposerCannotCoPropose,
    // money
    #[msg("Amount is below min_commit")]
    CommitBelowMinimum,
    #[msg("Amount would exceed max_commit_per_wallet")]
    CommitAboveCap,
    #[msg("Nothing to refund yet: the candidate is not refundable")]
    NotRefundable,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("The vault does not hold enough to pay this out")]
    VaultUnderfunded,
    #[msg("Accounts do not match the expected addresses")]
    AccountMismatch,
    // attestation (D-014)
    #[msg("An attestation is required for this event")]
    AttestationMissing,
    #[msg("The attestation is malformed or does not match the proposal")]
    AttestationInvalid,
    #[msg("The attestation has expired")]
    AttestationExpired,
    // added after the second review; appended so that existing error codes keep their numbers
    #[msg("Reserved settings bytes must be zero")]
    ParamsReservedNotZero,
    #[msg("The event was created under a different ticker table version")]
    CanonVersionMismatch,
    #[msg("This wallet co-proposed: pass its co-proposer share so that it is closed with the commitment")]
    CoProposerShareRequired,
}
