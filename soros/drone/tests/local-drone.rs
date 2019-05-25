use soros_drone::drone::{request_airdrop_transaction, run_local_drone};
use soros_sdk::hash::Hash;
use soros_sdk::signature::{Keypair, KeypairUtil};
use soros_sdk::system_instruction::SystemInstruction;
use soros_sdk::system_program;
use soros_sdk::transaction::Transaction;
use std::sync::mpsc::channel;

#[test]
fn test_local_drone() {
    let keypair = Keypair::new();
    let to = Keypair::new().pubkey();
    let lamports = 50;
    let blockhash = Hash::new(&to.as_ref());
    let expected_instruction = SystemInstruction::CreateAccount {
        lamports,
        space: 0,
        program_id: system_program::id(),
    };
    let mut expected_tx = Transaction::new(
        &keypair,
        &[to],
        &system_program::id(),
        &expected_instruction,
        blockhash,
        0,
    );
    expected_tx.sign(&[&keypair], blockhash);

    let (sender, receiver) = channel();
    run_local_drone(keypair, sender);
    let drone_addr = receiver.recv().unwrap();

    let result = request_airdrop_transaction(&drone_addr, &to, lamports, blockhash);
    assert_eq!(expected_tx, result.unwrap());
}
