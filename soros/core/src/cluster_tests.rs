use crate::blocktree::Blocktree;
/// Cluster independant integration tests
///
/// All tests must start from an entry point and a funding keypair and
/// discover the rest of the network.
use crate::cluster_info::FULLNODE_PORT_RANGE;
use crate::contact_info::ContactInfo;
use crate::entry::{Entry, EntrySlice};
use crate::gossip_service::discover;
use crate::poh_service::PohServiceConfig;
use soros_client::client::create_client;
use soros_sdk::hash::Hash;
use soros_sdk::signature::{Keypair, KeypairUtil};
use soros_sdk::system_transaction::SystemTransaction;
use soros_sdk::timing::{
    duration_as_ms, DEFAULT_SLOTS_PER_EPOCH, DEFAULT_TICKS_PER_SLOT, NUM_TICKS_PER_SECOND,
};
use std::thread::sleep;
use std::time::Duration;

const SLOT_MILLIS: u64 = (DEFAULT_TICKS_PER_SLOT * 1000) / NUM_TICKS_PER_SECOND;

/// Spend and verify from every node in the network
pub fn spend_and_verify_all_nodes(
    entry_point_info: &ContactInfo,
    funding_keypair: &Keypair,
    nodes: usize,
) {
    let cluster_nodes = discover(&entry_point_info.gossip, nodes).unwrap();
    assert!(cluster_nodes.len() >= nodes);
    for ingress_node in &cluster_nodes {
        let random_keypair = Keypair::new();
        let mut client = create_client(ingress_node.client_facing_addr(), FULLNODE_PORT_RANGE);
        let bal = client
            .poll_get_balance(&funding_keypair.pubkey())
            .expect("balance in source");
        assert!(bal > 0);
        let mut transaction = SystemTransaction::new_move(
            &funding_keypair,
            &random_keypair.pubkey(),
            1,
            client.get_recent_blockhash(),
            0,
        );
        let sig = client
            .retry_transfer(&funding_keypair, &mut transaction, 5)
            .unwrap();
        for validator in &cluster_nodes {
            let mut client = create_client(validator.client_facing_addr(), FULLNODE_PORT_RANGE);
            client.poll_for_signature(&sig).unwrap();
        }
    }
}

pub fn send_many_transactions(node: &ContactInfo, funding_keypair: &Keypair, num_txs: u64) {
    let mut client = create_client(node.client_facing_addr(), FULLNODE_PORT_RANGE);
    for _ in 0..num_txs {
        let random_keypair = Keypair::new();
        let bal = client
            .poll_get_balance(&funding_keypair.pubkey())
            .expect("balance in source");
        assert!(bal > 0);
        let mut transaction = SystemTransaction::new_move(
            &funding_keypair,
            &random_keypair.pubkey(),
            1,
            client.get_recent_blockhash(),
            0,
        );
        client
            .retry_transfer(&funding_keypair, &mut transaction, 5)
            .unwrap();
    }
}

pub fn fullnode_exit(entry_point_info: &ContactInfo, nodes: usize) {
    let cluster_nodes = discover(&entry_point_info.gossip, nodes).unwrap();
    assert!(cluster_nodes.len() >= nodes);
    for node in &cluster_nodes {
        let mut client = create_client(node.client_facing_addr(), FULLNODE_PORT_RANGE);
        assert!(client.fullnode_exit().unwrap());
    }
    sleep(Duration::from_millis(SLOT_MILLIS));
    for node in &cluster_nodes {
        let mut client = create_client(node.client_facing_addr(), FULLNODE_PORT_RANGE);
        assert!(client.fullnode_exit().is_err());
    }
}

