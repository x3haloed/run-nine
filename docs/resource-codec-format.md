# RUN9 classic resource codec: reference-derived format and policy

This describes `resource-codec` version 0.2.0, independently authored from the
approved permissive references pinned in
[resource-codec-reference-pins.json](resource-codec-reference-pins.json).
It is a reference-derived specification, **not an assertion of verified native
Resource Manager behavior**. No GPL implementation, legacy tests, application
resource, or OS resource was read to author this implementation or its fixtures.
Native comparisons are a separate integration requirement.

## Sources and confidence

All fourteen approved files were SHA-256 verified against the ledger before use.
Primary grammar references are ResourceDASM at
`db45c79522bf9683f92b13ba675f582fb1fb31fd`:
[System01.cc](https://github.com/fuzziqersoftware/resource_dasm/blob/db45c79522bf9683f92b13ba675f582fb1fb31fd/src/ResourceDecompressors/System01.cc)
(`const_table0`, `read_encoded_int`, `execute_extension_command`, and the
`is_system1 == false` branch),
[System3.cc](https://github.com/fuzziqersoftware/resource_dasm/blob/db45c79522bf9683f92b13ba675f582fb1fb31fd/src/ResourceDecompressors/System3.cc)
(`decode_int_1_63`, `decode_int_0_2042`, all `read_int_max_*` functions,
`read_int_max`, `decompress_system3`), and
[System.hh](https://github.com/fuzziqersoftware/resource_dasm/blob/db45c79522bf9683f92b13ba675f582fb1fb31fd/src/ResourceDecompressors/System.hh)
(`CompressedResourceHeader`). These are MIT-licensed. The fixed dictionary is
transcribed format data from System01.cc; its full MIT notice is retained in
`resource-codec/NOTICES.txt`.

Independent references are Kaitai Struct formats at
`7d175667527372eb0c521045c1dac212230d9779`:
[compressed_resource.ksy](https://github.com/kaitai-io/kaitai_struct_formats/blob/7d175667527372eb0c521045c1dac212230d9779/macos/compressed_resource.ksy),
[dcmp_0.ksy](https://github.com/kaitai-io/kaitai_struct_formats/blob/7d175667527372eb0c521045c1dac212230d9779/macos/resource_compression/dcmp_0.ksy),
[dcmp_variable_length_integer.ksy](https://github.com/kaitai-io/kaitai_struct_formats/blob/7d175667527372eb0c521045c1dac212230d9779/macos/resource_compression/dcmp_variable_length_integer.ksy),
and MIT python-rsrcfork at `d2bbab1f5dd8670baa1ea5a31a1754b824743493`:
[common.py](https://github.com/dgelessus/python-rsrcfork/blob/d2bbab1f5dd8670baa1ea5a31a1754b824743493/src/rsrcfork/compress/common.py)
and [dcmp0.py](https://github.com/dgelessus/python-rsrcfork/blob/d2bbab1f5dd8670baa1ea5a31a1754b824743493/src/rsrcfork/compress/dcmp0.py).
Python's fixed table independently supplies the expected table-token results in
the synthetic test corpus. Its MIT notice is also retained.

## Entry point and header

`decompress_if_needed(attrs, data)` returns `Ok(None)` if resource-map attribute
bit 0 is clear, or if the complete initial signature `A8 9F 65 72` is absent.
Even truncated signature prefixes pass through. Other map attribute bits do not
change selection. This preserves caller-owned bytes without allocating a copy.
Once the signature and bit both match, malformed data is an error.

All multibyte header fields are big-endian. The header occupies eighteen bytes:

| Offset | Size | Meaning / current acceptance |
| --- | --- | --- |
| 0 | 4 | signature A8 9F 65 72 |
| 4 | 2 | header size: accept 0 or 18; always consume eighteen bytes |
| 6 | 1 | version: 8 or 9 |
| 7 | 1 | header attributes: require exactly 1 |
| 8 | 4 | unsigned declared decoded byte length |
| 12, version 8 | 1 | working-buffer fractional size; ignored |
| 13, version 8 | 1 | extra expansion space; ignored |
| 14, version 8 | 2 | signed decompressor ID; require 0 |
| 16, version 8 | 2 | reserved; require zero |
| 12, version 9 | 2 | signed decompressor ID; require 3 |
| 14, version 9 | 4 | decompressor parameters; ignored for dcmp3 |

The ignored buffer parameters concern native in-place working space, not this
separate-output decoder. System3.cc does not use the four version 9 bytes.
Other version/ID combinations explicitly return `Unsupported`; no custom
resource code is executed. Header size meaning is itself labeled uncertain by
Kaitai and python-rsrcfork. Kaitai accepts only 18; python accepts 0 and 18.
Kaitai/python require byte 7 equal to 1; ResourceDASM uses its low bit instead.
The stricter exact-value policy needs native evaluation (see hypotheses below).

## Shared dcmp0 integer

`V` denotes a signed variable integer. Read initial byte `h`:

* `00..7F`: value `h`.
* `80..FE`: consume byte `l`; value `(h * 256 + l) - C000`.
  This represents `-16384..16127`.
* `FF`: consume a big-endian signed 32-bit value.

Nonminimal representations are accepted. All finite encodings are checked for
truncation. Conversions to byte/word in extension values retain the low 8/16
bits. Extension counts use `C = V & FFFF`, following ResourceDASM's explicit
comment about native `dbf` low-word counters. Negative and high-bit count
inputs consequently wrap into `0..65535`; they do not become host-size lengths.
This differs from Python's stricter positive-count and value-range checks.

## dcmp0 byte-token grammar

Decode until a mandatory `FF` end marker. A memo is initially empty and stores
literal slices in insertion order; references never refer to arbitrary output.

| Token | Following bytes | Action |
| --- | --- | --- |
| 00 | V, then `2*V` bytes | output literal; V must be nonnegative |
| 01..0F | `2*token` bytes | output literal |
| 10 | V, then `2*V` bytes | output literal and memoize |
| 11..1F | `2*(token-10)` bytes | output literal and memoize |
| 20 | u8 slot | output memo slot `28 + slot` |
| 21 | u8 slot | output memo slot `128 + slot` |
| 22 | u16 slot | output memo slot `28 + slot` |
| 23..4A | none | output memo slot `token-23` |
| 4B..FD | none | output big-endian fixed dictionary word `table[token-4B]` |
| FE | extension byte plus parameters | see below |
| FF | none | terminate |

Hexadecimal token/slot constants above omit the `0x` prefix. The immutable fixed
dictionary has 179 words; its exact entries are in `resource-codec/src/table.rs`.
Zero-length literals are accepted and can be memoized. The largest addressable
memo index is `40+65535=65575`. Retain only the first 65,576 memo entries.
Further stored literal blocks are still consumed and output normally, but their
metadata is discarded because no supported token can address them. In particular,
zero-length stored blocks cannot grow metadata without bound. Storage uses
fallibly allocated borrowed input slices; no expanded literal copy is held for
each memo slot. Reserve requests grow in chunks of at most 256 entries, capped
at the remaining addressable slots. The largest requested memo capacity is
65,576 slice entries (1,049,216 bytes on a host with 16-byte slice references),
plus allocator overhead; an allocator may round an allocation upward.

Literal lengths for tokens 00/10 are disputed: Python/Kaitai describe a single
unsigned length byte; ResourceDASM calls `read_encoded_int`. This implementation
chooses V and includes 127/128/255/256/16127/16128-word tests. Values below 128
agree across both descriptions.

### dcmp0 extensions

All arithmetic on generated 16/32-bit values wraps to that width. Counter
calculations and host allocation/length calculations are checked.

| FE selector | Inputs | Output |
| --- | --- | --- |
| 00 | segment V, C, C address values V | initialize address=6; for each value, add `value-6` modulo 65536 and emit tail then address; finally emit tail |
| 01 | target V, delta V, C, first A5 V, optional more A5 values | emit C+1 records `6100,target,4EED,A5` (four words); after each first record subtract 8 from target; delta!=0 adds delta to A5; delta==0 consumes next A5 V |
| 02 | value V, C | emit low byte of value C+1 times |
| 03 | value V, C | emit low word of value C+1 times |
| 04 | initial V, C, C signed raw bytes | emit initial low word, then C words formed by adding each signed byte |
| 05 | initial V, C, C differences V | emit initial low word, then C words formed by adding each V |
| 06 | initial V, C, C differences V | emit initial longword, then C longwords formed by adding each V |

The tail in selector 00 is words `3F3C,segment,A9F0`; total output is `8*C+6`
bytes. Selector 01 emits `8*(C+1)` bytes. Selectors 04/05 emit `2*(C+1)` and
06 emits `4*(C+1)`. Other selectors fail explicitly. Python/Kaitai omit selectors
01 and 05; these are supported from ResourceDASM only. Selector 00 count zero
is accepted by this implementation, unlike Python's explicit positive check.

### dcmp0 termination and alignment policy

The final FF must be the final compressed byte. Even zero-output streams need
FF. Output must equal declared length, except an odd declared length may have
one final alignment byte, which is discarded. No second extra byte is accepted.
The optional byte is retained internally until FF, so subsequent commands cannot
use trimming to emit additional bytes. Zero-output commands may precede FF.
ResourceDASM trims arbitrary excess; Python describes only the odd-length case.
The narrower policy here is intentional and awaits native comparison.

## dcmp3 bit grammar

Read bits most-significant first within each byte. There is no end marker;
decoding stops when output length exactly equals the header length. Any command
that crosses that length fails. Residual bits in the last consumed byte must be
zero, and no whole trailing byte is accepted. Empty output requires an empty
payload. System3.cc ignores residual input, so this is a RUN9 strictness policy,
not a verified native rule.

First read a run code R using these prefix classes (`x` is an unsigned payload):

| Prefix | Extra bits | R |
| --- | --- | --- |
| 0 | 1 | x (0..1) |
| 100 | 0 | 2 |
| 101 | 1 | 3+x |
| 1100 | 1 | 5+x |
| 1101 | 2 | 7+x |
| 1110 | 3 | 11+x |
| 11110 | 3 | 19+x |
| 111110 | 5 | 27+x |
| 1111110 | 6 | 59+x |
| 11111110 | 7 | 123+x |
| 111111110 | 8 | 251+x |
| 1111111110 | 9 | 507+x |
| 1111111111 | 10 | 1019+x (up to 2042) |

State `allow_literal` starts true. If R==0 and state true, decode literal length
L, then L raw 8-bit bytes at the current bit position. State becomes L==63.
Otherwise copy from prior output: length is R+2 when state true, R+3 when false.
Decode an offset using the output size before this command; state then becomes
true. Thus a short literal must be followed by a copy if output is unfinished;
consecutive literal blocks require the earlier block to contain exactly 63 bytes.

Literal length prefixes:

| Prefix | Extra bits | L |
| --- | --- | --- |
| 0 | 0 | 1 |
| 100 | 0 | 2 |
| 101 | 0 | 3 |
| 110 | 2 | 4+x |
| 1110 | 3 | 8+x |
| 11110 | 4 | 16+x |
| 11111 | 5 | 32+x (up to 63) |

### dcmp3 offset families and compatibility anomalies

Choose first-arm payload width k using output size P:

| P upper bound, inclusive | k |
| --- | --- |
| 10 | 0 |
| 20 | 1 |
| 40 | 2 |
| 80 | 3 |
| 160 | 4 |
| 672 (2A0) | 5 |
| 1000 (3E8) | 6 |
| 2688 (A80) | 7 |
| 5376 (1500) | 8 |
| 10752 (2A00) | 9 |
| 21504 (5400) | 10 |
| 43008 (A800) | 11 |
| 70000 (11170) | 12 |
| 172032 (2A000) | 13 |
| larger | 14 |

Prefix `0` reads k bits and gives offset `1+x`. Prefix `10` reads k+2 bits and
gives offset `2^k+1+x`. Prefix `11` has base B=`5*2^k+1`; choose the smallest
w in `1..=k+4` such that P <= B-1+2^w, then read w bits and return B+x.
If no such w exists, prefix 11 is invalid. Early prefixes remain possible even
when P exceeds the third arm's maximum. P is context for choosing bit widths,
not an implicit clipping operation: decoded offset must separately be <=P.

Retain three pinned System3.cc anomalies:

* In k=7, the third-arm threshold 648 (288 hex) uses **4** bits instead of 3;
  next threshold 656 (290 hex) also uses 4. This branch is unreachable through
  the documented outer dispatch, which selects k=7 only for P>=1001. It is
  retained as a provenance fact, without claiming public-input coverage.
* In k=7, the w=10 threshold is **1644 (66C hex)** instead of 1664 (680 hex).
  P=1645..1664 uses 11 bits. Synthetic public vectors probe both edges.
* In k=14, w=7 has impossible threshold **8204 (200C hex)** instead of
  82048 (14080 hex), after the already larger w=6 threshold 81984 (14040 hex).
  Thus the effective selection skips w=7 and uses 8 for P=81985..82176.

Offsets are positive. Zero/beyond-start offsets fail. Copies proceed one byte
at a time from the current output minus offset, permitting overlapping repeated
patterns (e.g., distance 1 can expand a run much longer than the existing output).
No ordinary `memcpy` assumption is made.

## Bounds, errors and verification artifacts

Default maximum declared expansion is 64 MiB, explicitly a RUN9 policy.
`decompress_if_needed_with_limit` accepts a caller-selected limit. Declared
length is checked before output allocation. Allocation uses `try_reserve_exact`;
memo growth uses capped `try_reserve_exact` requests. Every output command checks its entire length
before expansion. Parsing range arithmetic is checked; malformed input returns
an owned `Debug`/`Display` error without panics. Input traversal bounds the number
of zero-output literal/memo commands. No unsafe code or external dependencies
are used. A caller selecting an enormous limit authorizes the corresponding
fallible allocation request; no claim is made that all host allocators avoid OS
termination under memory pressure.

`tests/support/mod.rs` contains independently authored synthetic byte/bit encoders
and exact expected results, with format dictionary facts cross-checked against
the independent Python reference. The corpus includes all token/extension classes,
all fixed dictionary words, all memo addressing classes and their endpoints,
excess memo stores with a ten-byte output limit and first/last-slot references,
integer-width boundaries, wrapped arithmetic/counts, all literal lengths, run
prefix classes, offset-family/third-arm threshold boundaries and anomalies,
truncations, invalid references/offsets, header rejection, trailing/alignment
policy, and default/custom expansion limits. Deterministic arbitrary-input tests
exercise 12,000 bounded malformed streams. These tests establish internal
agreement with the written grammar; they do not establish native compatibility.

Export reproducible public fixtures with:

```sh
cargo run --release --manifest-path resource-codec/Cargo.toml --example export_vectors > /tmp/run9-codec-vectors.tsv
```

TSV fields: `name`, decimal `attrs`, decimal `limit`, hex `input_hex`, `outcome`
(`decoded`, `pass`, or an error category), hex `output_hex`. No original resource
bytes are present. The generator is the source of truth; its large repetitive
export need not be committed.

## Hypotheses to resolve with native probes

1. Header size zero is accepted as eighteen bytes; exact header attribute byte 1
   and version 8 reserved zero are required. Test alternate attribute bits.
2. 00/10 extended literal lengths use V rather than one unsigned byte. Probe
   127/128/255/256 words and nonminimal V encodings.
3. Extensions 01/05 exist for dcmp0; extension values truncate, counts mask to
   low16, and selector 00 accepts zero count. Probe signed/high-bit parameters.
4. Only odd declared lengths admit a single final decoded alignment byte;
   arbitrary overexpansion is rejected, while zero-output final commands work.
5. FF is mandatory/final for dcmp0, and dcmp3 permits only zero intra-byte
   padding with no trailing bytes. These are strict parser policies.
6. The irregular dcmp3 dispatch/thresholds are pinned ResourceDASM-derived
   hypotheses, not proven native behavior. Probe whether native decoding agrees
   at P=1644/1645/1664/1665 and P=81984/81985/82048/82049.
7. Ignoring dcmp3 header parameters and the version 8 allocation parameters does
   not alter logical decoded bytes when decoding into a separate bounded buffer.

Application baselines contain dcmp0 only; success there cannot validate dcmp3.
Native/synthetic comparisons and runtime integration remain separate work.
