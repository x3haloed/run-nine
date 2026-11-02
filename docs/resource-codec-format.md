# RUN9 classic resource codec: reference-derived format and policy

This describes `resource-codec` version 0.2.0, independently authored from the
approved permissive references pinned in
[resource-codec-reference-pins.json](resource-codec-reference-pins.json).
It is a reference-derived specification with separately labeled targeted native
observations, **not a claim of complete native Resource Manager equivalence**.
No GPL implementation, legacy tests, application resource, or OS resource was read to author this implementation or its fixtures.
Native comparisons are a separate integration requirement.

## Sources and confidence

Every file in the approved reference ledger was SHA-256 verified before use.
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
Zero-length literals are accepted and can be memoized. The largest syntactically addressable
memo index is `40+65535=65575`. Retain only the first 65,576 memo entries.
Further stored literal blocks are still consumed and output normally, but their
metadata is discarded because no supported token can address them. In particular,
zero-length stored blocks cannot grow metadata without bound. Storage uses
fallibly allocated borrowed input slices; no expanded literal copy is held for
each memo slot. Reserve requests grow in chunks of at most 256 entries, capped
at the remaining addressable slots. The largest requested memo capacity is
65,576 slice entries (1,049,216 bytes on a host with 16-byte slice references),
plus allocator overhead; an allocator may round an allocation upward.

### Native memo workspace caveat and small boundary probes

The grammar's syntactic index range and this decoder's metadata cap do not prove
native equivalence at 65,575. In approved System01.cc's working-buffer comment,
ResourceDASM describes the original memo's next-slot offset and string-start
fields as `uint16_t`, then replaces that native arrangement with a host vector.
For 65,576 stored literals, the per-slot 16-bit offsets alone require 131,152
bytes, before the next-slot field and literal data; that cannot fit in a single
unwrapped table addressed by 16-bit scratch offsets. Increasing allocation hints
alone does not establish native validity. The reported measurements do not prove
an exact failure mechanism or a precise native slot limit, and do not justify
emulating overflow or corruption.

The parent reported that the giant owned `dcmp0-memo-all-address-forms` case
(65,576 stored two-byte literals and 46 references) produced the first 131,152
literal bytes correctly, then diverged at the first reference in native OS9.
Adapted output was 787,204 bytes with owned zero-fill. A fraction/extra-hints
128/255 run differed in 116 bytes; a separate 255/255 run completed but differed
in 52,884 bytes, again starting at offset 131,152. All three parent host decoders
agreed on the arithmetic expected bytes. Small native stored-literal/reference
controls passed. These parent-reported observations do not establish that the
giant native result is the format's intended semantics, nor demonstrate native
support for all 65,576 syntactically representable memo entries. The giant case
remains a bounded host-parser stress case with an explicit native caveat.
At these extreme memo counts, RUN9 deliberately retains the bounded separate
output interpretation shared by all three host decoders; native slot-65,575
equivalence is not claimed. No production semantics were changed to reproduce
the divergent bytes. Parent-reported normal application comparisons (689
resources, including 26 compressed dcmp0 resources) remain byte-exact; they do
not establish native behavior for the giant synthetic case.

A separate small native experiment corpus is authored in
`tests/support/memo_boundaries.rs` and exported with:

```sh
cargo run --release --manifest-path resource-codec/Cargo.toml --example export_memo_boundaries > /tmp/run9-memo-boundaries.tsv
```

It contains eleven isolated references and one composite case, each storing at
most 553 two-byte literals using fixed token 11. Stored slot i contains word
`3000+i` (hex base), and expected reference words are calculated directly from
the requested index. Targets are slots 0/39 via tokens 23/4A, 40/295 via 20,
296/551 via 21, and 40/295/296/551/552 via 22. The 552 case encodes offset
`0200` after 22, crossing the two-byte token forms' maximum index 551.
The composite stores 553 words then exercises all eleven encodings.

Each exported fixture explicitly uses fraction/extra hints 255/255 and extends
its literal/reference expected prefix to 4096 bytes using an owned zero-repeat
suffix. The longest expected prefix is 1128 bytes. This adaptation is part of
these owned fixtures, not an assertion that the header values guarantee native
workspace sufficiency. The TSV uses the main six-column schema and `decoded`
outcome as the host expectation. The parent subsequently reported all twelve
cases byte-exact in native OS9 (final boundary run cases 17..28; evidence below).
This establishes those tested address forms under the recorded fixture/workspace
conditions, not giant-table equivalence. Main corpus exports and production
decoding are unchanged.

### Extended dcmp0 literal lengths

