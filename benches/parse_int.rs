#![feature(test)]

extern crate test;

use test::Bencher;

#[bench]
fn bench_parse_int(b: &mut Bencher) {
    b.iter(|| ());
}
