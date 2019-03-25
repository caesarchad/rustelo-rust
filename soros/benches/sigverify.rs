#![feature(test)]

extern crate test;

use bitconch::packet::to_packets;
use bitconch::sigverify;
use bitconch::test_tx::test_tx;
use test::Bencher;

#[bench]
fn bench_sigverify(bencher: &mut Bencher) {
    let tx = test_tx();

    // generate packet vector
    let batches = to_packets(&vec![tx; 128]);

    // verify packets
    bencher.iter(|| {
        let _ans = sigverify::ed25519_verify(&batches);
    })
}