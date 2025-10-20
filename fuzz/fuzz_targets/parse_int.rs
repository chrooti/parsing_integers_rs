#![no_main]

use libfuzzer_sys::arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use simd_parse_int::ParseResult;
use simd_parse_int::parse;
use std::io::Write;

#[derive(Clone, Debug, Arbitrary)]
pub struct ValidNumber {
    pub leading_zero_count: usize,
    pub number: usize,
    pub suffix: Vec<u8>,
}

fuzz_target!(|data: ValidNumber| {
    let leading_zero_count = data.leading_zero_count % 25;
    let mut input: Vec<u8> = (0..leading_zero_count)
            .map(|_| 0x30)
            .collect();
    let Ok(()) = write!(&mut input, "{}", data.number) else {
        panic!("Failure to convert a number into a string inside the input vector");
    };

    let expected_len = input.len();
    input.extend(data.suffix.iter());
    input[expected_len..].iter_mut().for_each(|c| if *c >= 0x30 && *c <= 0x39 { *c += 10 });

    let ParseResult { value, len } = parse(&input[..]);

    if value != data.number {
        println!("leading zeroes: {}, number: {}, suffix: {:?}", leading_zero_count, data.number, data.suffix);
        println!("input: {:?}", input);
        panic!("value != number");
    }

    if len != expected_len {
        println!("leading zeroes: {}, number: {}, suffix: {:?}", leading_zero_count, data.number, data.suffix);
        println!("input: {:?}", input);
        println!("len: {}, expected_len: {}", len, expected_len);
        panic!("len != expected_len");
    }
});
