//! Phase 0 pipeline check: load the built program into LiteSVM and call `ping`.
//! Needs `anchor build` first (the test reads target/deploy/prequel.so).

use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

#[test]
fn ping_succeeds_and_logs() {
    let program_id = prequel::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/prequel.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction::new_with_bytes(
        program_id,
        &prequel::instruction::Ping {}.data(),
        prequel::accounts::Ping {
            signer: payer.pubkey(),
        }
        .to_account_metas(None),
    );
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let meta = svm.send_transaction(tx).expect("ping should succeed");
    assert!(
        meta.logs.iter().any(|l| l.contains("prequel: pong")),
        "logs: {:?}",
        meta.logs
    );
}
