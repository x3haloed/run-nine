// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
mod support;
use run9_resource_codec::{decompress_if_needed, decompress_if_needed_with_limit};
#[test]
fn owned_vectors() {
    let vectors = support::vectors();
    eprintln!("{} owned vectors", vectors.len());
    for v in vectors {
        let result = decompress_if_needed_with_limit(v.attrs, &v.input, v.limit);
        if let Some(want) = v.error {
            assert_eq!(
                support::category(&result.expect_err(&v.name)),
                want,
                "{}",
                v.name
            );
        } else {
            assert_eq!(
                result.unwrap_or_else(|e| panic!("{}: {e}", v.name)),
                v.output,
                "{}",
                v.name
            );
        }
    }
}
#[test]
fn representative_truncations() {
    let names = [
        "dcmp0-export",
        "dcmp0-jump-0",
        "dcmp0-jump-8",
        "dcmp0-repeat-2--2147483648",
        "dcmp0-delta32-16128",
        "dcmp0-delta16-byte",
        "dcmp0-delta16-variable",
        "dcmp0-long-128-16",
        "dcmp3-literal-31",
        "dcmp3-run-2042-false",
        "dcmp3-offset-p1645-d1645",
    ];
    for v in support::vectors()
        .into_iter()
        .filter(|v| names.contains(&v.name.as_str()))
    {
        for n in 4..v.input.len() {
            assert!(
                decompress_if_needed(v.attrs, &v.input[..n]).is_err(),
                "{} prefix {n}",
                v.name
            );
        }
    }
}
#[test]
fn arbitrary_inputs_never_panic() {
    let mut seed = 0x5841d297u32;
    for n in 0..12000 {
        let version = if n % 2 == 0 { 8 } else { 9 };
        let len = n % 97;
        let mut payload = Vec::new();
        for _ in 0..n % 61 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            payload.push(seed as u8);
        }
        let data = support::header(version, len, &payload);
        let _ = decompress_if_needed_with_limit(1, &data, 128);
    }
}
