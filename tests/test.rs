use simd_parse_int;

#[test]
fn test_parse() {
    assert_eq!(simd_parse_int::parse(&[0]), simd_parse_int::ParseResult{value: 0, len: 0});
    assert_eq!(simd_parse_int::parse(&['3' as i8]), simd_parse_int::ParseResult{value: 3, len: 1});
}