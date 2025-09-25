#![feature(likely_unlikely)]

use std::arch::x86_64::_mm_load_si128;
use std::arch::x86_64::__m128i;
use std::arch::x86_64::_mm_add_epi8;
use std::arch::x86_64::_mm_and_si128;
use std::arch::x86_64::_mm_cmpgt_epi8;
use std::arch::x86_64::_mm_cvtsi128_si64;
use std::arch::x86_64::_mm_lddqu_si128;
use std::arch::x86_64::_mm_madd_epi16;
use std::arch::x86_64::_mm_maddubs_epi16;
use std::arch::x86_64::_mm_movemask_epi8;
use std::arch::x86_64::_mm_packus_epi32;
use std::arch::x86_64::_mm_set_epi8;
use std::arch::x86_64::_mm_set_epi16;
use std::arch::x86_64::_mm_set1_epi8;
use std::arch::x86_64::_mm_shuffle_epi8;
use std::arch::x86_64::_mm_sub_epi8;

use core::hint::likely;
use core::hint::unlikely;

#[derive(PartialEq, Eq, Debug)]
pub struct ParseResult {
    pub value: usize,
    pub len: usize,
}

impl From<ParseResult> for Option<usize> {
    fn from(result: ParseResult) -> Self {
        if result.len == 0 {
            None
        } else {
            Some(result.value)
        }
    }
}

impl From<ParseResult> for Result<usize, ()> {
    fn from(result: ParseResult) -> Self {
        if result.len == 0 {
            Err(())
        } else {
            Ok(result.value)
        }
    }
}

pub fn parse(x: &[i8]) -> ParseResult {
    unsafe {
        // the case len == 1 is the only one where we're considerably (2x) slower
        // than the naive loop, so check for it specifically
        // benchmarks shows this doesn't slow the common case
        if unlikely(x.len() == 1) {
            let c = (x[0] - '0' as i8) as u8;
            if c > 9 {
                return ParseResult { value: 0, len: 0 };
            }

            return ParseResult {
                value: c as usize,
                len: 1,
            };
        }

        // realign str to 16 bytes by masking away the last 4 bits of its address
        let mut str = x.as_ptr();
        let offset = str.addr() & (0b1111 as usize);
        str = str.sub(offset);

        let mut chunk = _mm_load_si128(x.as_ptr() as *const __m128i);

        // since we loaded some bytes before the start of the actual string here we shift them away
        chunk = shift_right_8x16(chunk, offset);

        let mut result = parse_16_chars(chunk);
        let mut value = result.value;
        let mut i = result.digit_count;

        // common case: we're parsing a string with len <= 16
        if (result.digit_count != 16 - offset) || (x.len() - i == 0) {
            return ParseResult {
                value: value,
                len: i,
            };
        }

        // this duplicates the "last round" code below to speed up the common case
        // of 16 < size <= 32 by trading off duplicated code for better branch prediction.
        if likely(x.len() - i <= 16) {
            let chunk_end_index = (x.len() - i) as i8;
            chunk = _mm_load_si128(str.add(16) as *const __m128i);

            let result = parse_last_chars(chunk, chunk_end_index);
            i += result.digit_count;

            // from now on we need to check for overflow because (10^33 - 1) > 2^64
            // (maximum representable number > maximum number that fits in 64 bits)
            if let Some(sum) = (value * POWERS_OF_TEN[result.digit_count]).checked_add(result.value)
            {
                // likely!! TODO
                return ParseResult {
                    value: sum,
                    len: i,
                };
            } else {
                return ParseResult { value: 0, len: 0 };
            }
        }

        // strings with length > 32. THe only way to end up here with a valid number
        // is by having 12 zeroes in front of the string

        let mut loops = 1 as usize;

        // same as above, with checked addition for overflow and without shifting away bytes
        // continues until there are <= 16 bytes to parse
        loop {
            chunk = _mm_load_si128(str.add(16 * loops) as *const __m128i);

            let result = parse_16_chars(chunk);
            i += result.digit_count;
            loops += 1;

            if let Some(sum) = (value * POWERS_OF_TEN[result.digit_count]).checked_add(result.value)
            {
                // likely!! TODO
                value = sum
            } else {
                return ParseResult { value: 0, len: 0 };
            }

            if result.digit_count != 16 {
                return ParseResult {
                    value: value,
                    len: i,
                };
            }

            if x.len() - i <= 16 {
                break;
            }
        }

        // last round: we parse the remaining < 16 bytes

        let chunk_end_index = (x.len() - i) as i8;
        chunk = _mm_load_si128(str.add(16 * loops) as *const __m128i);

        result = parse_last_chars(chunk, chunk_end_index);
        i += result.digit_count;

        if let Some(sum) = (value * POWERS_OF_TEN[result.digit_count]).checked_add(result.value) {
            // likely!! TODO
            return ParseResult {
                value: sum,
                len: i,
            };
        } else {
            return ParseResult { value: 0, len: 0 };
        }
    }
}

struct IntermediateResult {
    value: usize,
    digit_count: usize,
}

