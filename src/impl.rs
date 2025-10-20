use crate::utils::_mm_set2_epi8;
use crate::utils::_mm_set2_epi16;
use crate::utils::POWERS_OF_TEN;
use crate::utils::shift_left_8x16;
use crate::utils::shift_right_8x16;
use std::arch::x86_64::__m128i;
use std::arch::x86_64::_mm_add_epi8;
use std::arch::x86_64::_mm_and_si128;
use std::arch::x86_64::_mm_cmpgt_epi8;
use std::arch::x86_64::_mm_cvtsi128_si64;
use std::arch::x86_64::_mm_load_si128;
use std::arch::x86_64::_mm_madd_epi16;
use std::arch::x86_64::_mm_maddubs_epi16;
use std::arch::x86_64::_mm_movemask_epi8;
use std::arch::x86_64::_mm_packus_epi32;
use std::arch::x86_64::_mm_set_epi8;
use std::arch::x86_64::_mm_set_epi16;
use std::arch::x86_64::_mm_set1_epi8;
use std::arch::x86_64::_mm_sub_epi8;



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

#[inline]
#[cold]
fn cold_path() {}

#[inline]
#[target_feature(enable = "sse4.1")]
pub fn parse(x: &[u8]) -> ParseResult {
    if x.len() <= 6 {
        let mut value: usize = 0;
        let mut len = 0;

        for (i, byte) in x.iter().enumerate() {
            let c = byte - 0x30;
            if c > 9 {
                break;
            }

            value = value * 10 + c as usize;
            len = i + 1;
        }

        return ParseResult {
            value: value,
            len: len,
        };
    }

    // realign str to 16 bytes by masking away the last 4 bits of its address
    let mut str = x.as_ptr();
    let offset = str.addr() & (0b1111 as usize);
    str = unsafe { str.sub(offset) };

    let mut chunk = unsafe { _mm_load_si128(str as *const __m128i) };

    // since we loaded some bytes before the start of the actual string here we shift them away
    chunk = shift_right_8x16(chunk, offset);

    let mut result = parse_last_chars(chunk, x.len().min(16) as i8);
    let mut i = result.len;
    let mut value = result.value;

    // string is not all digits
    if result.len != 16 - offset {
        return ParseResult {
            value: value,
            len: i,
        };
    }

    // this duplicates the "last round" code below to speed up the common case
    // of 16 < size <= 32 by trading off duplicated code for better branch prediction.
    if x.len() - i <= 16 {
        let chunk_len = (x.len() - i) as i8;
        chunk = unsafe { _mm_load_si128(str.add(16) as *const __m128i) };

        let result = parse_last_chars(chunk, chunk_len);
        i += result.len;

        // from now on we need to check for overflow because (10^33 - 1) > 2^64
        // (maximum representable number > maximum number that fits in 64 bits)
        let Some(new_value) = value.checked_mul(POWERS_OF_TEN[result.len]) else {
            cold_path();
            return ParseResult { value: 0, len: 0 };
        };

        let Some(new_value) = new_value.checked_add(result.value) else {
            cold_path();
            return ParseResult { value: 0, len: 0 };
        };

        return ParseResult { value: new_value, len: i };
    }

    // TODO: verify it makes a difference

    cold_path();

    // strings with length > 32. The only way to end up here with a valid number
    // is by having 12 or more zeros in front of the string

    let mut loops = 1 as usize;

    // same as above, with checked addition for overflow and without shifting away bytes
    // continues until there are <= 16 bytes to parse
    loop {
        chunk = unsafe { _mm_load_si128(str.add(16 * loops) as *const __m128i) };

        let result = parse_16_chars(chunk);
        i += result.len;
        loops += 1;

        let Some(new_value) = value.checked_mul(POWERS_OF_TEN[result.len]) else {
            cold_path();
            return ParseResult { value: 0, len: 0 };
        };

        let Some(new_value) = new_value.checked_add(result.value) else {
            cold_path();
            return ParseResult { value: 0, len: 0 };
        };

        value = new_value;

        if result.len != 16 {
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

    let chunk_len = (x.len() - i) as i8;
    chunk = unsafe { _mm_load_si128(str.add(16 * loops) as *const __m128i) };

    result = parse_last_chars(chunk, chunk_len);
    i += result.len;

    let Some(value) = value.checked_mul(POWERS_OF_TEN[result.len]) else {
        cold_path();
        return ParseResult { value: 0, len: 0 };
    };

    let Some(value) = value.checked_add(result.value) else {
        cold_path();
        return ParseResult { value: 0, len: 0 };
    };

    ParseResult {
        value: value,
        len: i,
    }
}

#[inline]
#[target_feature(enable = "sse4.1")]
fn parse_16_chars(input: __m128i) -> ParseResult {
    let mut chunk = input;
    // algorithm comes from: https://kholdstare.github.io/technical/2020/05/26/faster-integer-parsing.html

    // subtract '0' from the chunk so that valid number characters are
    // translated into their value in bytes (i.e. '0' -> 0)
    let ascii_zeros = _mm_set1_epi8(0x30);
    chunk = _mm_sub_epi8(chunk, ascii_zeros);

    // build a shift mask that contains 0 for chars outside the '0' - '9' range
    // NOTE: we wrap the values around before comparing because SSE has only signed comparison
    // so we need to translate the value 0 into -128
    // NOTE: we OR the mask with 0x1000 to avoid digit_count being zero if all the chars
    // are in range
    let nine_bytes = _mm_set1_epi8(9);
    let wrap = _mm_set1_epi8(-128);
    let is_ascii_bytemask =
        _mm_cmpgt_epi8(_mm_add_epi8(chunk, wrap), _mm_add_epi8(nine_bytes, wrap));
    let is_ascii_bitmask = _mm_movemask_epi8(is_ascii_bytemask) | 0x10000;

    // TODO: check if usize cast is relevant or rust is a saner language
    let digit_count = is_ascii_bitmask.trailing_zeros() as usize;
    let non_digit_count: usize = 16 - digit_count;

    // shift away everything starting from the first trailing non-digit
    chunk = shift_left_8x16(chunk, non_digit_count);

    // for each group of two digits we multiply by either 1 or 10 and sum the result
    // e.g. 9 |_mm_set1_epi8comes 92 | 15
    let tens = _mm_set2_epi8(1, 10);
    chunk = _mm_maddubs_epi16(chunk, tens);

    // again with each group of four digits
    // e.g. 92 | 15 becomes 0 | 9215 (still fits in a short)
    let hundreds = _mm_set2_epi16(1, 100);
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
    let r_high = _mm_cvtsi128_si64(chunk) >> 32;
    let r_low = _mm_cvtsi128_si64(chunk) & 0xffffffff;

    ParseResult {
        value: (r_high + (r_low * 100000000)) as usize,
        len: digit_count,
    }
}

#[inline]
#[target_feature(enable = "sse4.1")]
fn parse_last_chars(chunk: __m128i, len: i8) -> ParseResult {
    // we've loaded 16 bytes so we need to mask the ones after the end of the string
    let indices = _mm_set_epi8(15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0);
    let lengths = _mm_set1_epi8(len);
    let is_before_end_mask = _mm_cmpgt_epi8(lengths, indices);

    let chunk = _mm_and_si128(chunk, is_before_end_mask);

    parse_16_chars(chunk)
}
