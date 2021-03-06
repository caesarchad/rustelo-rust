//! The `write_stage` module implements the TPU's write stage. It
//! writes entries to the given writer, which is typically a file or
//! stdout, and then sends the Entry to its output channel.

use crate::tx_vault::Bank;
use buffett_metrics::counter::Counter;
use crate::crdt::Crdt;
use crate::entry::Entry;
use crate::ledger::{Block, LedgerWriter};
use log::Level;
use crate::result::{Error, Result};
use crate::service::Service;
use buffett_crypto::signature::Keypair;
use std::cmp;
use std::net::UdpSocket;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, RwLock};
use std::thread::{self, Builder, JoinHandle};
use std::time::{Duration, Instant};
use crate::streamer::responder;
use buffett_timing::timing::{duration_in_milliseconds, duration_in_seconds};
use crate::vote_stage::send_leader_vote;
use buffett_metrics::sub_new_counter_info;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WriteStageReturnType {
    LeaderRotation,
    ChannelDisconnected,
}

pub struct WriteStage {
    thread_hdls: Vec<JoinHandle<()>>,
    write_thread: JoinHandle<WriteStageReturnType>,
}

impl WriteStage {
    // Given a vector of potential new entries to write, return as many as we can
    // fit before we hit the entry height for leader rotation. Also return a boolean
    // reflecting whether we actually hit an entry height for leader rotation.
    fn find_leader_rotation_index(
        crdt: &Arc<RwLock<Crdt>>,
        leader_rotation_interval: u64,
        entry_height: u64,
        mut new_entries: Vec<Entry>,
    ) -> (Vec<Entry>, bool) {
        let new_entries_length = new_entries.len();

        // i is the number of entries to take
        let mut i = 0;
        let mut is_leader_rotation = false;

        loop {
            if (entry_height + i as u64) % leader_rotation_interval == 0 {
                let rcrdt = crdt.read().unwrap();
                let my_id = rcrdt.my_data().id;
                let next_leader = rcrdt.get_scheduled_leader(entry_height + i as u64);
                if next_leader != Some(my_id) {
                    is_leader_rotation = true;
                    break;
                }
            }

            if i == new_entries_length {
                break;
            }

            // Find out how many more entries we can squeeze in until the next leader
            // rotation
            let entries_until_leader_rotation =
                leader_rotation_interval - (entry_height % leader_rotation_interval);

            // Check the next leader rotation height entries in new_entries, or
            // if the new_entries doesnt have that many entries remaining,
            // just check the rest of the new_entries_vector
            i += cmp::min(
                entries_until_leader_rotation as usize,
                new_entries_length - i,
            );
        }

        new_entries.truncate(i as usize);

        (new_entries, is_leader_rotation)
    }

    /// Process any Entry items that have been published by the RecordStage.
    /// continuosly send entries out
    pub fn write_and_send_entries(
        crdt: &Arc<RwLock<Crdt>>,
        ledger_writer: &mut LedgerWriter,
        entry_sender: &Sender<Vec<Entry>>,
        entry_receiver: &Receiver<Vec<Entry>>,
        entry_height: &mut u64,
        leader_rotation_interval: u64,
    ) -> Result<()> {
        let mut ventries = Vec::new();
        let mut received_entries = entry_receiver.recv_timeout(Duration::new(1, 0))?;
        let now = Instant::now();
        let mut num_new_entries = 0;
        let mut num_txs = 0;

        loop {
            // Find out how many more entries we can squeeze in until the next leader
            // rotation
            let (new_entries, is_leader_rotation) = Self::find_leader_rotation_index(
                crdt,
                leader_rotation_interval,
                *entry_height + num_new_entries as u64,
                received_entries,
            );

            num_new_entries += new_entries.len();
            ventries.push(new_entries);

            if is_leader_rotation {
                break;
            }

            if let Ok(n) = entry_receiver.try_recv() {
                received_entries = n;
            } else {
                break;
            }
        }
        sub_new_counter_info!("write_stage-entries_received", num_new_entries);

        info!("{} entries written to ", num_new_entries);

        let mut entries_send_total = 0;
        let mut crdt_votes_total = 0;

        let start = Instant::now();
        for entries in ventries {
            for e in &entries {
                num_txs += e.transactions.len();
            }
            let crdt_votes_start = Instant::now();
            let votes = &entries.votes();
            crdt.write().unwrap().insert_votes(&votes);
            crdt_votes_total += duration_in_milliseconds(&crdt_votes_start.elapsed());

            ledger_writer.write_entries(entries.clone())?;
            // Once the entries have been written to the ledger, then we can
            // safely incement entry height
            *entry_height += entries.len() as u64;

            sub_new_counter_info!("write_stage-write_entries", entries.len());

            //TODO(anatoly): real stake based voting needs to change this
            //leader simply votes if the current set of validators have voted
            //on a valid last id

            trace!("New entries? {}", entries.len());
            let entries_send_start = Instant::now();
            if !entries.is_empty() {
                sub_new_counter_info!("write_stage-recv_vote", votes.len());
                sub_new_counter_info!("write_stage-entries_sent", entries.len());
                trace!("broadcasting {}", entries.len());
                entry_sender.send(entries)?;
            }

            entries_send_total += duration_in_milliseconds(&entries_send_start.elapsed());
        }
        sub_new_counter_info!(
            "write_stage-time_ms",
            duration_in_milliseconds(&now.elapsed()) as usize
        );
        info!("done write_stage txs: {} time {} ms txs/s: {} entries_send_total: {} crdt_votes_total: {}",
              num_txs, duration_in_milliseconds(&start.elapsed()),
              num_txs as f32 / duration_in_seconds(&start.elapsed()),
              entries_send_total,
              crdt_votes_total);

        Ok(())
    }

