// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
#[path = "support/memo_boundaries.rs"]
mod memo_boundaries;
use run9_resource_codec::decompress_if_needed_with_limit;
#[test]
fn small_memo_address_forms_match_arithmetic() {
    let cases = memo_boundaries::fixtures();
    assert_eq!(cases.len(), 12);
    for case in cases {
        assert!(case.stores <= 553);
        assert_eq!(
            case.prefix_output_len,
            2 * (case.stores + case.references.len())
        );
        assert_eq!(&case.input[12..18], &[255, 255, 0, 0, 0, 0]);
        for (index, &slot) in case.references.iter().enumerate() {
            let offset = 2 * (case.stores + index);
            assert_eq!(
                &case.expected[offset..offset + 2],
                &(0x3000 + slot as u16).to_be_bytes()
            );
        }
        assert!(case.expected[case.prefix_output_len..]
            .iter()
            .all(|&b| b == 0));
        assert_eq!(case.expected.len(), 4096);
        assert_eq!(
            decompress_if_needed_with_limit(1, &case.input, 4096).unwrap(),
            Some(case.expected),
            "{}",
            case.name
        );
    }
}
