use std::arch::x86_64::__m128i;
use std::arch::x86_64::_mm_lddqu_si128;
use std::arch::x86_64::_mm_set_epi8;
use std::arch::x86_64::_mm_set_epi16;
use std::arch::x86_64::_mm_shuffle_epi8;

pub static POWERS_OF_TEN: [usize; 20] = [
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

#[inline]
#[target_feature(enable = "sse2")]
pub fn _mm_set2_epi8(x: i8, y: i8) -> __m128i {
    _mm_set_epi8(x, y, x, y, x, y, x, y, x, y, x, y, x, y, x, y)
}

#[inline]
#[target_feature(enable = "sse2")]
pub fn _mm_set2_epi16(x: i16, y: i16) -> __m128i {
    _mm_set_epi16(x, y, x, y, x, y, x, y)
}

#[inline]
#[target_feature(enable = "ssse3")]
pub fn shift_left_8x16(x: __m128i, amount: usize) -> __m128i {
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

    let shuffle = unsafe {
        _mm_lddqu_si128(SHUFFLE_LUT.get_unchecked(16 - amount) as *const i8 as *const __m128i)
    };

    _mm_shuffle_epi8(x, shuffle)
}

#[inline]
#[target_feature(enable = "ssse3")]
pub fn shift_right_8x16(x: __m128i, amount: usize) -> __m128i {
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

    let shuffle = unsafe {
        _mm_lddqu_si128(SHUFFLE_LUT.get_unchecked(amount) as *const i8 as *const __m128i)
    };

    _mm_shuffle_epi8(x, shuffle)
}
