// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
#[path = "support/literal_hypotheses.rs"]
mod hypotheses;
use hypotheses::Encoding;

#[test]
fn experiment_wire_bytes_and_expected_lengths() {
    let cases = hypotheses::fixtures();
    assert_eq!(cases.len(), 12);
    for case in &cases {
        assert_eq!(case.expected.len(), case.words * 2, "{}", case.name);
        assert_eq!(&case.input[..8], &[0xa8, 0x9f, 0x65, 0x72, 0, 18, 8, 1]);
        assert_eq!(
            &case.input[8..12],
            &(case.expected.len() as u32).to_be_bytes()
        );
        assert_eq!(&case.input[12..18], &[1, 255, 0, 0, 0, 0]);
        assert_eq!(case.input[18], case.tag);
        let encoded = match case.encoding {
            Encoding::Byte => vec![case.words as u8],
            Encoding::Variable if case.words == 127 => vec![0x7f],
            Encoding::Variable => (0xc000u16 + case.words as u16).to_be_bytes().to_vec(),
        };
        assert_eq!(&case.input[19..19 + encoded.len()], &encoded);
        assert_eq!(
            &case.input[19 + encoded.len()..case.input.len() - 1],
            &case.expected
        );
        assert_eq!(case.input.last(), Some(&0xff));
        assert_eq!(
            case.input.len(),
            18 + 1 + encoded.len() + case.words * 2 + 1
        );
    }
    // 127 is the common control. 128 and 255 discriminate the two hypotheses.
    for pair in cases.chunks_exact(2) {
        assert_eq!(pair[0].expected, pair[1].expected);
        assert_eq!(pair[0].input == pair[1].input, pair[0].words == 127);
    }
}
