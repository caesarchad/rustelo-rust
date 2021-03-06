use soros_drone::drone::{request_airdrop_transaction, run_local_drone};
use soros_sdk::hash::Hash;
use soros_sdk::message::Message;
use soros_sdk::pubkey::Pubkey;
use soros_sdk::signature::{Keypair, KeypairUtil};
use soros_sdk::system_instruction;
use soros_sdk::transaction::Transaction;
use std::sync::mpsc::channel;

#[test]
fn test_local_drone() {
    let keypair = Keypair::new();
    let to = Pubkey::new_rand();
    // let lamports = 50;
    let dif = 50;
    let blockhash = Hash::new(&to.as_ref());
    let create_instruction =
        // system_instruction::create_user_account(&keypair.pubkey(), &to, lamports);
        system_instruction::create_user_account(&keypair.pubkey(), &to, dif);
    let message = Message::new(vec![create_instruction]);
    let expected_tx = Transaction::new(&[&keypair], message, blockhash);

    let (sender, receiver) = channel();
    run_local_drone(keypair, sender, None);
    let drone_addr = receiver.recv().unwrap();

    // let result = request_airdrop_transaction(&drone_addr, &to, lamports, blockhash);
    let result = request_airdrop_transaction(&drone_addr, &to, dif, blockhash);
    assert_eq!(expected_tx, result.unwrap());
}
