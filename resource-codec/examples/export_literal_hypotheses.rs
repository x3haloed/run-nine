// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
//! Small separate native experiment corpus; outcome=hypothesis is intentional.
//! No assertion that the production decoder or native Resource Manager accepts it.
#[path = "../tests/support/literal_hypotheses.rs"]
mod literal_hypotheses;
use std::io::{self, Write};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn main() -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    writeln!(out, "name\tattrs\tlimit\tinput_hex\toutcome\toutput_hex")?;
    for case in literal_hypotheses::fixtures() {
        writeln!(
            out,
            "{}\t1\t1024\t{}\thypothesis\t{}",
            case.name,
            hex(&case.input),
            hex(&case.expected)
        )?;
    }
    Ok(())
}