    /// Create a new WriteStage for writing and broadcasting entries.
    pub fn new(
        keypair: Arc<Keypair>,
        bank: Arc<Bank>,
        crdt: Arc<RwLock<Crdt>>,
        ledger_path: &str,
        entry_receiver: Receiver<Vec<Entry>>,
        entry_height: u64,
    ) -> (Self, Receiver<Vec<Entry>>) {
        let (vote_blob_sender, vote_blob_receiver) = channel();
        let send = UdpSocket::bind("0.0.0.0:0").expect("bind");
        let t_responder = responder(
            "write_stage_vote_sender",
            Arc::new(send),
            vote_blob_receiver,
        );
        let (entry_sender, entry_receiver_forward) = channel();
        let mut ledger_writer = LedgerWriter::recover(ledger_path).unwrap();

        let write_thread = Builder::new()
            .name("bitconch-writer".to_string())
            .spawn(move || {
                let mut last_vote = 0;
                let mut last_valid_validator_timestamp = 0;
                let id;
                let leader_rotation_interval;
                {
                    let rcrdt = crdt.read().unwrap();
                    id = rcrdt.id;
                    leader_rotation_interval = rcrdt.get_leader_rotation_interval();
                }
                let mut entry_height = entry_height;
                loop {
                    // Note that entry height is not zero indexed, it starts at 1, so the
                    // old leader is in power up to and including entry height
                    // n * leader_rotation_interval for some "n". Once we've forwarded
                    // that last block, check for the next scheduled leader.
                    if entry_height % (leader_rotation_interval as u64) == 0 {
                        let rcrdt = crdt.read().unwrap();
                        let my_id = rcrdt.my_data().id;
                        let scheduled_leader = rcrdt.get_scheduled_leader(entry_height);
                        drop(rcrdt);
                        match scheduled_leader {
                            Some(id) if id == my_id => (),
                            // If the leader stays in power for the next
                            // round as well, then we don't exit. Otherwise, exit.
                            _ => {
                                // When the broadcast stage has received the last blob, it
                                // will signal to close the fetch stage, which will in turn
                                // close down this write stage
                                return WriteStageReturnType::LeaderRotation;
                            }
                        }
                    }

                    if let Err(e) = Self::write_and_send_entries(
                        &crdt,
                        &mut ledger_writer,
                        &entry_sender,
                        &entry_receiver,
                        &mut entry_height,
                        leader_rotation_interval,
                    ) {
                        match e {
                            Error::RecvTimeoutError(RecvTimeoutError::Disconnected) => {
                                return WriteStageReturnType::ChannelDisconnected
                            }
                            Error::RecvTimeoutError(RecvTimeoutError::Timeout) => (),
                            _ => {
                                sub_new_counter_info!(
                                    "write_stage-write_and_send_entries-error",
                                    1
                                );
                                error!("{:?}", e);
                            }
                        }
                    };
                    if let Err(e) = send_leader_vote(
                        &id,
                        &keypair,
                        &bank,
                        &crdt,
                        &vote_blob_sender,
                        &mut last_vote,
                        &mut last_valid_validator_timestamp,
                    ) {
                        sub_new_counter_info!("write_stage-leader_vote-error", 1);
                        error!("{:?}", e);
                    }
                }
            }).unwrap();

        let thread_hdls = vec![t_responder];
        (
            WriteStage {
                write_thread,
                thread_hdls,
            },
            entry_receiver_forward,
        )
    }
}

impl Service for WriteStage {
    type JoinReturnType = WriteStageReturnType;

    fn join(self) -> thread::Result<WriteStageReturnType> {
        for thread_hdl in self.thread_hdls {
            thread_hdl.join()?;
        }

        self.write_thread.join()
    }
}