pub fn verify_ledger_ticks(ledger_path: &str, ticks_per_slot: usize) {
    let ledger = Blocktree::open(ledger_path).unwrap();
    let zeroth_slot = ledger.get_slot_entries(0, 0, None).unwrap();
    let last_id = zeroth_slot.last().unwrap().hash;
    let next_slots = ledger.get_slots_since(&[0]).unwrap().remove(&0).unwrap();
    let mut pending_slots: Vec<_> = next_slots
        .into_iter()
        .map(|slot| (slot, 0, last_id))
        .collect();
    while !pending_slots.is_empty() {
        let (slot, parent_slot, last_id) = pending_slots.pop().unwrap();
        let next_slots = ledger
            .get_slots_since(&[slot])
            .unwrap()
            .remove(&slot)
            .unwrap();

        // If you're not the last slot, you should have a full set of ticks
        let should_verify_ticks = if !next_slots.is_empty() {
            Some((slot - parent_slot) as usize * ticks_per_slot)
        } else {
            None
        };

        let last_id = verify_slot_ticks(&ledger, slot, &last_id, should_verify_ticks);
        pending_slots.extend(
            next_slots
                .into_iter()
                .map(|child_slot| (child_slot, slot, last_id)),
        );
    }
}

pub fn sleep_n_epochs(
    num_epochs: f64,
    config: &PohServiceConfig,
    ticks_per_slot: u64,
    slots_per_epoch: u64,
) {
    let num_ticks_per_second = {
        match config {
            PohServiceConfig::Sleep(d) => (1000 / duration_as_ms(d)) as f64,
            _ => panic!("Unsuppported tick config for testing"),
        }
    };

    let num_ticks_to_sleep = num_epochs * ticks_per_slot as f64 * slots_per_epoch as f64;
    sleep(Duration::from_secs(
        ((num_ticks_to_sleep + num_ticks_per_second - 1.0) / num_ticks_per_second) as u64,
    ));
}

pub fn kill_entry_and_spend_and_verify_rest(
    entry_point_info: &ContactInfo,
    funding_keypair: &Keypair,
    nodes: usize,
) {
    soros_logger::setup();
    let cluster_nodes = discover(&entry_point_info.gossip, nodes).unwrap();
    assert!(cluster_nodes.len() >= nodes);
    let mut client = create_client(entry_point_info.client_facing_addr(), FULLNODE_PORT_RANGE);
    info!("sleeping for an epoch");
    sleep(Duration::from_millis(SLOT_MILLIS * DEFAULT_SLOTS_PER_EPOCH));
    info!("done sleeping for an epoch");
    info!("killing entry point");
    assert!(client.fullnode_exit().unwrap());
    info!("sleeping for a slot");
    sleep(Duration::from_millis(SLOT_MILLIS));
    info!("done sleeping for a slot");
    for ingress_node in &cluster_nodes {
        if ingress_node.id == entry_point_info.id {
            continue;
        }
        let random_keypair = Keypair::new();
        let mut client = create_client(ingress_node.client_facing_addr(), FULLNODE_PORT_RANGE);
        let bal = client
            .poll_get_balance(&funding_keypair.pubkey())
            .expect("balance in source");
        assert!(bal > 0);
        let mut transaction = SystemTransaction::new_move(
            &funding_keypair,
            &random_keypair.pubkey(),
            1,
            client.get_recent_blockhash(),
            0,
        );
        let sig = client
            .retry_transfer(&funding_keypair, &mut transaction, 5)
            .unwrap();
        for validator in &cluster_nodes {
            if validator.id == entry_point_info.id {
                continue;
            }
            let mut client = create_client(validator.client_facing_addr(), FULLNODE_PORT_RANGE);
            client.poll_for_signature(&sig).unwrap();
        }
    }
}

fn get_and_verify_slot_entries(blocktree: &Blocktree, slot: u64, last_entry: &Hash) -> Vec<Entry> {
    let entries = blocktree.get_slot_entries(slot, 0, None).unwrap();
    assert!(entries.verify(last_entry));
    entries
}

fn verify_slot_ticks(
    blocktree: &Blocktree,
    slot: u64,
    last_entry: &Hash,
    expected_num_ticks: Option<usize>,
) -> Hash {
    let entries = get_and_verify_slot_entries(blocktree, slot, last_entry);
    let num_ticks: usize = entries.iter().map(|entry| entry.is_tick() as usize).sum();
    if let Some(expected_num_ticks) = expected_num_ticks {
        assert_eq!(num_ticks, expected_num_ticks);
    }
    entries.last().unwrap().hash
}