// #[target_feature(enable = "ssse3")] TODO
fn parse_16_chars(input: __m128i) -> IntermediateResult {
    unsafe {
        let mut chunk = input;
        // algorithm comes from: https://kholdstare.github.io/technical/2020/05/26/faster-integer-parsing.html

        // subtract '0' from the chunk so that valid number characters are
        // translated into their value in bytes (i.e. '0' -> 0)
        let zero_chars = _mm_set1_epi8('0' as i8);
        chunk = _mm_sub_epi8(chunk, zero_chars);

        // build a shift mask that contains 0 for chars outside the '0' - '9' range
        // NOTE: we wrap the values around before comparing because SSE has only signed comparison
        // so we need to translate the value 0 into -128
        // NOTE: we OR the mask with 0x1000 to avoid digit_count being zero if all the chars
        // are in range
        let nine_bytes = _mm_set1_epi8(9);
        let wrap = _mm_set1_epi8(-128);
        let gt = _mm_cmpgt_epi8(_mm_add_epi8(chunk, wrap), _mm_add_epi8(nine_bytes, wrap));
        let tens_mask = _mm_movemask_epi8(gt) | 0x10000;

        // TODO: check if usize cast is relevant or rust is a saner language
        let digit_count = tens_mask.leading_zeros() as usize;
        let non_digit_count: usize = 16 - digit_count;

        // shift away everything starting from the first trailing non-digit
        chunk = shift_left_8x16(chunk, non_digit_count);

        // for each group of two digits we multiply by either 1 or 10 and sum the result
        // e.g. 9 | 2 | 1 | 5 becomes 92 | 15
        let tens = _mm_set2_epi8(10, 1);
        chunk = _mm_maddubs_epi16(chunk, tens);

        // again with each group of four digits
        // e.g. 92 | 15 becomes 0 | 9215 (still fits in a short)
        let hundreds = _mm_set2_epi16(100, 1);
        chunk = _mm_madd_epi16(chunk, hundreds);

        // the last madd has left the results in 32-bit parts
        // we need to pack those back into 16-bit parts before doing another madd below
        // we know they fit because they have an upper-bound of 100 * 100 < 65535
        chunk = _mm_packus_epi32(chunk, chunk);

        // again with groups of eight digits
        // let ten_thousands = _mm_set_epi16(10000, 1, 10000, 1, 0, 0, 0, 0);
        let ten_thousands = _mm_set_epi16(0, 0, 0, 0, 1, 10000, 1, 10000);
        chunk = _mm_madd_epi16(chunk, ten_thousands);

        // again with a single group of 16 digits (all fit in the low 64 bits of "chunk")
        let r_high: i64 = _mm_cvtsi128_si64(chunk) >> 32;
        let r_low: i64 = _mm_cvtsi128_si64(chunk) & 0xffffffff;

        return IntermediateResult {
            value: (r_high + (r_low * 100000000)) as usize,
            digit_count: digit_count,
        };
    }
}

fn parse_last_chars(input: __m128i, chunk_end_index: i8) -> IntermediateResult {
    unsafe {
        // we've loaded 16 bytes so we need to mask the ones after the end of the string
        let indices = _mm_set_epi8(15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0);
        let chunk_index_mask = _mm_set1_epi8(chunk_end_index);
        let is_before_end_mask = _mm_cmpgt_epi8(chunk_index_mask, indices);

        let chunk = _mm_and_si128(input, is_before_end_mask);

        return parse_16_chars(chunk);
    }
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

#[target_feature(enable = "sse2")]
fn _mm_set2_epi8(x: i8, y: i8) -> __m128i {
    _mm_set_epi8(y, x, y, x, y, x, y, x, y, x, y, x, y, x, y, x)
}

#[target_feature(enable = "sse2")]
fn _mm_set2_epi16(x: i16, y: i16) -> __m128i {
    _mm_set_epi16(y, x, y, x, y, x, y, x)
}

#[target_feature(enable = "ssse3")]
fn shift_left_8x16(x: __m128i, amount: usize) -> __m128i {
    // credits to Steven Hoving (https://github.com/KholdStare/qnd-integer-parsing-experiments/issues/3)

    #[rustfmt::skip]
    static SHUFFLE_LUT: [i8; 32] = [
        -128, -128, -128, -128, 
        -128, -128, -128, -128, 
        -128, -128, -128, -128, 
        -128, -128, -128, -128, 
        0, 1, 2, 3, 
        4, 5, 6, 7, 
        8, 9, 10, 11, 
        12, 13, 14, 15,
    ];

    let shuffle =
        unsafe { _mm_lddqu_si128(SHUFFLE_LUT.as_ptr().add(16).sub(amount) as *const __m128i) };

    _mm_shuffle_epi8(x, shuffle)
}

#[target_feature(enable = "ssse3")]
fn shift_right_8x16(x: __m128i, amount: usize) -> __m128i {
    #[rustfmt::skip]
    static SHUFFLE_LUT: [i8; 32] = [
        0, 1, 2, 3, 
        4, 5, 6, 7, 
        8, 9, 10, 11, 
        12, 13, 14, 15,
        -128, -128, -128, -128, 
        -128, -128, -128, -128, 
        -128, -128, -128, -128, 
        -128, -128, -128, -128, 
    ];

    let shuffle = unsafe { _mm_lddqu_si128(SHUFFLE_LUT.as_ptr().add(amount) as *const __m128i) };

    _mm_shuffle_epi8(x, shuffle)
}
