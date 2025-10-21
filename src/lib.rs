#![feature(likely_unlikely)]
#![feature(cold_path)]

mod r#impl;
mod utils;

#[cfg(debug_assertions)]
mod debug;

pub use crate::r#impl::ParseResult;

#[cfg(target_feature = "sse4.1")]
pub fn parse(x: &[u8]) -> ParseResult {
    unsafe { crate::r#impl::parse(x) }
}
