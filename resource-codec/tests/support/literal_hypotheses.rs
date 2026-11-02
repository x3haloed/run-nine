// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
//! Experimental fixture expectations, not assertions of production/native grammar.
#![allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Byte,
    Variable,
}
pub struct Hypothesis {
    pub name: String,
    pub words: usize,
    pub tag: u8,
    pub encoding: Encoding,
    pub input: Vec<u8>,
    pub expected: Vec<u8>,
}
pub fn fixtures() -> Vec<Hypothesis> {
    let mut cases = Vec::new();
    for words in [127, 128, 255] {
        for tag in [0x00, 0x10] {
            for encoding in [Encoding::Byte, Encoding::Variable] {
                // Explicit disputed wire representations, independent of the
                // production parser and main corpus's variable-integer encoder.
                let length: &[u8] = match (encoding, words) {
                    (_, 127) => &[0x7f],
                    (Encoding::Byte, 128) => &[0x80],
                    (Encoding::Byte, 255) => &[0xff],
                    (Encoding::Variable, 128) => &[0xc0, 0x80],
                    (Encoding::Variable, 255) => &[0xc0, 0xff],
                    _ => unreachable!(),
                };
                let expected: Vec<u8> = (0..words * 2)
                    .map(|i| ((i * 37 + 19) % 251) as u8)
                    .collect();
                let mut input = vec![0xa8, 0x9f, 0x65, 0x72, 0, 18, 8, 1];
                input.extend_from_slice(&(expected.len() as u32).to_be_bytes());
                // Native in-place allocation hints, not a bytecode grammar claim.
                input.extend_from_slice(&[1, 255, 0, 0, 0, 0]);
                input.push(tag);
                input.extend_from_slice(length);
                input.extend_from_slice(&expected);
                input.push(0xff);
                let label = match encoding {
                    Encoding::Byte => "byte",
                    Encoding::Variable => "variable",
                };
                cases.push(Hypothesis {
                    name: format!("dcmp0-hypothesis-{label}-words{words}-tag{tag:02x}"),
                    words,
                    tag,
                    encoding,
                    input,
                    expected,
                });
            }
        }
    }
    cases
}
