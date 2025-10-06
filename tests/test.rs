use simd_parse_int;

#[test]
fn test_parse() {
    assert_eq!(
        simd_parse_int::parse(&[0]),
        simd_parse_int::ParseResult { value: 0, len: 0 }
    );
    assert_eq!(
        simd_parse_int::parse(&b"123".map(|u| u as i8)),
        simd_parse_int::ParseResult { value: 123, len: 3 }
    );
    assert_eq!(
        simd_parse_int::parse(&b"12345234562624652344".map(|u| u as i8)),
        simd_parse_int::ParseResult {
            value: 12345234562624652344,
            len: 20
        }
    );
    assert_eq!(
        simd_parse_int::parse(&b"00002222221343435542".map(|u| u as i8)),
        simd_parse_int::ParseResult {
            value: 2222221343435542,
            len: 20
        }
    );
}