Literal lengths for tokens 00/10 are disputed: Python/Kaitai describe a single
unsigned length byte; ResourceDASM calls `read_encoded_int`. This implementation
chooses V and includes 127/128/255/256/16127/16128-word tests. Values below 128
agree across both descriptions. Targeted native observations now positively
support V at 128/255 words for both tags, as detailed below; larger lengths
remain reference-derived.

A separate, deliberately small experimental corpus isolates this disagreement:
`tests/support/literal_hypotheses.rs` authors twelve fixtures (tags 00/10 times
127/128/255 words times byte/V encoding), exported independently using:

```sh
cargo run --release --manifest-path resource-codec/Cargo.toml --example export_literal_hypotheses > /tmp/run9-literal-hypotheses.tsv
```

The TSV retains the six main corpus columns, but `outcome` is **hypothesis**,
not `decoded`: `output_hex` is the proposed result under the named interpretation,
not an assertion of native acceptance. These fixtures are excluded from the main
production acceptance corpus. Their explicit length bytes are:

| Words | Byte hypothesis | V hypothesis |
| --- | --- | --- |
| 127 | 7F | 7F |
| 128 | 80 | C0 80 |
| 255 | FF | C0 FF |

The 127-word pair is a byte-identical shared control; 128/255 discriminate the
interpretations. Literal contents and proposed outputs are identical within each
pair, use newly authored deterministic bytes, and contain 254/256/510 bytes.
Each fixture has a version 8 header, resource attrs 1, reserved zero, native
working-buffer hints 1/255, one literal command, and final FF. Hints are supplied
to reduce native workspace confounding; they are not a new format guarantee.
Any native transport padding/adaptation must remain separately recorded. Fixture
shape tests check exact lengths and wire bytes without calling the production
decoder or asserting either disputed interpretation is correct. Production
literal handling is unchanged by the following targeted native observations.

### Targeted native literal observations

The integrating parent reported an OS9 Resource Manager run with
`complete=true` and six of six byte-exact comparisons for the VARIABLE fixtures:
`dcmp0-hypothesis-variable-words{127,128,255}-tag{00,10}`. Native inputs adapted
workspace and output padding: version 8 fraction/extra hints were 255/255, the
original literal prefix was preserved, and an owned zero-repeat suffix extended
expected output with zeros to a declared length of 4096. These successful cases
positively support variable-integer literal lengths at 128 and 255 words for both
tags under that adaptation. They do not establish native behavior for lengths
above 255, nonminimal encodings, insufficient workspace, or arbitrary malformed
streams. The default 64 MiB output bound remains RUN9 policy.

The separate BYTE experiment passed the two byte-identical 127-word controls,
then produced no result for its first 128-word case across two saved observations.
The parent deliberately aborted the incomplete run. The stall alone is
inconclusive and is not recorded as a native rejection verdict. Parent-reported
host comparisons found pinned MIT ResourceDASM matching all six VARIABLE cases
and MIT Python matching all six BYTE cases; each matched the two common controls
under the alternate encoding.

Private evidence pointers supplied by the parent (not opened by this fresh
implementation context) are under
`.local/codec-replacement/native-literal-variable/`:

| File | Parent-supplied SHA-256 |
| --- | --- |
| `manifest.json` | `60e06236d8113dc9d7d629a4b170cc5ea31831d21496167ace6f5184ab28d889` |
| `native-final.jsonl` | `d4088dc6353c74b9ecbf9bbd4ea7b7a491fdf32b7007f922b16f5b3cc85782c3` |
| `comparison-final.json` | `bc291fddd6f65c57e82ba94743b7e72a9cd0908da42c141ef835a72f3f94e62e` |

These hashes identify private observation records; no corresponding artifact
contents are copied here. The production parser and main acceptance corpus were
not changed.

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
that crosses that length fails. Once output is complete, unused bits and any
trailing bytes are ignored. A zero declared output length returns empty output
without reading the payload. The expansion limit and per-command output bounds
still apply before termination.

This compatibility policy follows the pinned MIT System3.cc decoder and measured
oracle I/O reported by the integrating parent: in the archived three-way-v1 run,
both the frozen baseline decoder and pinned MIT ResourceDASM body decoder
returned byte `41` for the nonzero unused-padding case and empty output for the
zero-length resource with a trailing byte. The original candidate rejected both
with `TrailingData`; that extra strictness was removed. This implementation
context did not read the baseline source, and the measurement does not prove
native Resource Manager behavior. The parent subsequently reported all seventeen
owned residual-input variants byte-exact in native OS9 (final boundary run cases
0..16; evidence below). This validates the tested residual cases under recorded
adaptations, not arbitrary malformed input or all possible trailing payloads.

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

## Recorded native boundary and extension observations

