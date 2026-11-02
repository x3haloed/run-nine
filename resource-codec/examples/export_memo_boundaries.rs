// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
//! Separate small native memo-address experiment corpus; no original resources.
#[path = "../tests/support/memo_boundaries.rs"]
mod memo_boundaries;
use std::io::{self, Write};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn main() -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    writeln!(out, "name\tattrs\tlimit\tinput_hex\toutcome\toutput_hex")?;
    for case in memo_boundaries::fixtures() {
        writeln!(
            out,
            "{}\t1\t4096\t{}\tdecoded\t{}",
            case.name,
            hex(&case.input),
            hex(&case.expected)
        )?;
    }
    Ok(())
}
