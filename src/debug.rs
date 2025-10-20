#![allow(dead_code)]

use std::arch::x86_64::__m128i;

#[cfg(debug_assertions)]
pub fn print_chunk(chunk: __m128i) {
    println!("chunk {:?}", unsafe {
        std::slice::from_raw_parts((&chunk) as *const __m128i as *const u8, 16)
    });
}