These measurements were supplied by the integrating parent; this implementation
context did not read the private evidence files or any GPL implementation.

The final OS9 boundary run reported `complete=true`, 30/30 byte-exact cases:

* Cases 0..16: all seventeen owned dcmp3 residual-input variants (five nonzero
  padding values and twelve trailing-content cases).
* Cases 17..28: all twelve small memo fixtures described above, including both
  endpoints of tokens 20/21, token 22 crossing slot 551 to 552, and the composite.
* Case 29: header size zero, using an adapted even dcmp0 stream with owned
  zero-fill to declared output length 4096. This positively demonstrates that
  specific header-zero case; it does not establish other header permissiveness.

Private directory `.local/codec-replacement/native-final-boundaries/`:

| File | Parent-supplied SHA-256 |
| --- | --- |
| `manifest.json` | `8690a5efb2aa8b9c886da3b1e28326ae8744b2c35d501a5398407d8e5c1b3eef` |
| `native-v2.jsonl` | `f68d6b6fb3eb19f920fd9bbfcd1a1100549a11b8f907b2c8c57a79393f997ad1` |
| `comparison-v2.json` | `58e66cb949e47d8bf910b272fe2da7d207da0d268677235dc689a1146556134b` |

A separate OS9 run reported `complete=true`, 143/143 byte-exact selected owned
extension cases, including negative/nonminimal variable-value encodings and
low-word count masking cases. It excluded the long-literal/header-zero questions
addressed separately. Parent-reported adaptations retained each even dcmp0
output prefix, added owned zero-repeat padding to output length
`max(4096, 4 * original compressed input length)`, and used version 8
fraction/extra hints 255/255. These are positive observations for the selected
cases identified by the parent's manifest, not a proof for all extension values,
all count encodings, or arbitrary malformed-input policies.

Private directory `.local/codec-replacement/native-extension-differences/`:

| File | Parent-supplied SHA-256 |
| --- | --- |
| `manifest.json` | `cfe3beef90904c02193cac577825b240c31c1f9763a3b7d8a6e89990b99557d1` |
| `native-v1.jsonl` | `e3d2b81a3e700799cfc7df309eb0cb6641dd4c4e8ec77ea603334ddf35978517` |
| `comparison-v1.json` | `b7e4c69fd863e36be244338d4378c5be400c418ee8ab25a6b9043e3791d1d31c` |

No production parser, output bound, or main corpus expectation was changed for
these positive native observations. The giant memo normalization caveat remains.

## Remaining hypotheses and limits of native observations

1. One adapted header-size-zero case is native-confirmed as described above.
   Exact header attribute byte 1 and version 8 reserved zero remain parser
   requirements; alternate attribute/reserved-bit policies need separate probes.
2. Native six-case observations support V literal lengths at 127/128/255 words
   for tags 00/10 with the recorded workspace/zero-fill adaptation. Probe lengths
   above 255 (including 256) and nonminimal V encodings separately.
3. Selected signed/nonminimal extension-value and low16-count cases now have
   143 positive native observations. The full grammar, including selectors 01/05
   and selector 00 zero count, remains specified from the approved references;
   do not infer exhaustive native equivalence beyond the identified tested cases.
4. Only odd declared lengths admit a single final decoded alignment byte;
   arbitrary overexpansion is rejected, while zero-output final commands work.
5. FF is mandatory/final for dcmp0 (a strict parser policy). dcmp3 ignores unused
   bits and trailing bytes after declared output is complete, supported by MIT
   reference/baseline agreement and seventeen positive native variants. Untested
   malformed-input policies still require separate evidence.
6. The irregular dcmp3 dispatch/thresholds are pinned ResourceDASM-derived
   hypotheses, not proven native behavior. Probe whether native decoding agrees
   at P=1644/1645/1664/1665 and P=81984/81985/82048/82049.
7. Ignoring dcmp3 header parameters and the version 8 allocation parameters does
   not alter logical decoded bytes when decoding into a separate bounded buffer.
   Standalone decoder fixtures can nevertheless have allocation hints insufficient
   for native in-place execution. The parent observed an invalid native handle
   while probing memo cases with version 8 hints 0,0; this is not grammar evidence.
   Kaitai explicitly warns insufficient working space can make the native
   decompressor misbehave. Native probe adaptations (such as version 8 hints
   1,255 or version 9 extra space 65535) must record original/adapted input hashes
   while preserving the tested bytecode/expected prefix and explicitly recording
   any added zero-fill suffix, changed declared length, and expected padding.

Application baselines contain dcmp0 only; success there cannot validate dcmp3.
Native/synthetic comparisons and runtime integration are tracked by the
integrating parent; crate tests alone do not establish whole-goal completion.
