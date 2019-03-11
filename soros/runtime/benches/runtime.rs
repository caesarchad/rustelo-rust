#![feature(test)]

extern crate test;

use bitconch_runtime::*;
use test::Bencher;

#[bench]
fn bench_has_duplicates(bencher: &mut Bencher) {
    bencher.iter(|| {
        let data = test::black_box([1, 2, 3]);
        assert!(!has_duplicates(&data));
    })
}
