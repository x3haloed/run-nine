This directory vendors m68k 0.14.7, MIT, upstream commit
856b8045a42125f1a340c1ed365d50dd62114781. PORT.json pins the crates.io archive
and every retained/modified source. No Apple implementation is included.

The default `std` feature keeps the hosted interpreter's behavior. The runtime
and the freestanding kernel now build this same pinned interpreter. The
`freestanding` feature with default features disabled adds `no_std` + `alloc`,
using core/alloc aliases for std primitives and a shared allocation prelude.
Instruction decoding/implementation is retained. This is an interpreter port,
not a replacement with a fixture-specific instruction subset.

No-std one-time initialization uses spin 0.9.9 Once. Its portable trace cache
uses a serialized spin Mutex instead of host TLS. The kernel is single-core,
interrupts masked; it calls CpuCore::step. Reentrant execution through a bus
callback while holding the global trace cache is outside the tested profile.
Host unit fixtures retain TLS even when testing the freestanding feature so
parallel tests remain isolated. The actual ARM64 kernel uses no host TLS.

Three float helpers unavailable in core (f64 log10, floor, round) delegate to
libm 0.2.16. This does not establish FPU equivalence: our runtime declares no
FPU and the owned ARM64 experiment uses integer instructions only. Host-only
environment diagnostics are conditional on std. Two existing non-JIT dead-code
warnings are narrowly annotated. Host JIT, tracing/profiling and serde are not
supported with no_std in this port.

Validation commands:
 cargo test --lib --manifest-path vendor/m68k/Cargo.toml
 cargo test --lib --manifest-path vendor/m68k/Cargo.toml --no-default-features --features freestanding
 cargo check --lib --manifest-path vendor/m68k/Cargo.toml --no-default-features --features freestanding --target aarch64-unknown-none
 python3 tools/verify-native-boot.py

The final command executes the identical owned 68K image in freestanding ARM64
and compares its result/guard bytes to a separately collected OS9 observation.
It is not Marathon, a complete classic Macintosh, or physical Pi500 execution.
