// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
//! Owned small memo-address cases with explicit wire tokens and arithmetic output.
#![allow(dead_code)]
pub struct Case {
    pub name: String,
    pub stores: usize,
    pub references: Vec<usize>,
    pub prefix_output_len: usize,
    pub input: Vec<u8>,
    pub expected: Vec<u8>,
}
fn build(name: String, stores: usize, references: &[(usize, &[u8])]) -> Case {
    let mut body = Vec::new();
    let mut expected = Vec::new();
    for slot in 0..stores {
        let word = (0x3000 + slot as u16).to_be_bytes();
        body.push(0x11); // Fixed two-byte stored literal; no extended-length dispute.
        body.extend(word);
        expected.extend(word);
    }
    for &(slot, encoding) in references {
        body.extend(encoding);
        expected.extend((0x3000 + slot as u16).to_be_bytes());
    }
    let prefix_output_len = expected.len();
    // Native adaptation is explicit: retain literal/reference prefix, add an
    // owned byte-repeat suffix and corresponding zero-filled expected suffix.
    let remaining = 4096 - prefix_output_len;
    body.extend([0xfe, 2, 0]);
    body.extend((0xc000 + (remaining - 1) as u16).to_be_bytes());
    body.push(0xff);
    expected.resize(4096, 0);
    let mut input = vec![0xa8, 0x9f, 0x65, 0x72, 0, 18, 8, 1, 0, 0, 0x10, 0];
    input.extend([255, 255, 0, 0, 0, 0]);
    input.extend(body);
    Case {
        name,
        stores,
        references: references.iter().map(|&(slot, _)| slot).collect(),
        prefix_output_len,
        input,
        expected,
    }
}
pub fn fixtures() -> Vec<Case> {
    let targets: &[(usize, &[u8])] = &[
        (0, &[0x23]),
        (39, &[0x4a]),
        (40, &[0x20, 0]),
        (295, &[0x20, 255]),
        (296, &[0x21, 0]),
        (551, &[0x21, 255]),
        (40, &[0x22, 0, 0]),
        (295, &[0x22, 0, 255]),
        (296, &[0x22, 1, 0]),
        (551, &[0x22, 1, 255]),
        (552, &[0x22, 2, 0]),
    ];
    let mut cases: Vec<Case> = targets
        .iter()
        .map(|&(slot, encoding)| {
            build(
                format!("dcmp0-memo-small-slot{slot}-tag{:02x}", encoding[0]),
                slot + 1,
                &[(slot, encoding)],
            )
        })
        .collect();
    cases.push(build("dcmp0-memo-small-composite553".into(), 553, targets));
    cases
}
