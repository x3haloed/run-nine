// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
//! `cargo run --example export_vectors > /tmp/run9-codec-vectors.tsv`
//! name, attrs decimal, limit decimal, input hex, outcome, output hex.
#[path = "../tests/support/mod.rs"]
mod support;
use std::io::{self, Write};
fn hex(v: &[u8]) -> String {
    v.iter().map(|b| format!("{b:02x}")).collect()
}
fn main() -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    writeln!(out, "name\tattrs\tlimit\tinput_hex\toutcome\toutput_hex")?;
    for v in support::vectors() {
        let outcome = v.error.unwrap_or(if v.output.is_some() {
            "decoded"
        } else {
            "pass"
        });
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}",
            v.name,
            v.attrs,
            v.limit,
            hex(&v.input),
            outcome,
            hex(v.output.as_deref().unwrap_or(&[]))
        )?;
    }
    Ok(())
}
