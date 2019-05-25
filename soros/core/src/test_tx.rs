use soros_sdk::hash::Hash;
use soros_sdk::signature::{Keypair, KeypairUtil};
use soros_sdk::system_transaction::SystemTransaction;
use soros_sdk::transaction::Transaction;

pub fn test_tx() -> Transaction {
    let keypair1 = Keypair::new();
    let pubkey1 = keypair1.pubkey();
    let zero = Hash::default();
    SystemTransaction::new_account(&keypair1, &pubkey1, 42, zero, 0)
}
