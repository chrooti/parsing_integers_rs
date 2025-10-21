## simd_parse_int

simd_parse_int offers a single `parse` function that parses a `u8` slice into an usize. Only tested on x86_64, requires SSE and nightly rust.

Performance profile is the same as `atoi_simd` and a naive loop for integers with ~10 digits and pulls away to around 2x perf at ~20 digits.
Measured on a 3900x by enabling SSE only, YMMV.