use criterion::{Criterion, criterion_group, criterion_main};
use rand::prelude::*;
use simd_parse_int::{self, ParseResult};
use std::hint::black_box;
use std::io::Write;
use std::{iter, usize};

struct BenchCases {
    name: String,
    storage: Vec<u8>,
    lengths: Vec<usize>,
}

static POWERS_OF_TEN: [usize; 20] = [
    1,
    10,
    100,
    1000,
    10000,
    100000,
    1000000,
    10000000,
    100000000,
    1000000000,
    10000000000,
    100000000000,
    1000000000000,
    10000000000000,
    100000000000000,
    1000000000000000,
    10000000000000000,
    100000000000000000,
    1000000000000000000,
    10000000000000000000,
];

impl<'a> BenchCases {
    fn new(count: usize, digits: usize, zeros: usize) -> BenchCases {
        let mut cases = BenchCases {
            name: format!("count={},digits={},zeros={}", count, digits, zeros),
            storage: Vec::new(),
            lengths: Vec::with_capacity(count),
        };

        let min_value = if digits > 0 {
            POWERS_OF_TEN[digits - 1]
        } else {
            0
        };
        let max_value = if digits < 20 {
            POWERS_OF_TEN[digits]
        } else {
            usize::MAX
        };

        for _ in 0..count {
            let mut rng = rand::rng();
            let zero_count: usize = if zeros == 0 {
                0
            } else {
                rng.random_range(0..zeros)
            };
            let value: usize = rng.random_range(min_value..max_value);

            let old_len = cases.storage.len();
            cases.storage.extend(iter::repeat(0x30).take(zero_count));
            write!(&mut cases.storage, "{}", value).unwrap();

            cases.lengths.push(cases.storage.len() - old_len);
        }

        return cases;
    }

    fn iter(&'a self) -> BenchCasesIterator<'a> {
        BenchCasesIterator {
            cases: self,
            storage_index: 0,
            length_index: 0,
        }
    }
}

struct BenchCasesIterator<'a> {
    cases: &'a BenchCases,
    storage_index: usize,
    length_index: usize,
}

impl<'a> Iterator for BenchCasesIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.storage_index == self.cases.storage.len() {
            return None;
        }

        let len = self.cases.lengths[self.length_index];
        let slice = &self.cases.storage[self.storage_index..self.storage_index + len];

        self.storage_index += len;
        self.length_index += 1;

        Some(slice)
    }
}

#[inline]
#[cold]
fn cold_path() {}

fn parse_naive(x: &[u8]) -> Result<(usize, usize), ()> {
    let mut value: usize = 0;
    let mut len = 0;
    let mut actual_len = 0;
    let mut should_check_overflow = 0;

    for (i, byte) in x.iter().enumerate() {
        let c = byte.wrapping_sub(0x30);
        if c > 9 {
            break;
        }

        should_check_overflow |= (c != 0) as usize;
        actual_len += should_check_overflow;

        if actual_len >= 19 {
            cold_path();

            let Some(new_value) = value.checked_mul(10) else {
                cold_path();
                return Err(());
            };

            let Some(new_value) = new_value.checked_add(c.into()) else {
                cold_path();
                return Err(());
            };

            value = new_value;
        } else {
            value = value * 10 + c as usize;
        }

        len = i + 1;
    }

    Ok((value, len))
}

fn criterion_benchmark(c: &mut Criterion) {
    // setup here

    let cases = BenchCases::new(10, 19, 0);

    for case in cases.iter() {
        let ParseResult {
            value: parse_int_value,
            len,
        } = simd_parse_int::parse(case);
        let atoi_simd_value = atoi_simd::parse::<usize>(case).unwrap();
        let (naive_value, _) = parse_naive(case).unwrap();

        if len == 0 {
            panic!("error");
        }

        if atoi_simd_value != parse_int_value || atoi_simd_value != naive_value {
            println!(
                "atoi: {}, parse_int_simd: {}, naive: {}",
                atoi_simd_value, parse_int_value, naive_value
            );
            panic!("sanity check failed");
        }
    }

    c.bench_function("mov", |b| {
        b.iter(|| {
            for _ in cases.iter() {
                black_box(42);
            }
        })
    });

    c.bench_function("simd_parse_int", |b| {
        b.iter(|| {
            for case in cases.iter() {
                black_box(simd_parse_int::parse(case));
            }
        })
    });

    c.bench_function("atoi_simd", |b| {
        b.iter(|| {
            for case in cases.iter() {
                let _ = black_box(atoi_simd::parse::<usize>(case));
            }
        })
    });

    c.bench_function("naive", |b| {
        b.iter(|| {
            for case in cases.iter() {
                let _ = black_box(parse_naive(case));
            }
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
