mod common;
use common::*;
use solana_signer::Signer;

#[test]
fn harness_deploys_with_a_known_upgrade_authority_and_warps_time() {
    let mut env = Env::new();
    assert_eq!(env.now(), T0);
    env.warp_to(T0 + 61);
    assert_eq!(env.now(), T0 + 61);
    let pd = env.svm.get_account(&program_data_address()).unwrap();
    assert_eq!(&pd.data[0..4], &3u32.to_le_bytes(), "ProgramData tag");
    assert_eq!(pd.data[12], 1, "Some(authority)");
    assert_eq!(&pd.data[13..45], env.deployer.pubkey().as_ref());
    assert!(env.lamports(&env.admin.pubkey()) >= 100 * SOL);
    assert_ne!(config_pda(), event_pda(0));
}

#[test]
fn ed25519_instruction_has_the_precompile_layout() {
    let k = solana_keypair::Keypair::new();
    let ix = ed25519_ix(&k, b"hello");
    assert_eq!(ix.program_id, ED25519_PROGRAM);
    assert_eq!(ix.data.len(), 2 + 14 + 32 + 64 + 5);
    assert_eq!(&ix.data[0..2], &[1, 0]);
    assert_eq!(&ix.data[4..6], &u16::MAX.to_le_bytes());
}
