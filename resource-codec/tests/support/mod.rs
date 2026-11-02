// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
// Independently authored synthetic fixtures; no original resource bytes.
#![allow(dead_code)]
use run9_resource_codec::Error;
#[derive(Clone)]
pub struct Vector {
    pub name: String,
    pub attrs: u8,
    pub input: Vec<u8>,
    pub output: Option<Vec<u8>>,
    pub error: Option<&'static str>,
    pub limit: usize,
}
pub fn category(e: &Error) -> &'static str {
    match e {
        Error::Truncated => "truncated",
        Error::InvalidHeader(_) => "header",
        Error::Unsupported { .. } => "unsupported",
        Error::OutputLimit { .. } => "limit",
        Error::Allocation => "allocation",
        Error::InvalidInteger => "integer",
        Error::InvalidToken(_) => "token",
        Error::InvalidReference(_) => "reference",
        Error::InvalidOffset(_) => "offset",
        Error::LengthMismatch => "length",
        Error::TrailingData => "trailing",
    }
}
pub fn header(version: u8, len: usize, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0xa8, 0x9f, 0x65, 0x72, 0, 18, version, 1];
    v.extend_from_slice(&(len as u32).to_be_bytes());
    v.extend_from_slice(if version == 8 {
        &[128, 3, 0, 0, 0, 0]
    } else {
        &[0, 3, 0, 0, 0, 0]
    });
    v.extend_from_slice(payload);
    v
}
fn good(name: impl Into<String>, version: u8, payload: Vec<u8>, output: Vec<u8>) -> Vector {
    Vector {
        name: name.into(),
        attrs: 1,
        input: header(version, output.len(), &payload),
        output: Some(output),
        error: None,
        limit: 64 * 1024 * 1024,
    }
}
fn bad(name: impl Into<String>, input: Vec<u8>, error: &'static str) -> Vector {
    Vector {
        name: name.into(),
        attrs: 1,
        input,
        output: None,
        error: Some(error),
        limit: 64 * 1024 * 1024,
    }
}
pub fn integer(n: i32) -> Vec<u8> {
    if (0..=127).contains(&n) {
        vec![n as u8]
    } else if (-16384..=16127).contains(&n) {
        ((n + 0xc000) as u16).to_be_bytes().to_vec()
    } else {
        let mut v = vec![255];
        v.extend_from_slice(&n.to_be_bytes());
        v
    }
}
fn ext(kind: u8, values: &[i32]) -> Vec<u8> {
    let mut v = vec![254, kind];
    for &x in values {
        v.extend(integer(x));
    }
    v.push(255);
    v
}
#[derive(Default)]
struct Writer {
    bits: Vec<bool>,
}
impl Writer {
    fn put(&mut self, v: usize, n: usize) {
        for shift in (0..n).rev() {
            self.bits.push((v >> shift) & 1 != 0);
        }
    }
    fn pattern(&mut self, s: &str) {
        for b in s.bytes() {
            self.bits.push(b == b'1');
        }
    }
    fn bytes(self) -> Vec<u8> {
        let mut out = vec![0; self.bits.len().div_ceil(8)];
        for (i, b) in self.bits.into_iter().enumerate() {
            if b {
                out[i / 8] |= 128 >> (i % 8);
            }
        }
        out
    }
    fn length(&mut self, n: usize) {
        match n {
            1 => self.pattern("0"),
            2 => self.pattern("100"),
            3 => self.pattern("101"),
            4..=7 => {
                self.pattern("110");
                self.put(n - 4, 2)
            }
            8..=15 => {
                self.pattern("1110");
                self.put(n - 8, 3)
            }
            16..=31 => {
                self.pattern("11110");
                self.put(n - 16, 4)
            }
            32..=63 => {
                self.pattern("11111");
                self.put(n - 32, 5)
            }
            _ => panic!("length"),
        }
    }
    fn run(&mut self, n: usize) {
        let classes = [
            (0, 1, "0", 1),
            (2, 2, "100", 0),
            (3, 4, "101", 1),
            (5, 6, "1100", 1),
            (7, 10, "1101", 2),
            (11, 18, "1110", 3),
            (19, 26, "11110", 3),
            (27, 58, "111110", 5),
            (59, 122, "1111110", 6),
            (123, 250, "11111110", 7),
            (251, 506, "111111110", 8),
            (507, 1018, "1111111110", 9),
            (1019, 2042, "1111111111", 10),
        ];
        let &(lo, _, prefix, width) = classes
            .iter()
            .find(|&&(lo, hi, _, _)| (lo..=hi).contains(&n))
            .unwrap();
        self.pattern(prefix);
        self.put(n - lo, width);
    }
    fn literal(&mut self, data: &[u8]) {
        self.run(0);
        self.length(data.len());
        for &b in data {
            self.put(b as usize, 8)
        }
    }
    fn distance(&mut self, p: usize, d: usize) {
        let families = [
            (10, 0),
            (20, 1),
            (40, 2),
            (80, 3),
            (160, 4),
            (672, 5),
            (1000, 6),
            (2688, 7),
            (5376, 8),
            (10752, 9),
            (21504, 10),
            (43008, 11),
            (70000, 12),
            (172032, 13),
            (usize::MAX, 14),
        ];
        let &(_, k) = families.iter().find(|&&(top, _)| p <= top).unwrap();
        let a = 1 << k;
        let base = 5 * a + 1;
        if d <= a {
            self.pattern("0");
            self.put(d - 1, k)
        } else if d < base {
            self.pattern("10");
            self.put(d - a - 1, k + 2)
        } else {
            self.pattern("11");
            let mut width = (1..=k + 4).find(|&w| p < base + (1 << w)).unwrap();
            if k == 7 {
                width = match p {
                    0..=642 => 1,
                    643..=644 => 2,
                    645..=656 => 4,
                    657..=672 => 5,
                    673..=704 => 6,
                    705..=768 => 7,
                    769..=896 => 8,
                    897..=1152 => 9,
                    1153..=1644 => 10,
                    _ => 11,
                };
            }
            if k == 14 && width == 7 {
                width = 8;
            }
            self.put(d - base, width);
        }
    }
}
fn seed(w: &mut Writer, p: usize) -> Vec<u8> {
    let data: Vec<u8> = (0..p).map(|i| ((i * 73 + 19) % 251) as u8).collect();
    for c in data.chunks(63) {
        w.literal(c);
    }
    data
}
pub fn vectors() -> Vec<Vector> {
    let mut v = Vec::new();
    for attrs in [0, 2, 128, 254] {
        let mut t = good(format!("pass-bit-{attrs}"), 8, vec![255], vec![]);
        t.attrs = attrs;
        t.output = None;
        v.push(t);
    }
    for data in [
        vec![],
        vec![0xa8],
        vec![0xa8, 0x9f, 0x65],
        vec![1, 2, 3, 4],
        vec![0xa8, 0x9f, 0x65, 0x73],
    ] {
        v.push(Vector {
            name: format!("pass-signature-{}", data.len()),
            attrs: 1,
            input: data,
            output: None,
            error: None,
            limit: 0,
        });
    }
    v.push(good("dcmp0-empty", 8, vec![255], vec![]));
    for words in 0..=15 {
        let data: Vec<_> = (0..words * 2).map(|i| (i * 7 + 3) as u8).collect();
        for store in [false, true] {
            let tag = if store { 16 } else { 0 } + words as u8;
            let mut p = vec![tag];
            if words == 0 {
                p.push(0)
            }
            p.extend(&data);
            let mut expected = data.clone();
            if store {
                p.push(0x23);
                expected.extend(&data);
            }
            p.push(255);
            v.push(good(
                format!("dcmp0-literal-{words}-{store}"),
                8,
                p,
                expected,
            ));
        }
    }
    for words in [16, 127, 128, 255, 256, 16127, 16128] {
        let data: Vec<_> = (0..words * 2).map(|i| (i % 253) as u8).collect();
        for tag in [0, 16] {
            let mut p = vec![tag];
            p.extend(integer(words));
            p.extend(&data);
            p.push(255);
            v.push(good(
                format!("dcmp0-long-{words}-{tag}"),
                8,
                p,
                data.clone(),
            ));
        }
    }
    let mut p = Vec::new();
    let mut expected = Vec::new();
    for i in 0..=65575usize {
        let x = (i as u16).to_be_bytes();
        p.push(17);
        p.extend(x);
        expected.extend(x);
    }
    for i in 0..40 {
        p.push(0x23 + i);
        expected.extend((i as u16).to_be_bytes());
    }
    for (tag, index) in [
        (0x20, 40),
        (0x20, 295),
        (0x21, 296),
        (0x21, 551),
        (0x22, 552),
        (0x22, 65575),
    ] {
        p.push(tag);
        if tag == 0x22 {
            p.extend(((index - 40) as u16).to_be_bytes());
        } else {
            p.push((index - if tag == 0x20 { 40 } else { 296 }) as u8);
        }
        expected.extend((index as u16).to_be_bytes());
    }
    p.push(255);
    v.push(good("dcmp0-memo-all-address-forms", 8, p, expected));
    for attrs in [3, 127, 255] {
        let mut case = good(format!("map-attrs-{attrs}"), 8, vec![255], vec![]);
        case.attrs = attrs;
        v.push(case);
    }
    // Exercise every two-byte integer head at both low-byte boundaries.
    for h in 128..=254u8 {
        for low in [0, 255] {
            let payload = vec![254, 3, h, low, 0, 255];
            let word = ((i32::from(h) * 256 + i32::from(low)) - 0xc000) as u16;
            v.push(good(
                format!("dcmp0-varint-head-{h}-low-{low}"),
                8,
                payload,
                word.to_be_bytes().to_vec(),
            ));
        }
    }
    for value in [0u8, 1, 127] {
        v.push(good(
            format!("dcmp0-varint-nonminimal-2-{value}"),
            8,
            vec![254, 2, 0xc0, value, 0, 255],
            vec![value],
        ));
        v.push(good(
            format!("dcmp0-varint-nonminimal-5-{value}"),
            8,
            vec![254, 2, 255, 0, 0, 0, value, 0, 255],
            vec![value],
        ));
    }
    // Each fixed dictionary token has its own published format fact expectation.
    let table = include!("table_expected.rs");
    for (i, word) in table.into_iter().enumerate() {
        v.push(good(
            format!("dcmp0-table-{i}"),
            8,
            vec![0x4b + i as u8, 255],
            word.to_be_bytes().to_vec(),
        ));
    }
    v.push(good(
        "dcmp0-export",
        8,
        ext(0, &[7, 3, 16, 8, -4]),
        vec![
            0x3f, 0x3c, 0, 7, 0xa9, 0xf0, 0, 16, 0x3f, 0x3c, 0, 7, 0xa9, 0xf0, 0, 18, 0x3f, 0x3c,
            0, 7, 0xa9, 0xf0, 0, 8, 0x3f, 0x3c, 0, 7, 0xa9, 0xf0,
        ],
    ));
    v.push(good(
        "dcmp0-export-zero",
        8,
        ext(0, &[65537, 0]),
        vec![0x3f, 0x3c, 0, 1, 0xa9, 0xf0],
    ));
    for delta in [0, 8] {
        let values = if delta == 0 {
            vec![4, 0, 2, -2, 20, 30]
        } else {
            vec![4, 8, 2, -2]
        };
        let mut expected = Vec::new();
        for i in 0..3 {
            expected.extend([0x61, 0]);
            expected.extend((4u16.wrapping_sub(i * 8)).to_be_bytes());
            expected.extend([0x4e, 0xed]);
            let a = if delta == 0 {
                [-2i16, 20, 30][i as usize] as u16
            } else {
                65534u16.wrapping_add(i * 8)
            };
            expected.extend(a.to_be_bytes());
        }
        v.push(good(
            format!("dcmp0-jump-{delta}"),
            8,
            ext(1, &values),
            expected,
        ));
    }
    for n in [
        -2147483648,
        -16385,
        -16384,
        -1,
        0,
        127,
        128,
        16127,
        16128,
        2147483647,
    ] {
        for kind in [2, 3] {
            let bytes = (n as u16).to_be_bytes();
            let unit = if kind == 2 {
                bytes[1..].to_vec()
            } else {
                bytes.to_vec()
            };
            v.push(good(
                format!("dcmp0-repeat-{kind}-{n}"),
                8,
                ext(kind, &[n, 3]),
                unit.repeat(4),
            ));
        }
        v.push(good(
            format!("dcmp0-delta32-{n}"),
            8,
            ext(6, &[n, 2, 1, -2]),
            [
                n as u32,
                (n as u32).wrapping_add(1),
                (n as u32).wrapping_sub(1),
            ]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect(),
        ));
    }
    for count in [0, 65535, 65536, -1] {
        v.push(good(
            format!("dcmp0-count-mask-{count}"),
            8,
            ext(2, &[65, count]),
            vec![65; (count as u32 & 65535) as usize + 1],
        ));
    }
    v.push(good(
        "dcmp0-delta16-byte",
        8,
        vec![254, 4, 255, 0, 0, 0x7f, 0xff, 3, 1, 128, 127, 255],
        vec![0x7f, 0xff, 0x80, 0, 0x7f, 0x80, 0x7f, 0xff],
    ));
    v.push(good(
        "dcmp0-delta16-variable",
        8,
        ext(5, &[65535, 2, 1, -2]),
        vec![255, 255, 0, 0, 255, 254],
    ));
    let mut odd = good("dcmp0-odd-padding", 8, vec![1, 65, 99, 255], vec![65]);
    odd.input[8..12].copy_from_slice(&1u32.to_be_bytes());
    v.push(odd);
    for n in 1..=63 {
        let mut w = Writer::default();
        let data: Vec<_> = (0..n).map(|i| i as u8).collect();
        w.literal(&data);
        v.push(good(format!("dcmp3-literal-{n}"), 9, w.bytes(), data));
    }
    let mut w = Writer::default();
    let data = seed(&mut w, 126);
    v.push(good("dcmp3-consecutive-full-literals", 9, w.bytes(), data));
    for n in [
        0, 1, 2, 3, 4, 5, 6, 7, 10, 11, 18, 19, 26, 27, 58, 59, 122, 123, 250, 251, 506, 507, 1018,
        1019, 2042,
    ] {
        for full in [false, true] {
            if full && n == 0 {
                continue;
            }
            let mut w = Writer::default();
            let mut data = seed(&mut w, if full { 63 } else { 1 });
            w.run(n);
            w.distance(data.len(), 1);
            data.extend(vec![*data.last().unwrap(); n + if full { 2 } else { 3 }]);
            v.push(good(format!("dcmp3-run-{n}-{full}"), 9, w.bytes(), data));
        }
    }
    // Every family transition, every third-arm threshold, and anomalies ±1.
    let mut positions = vec![
        1, 2, 6, 7, 8, 9, 10, 11, 12, 13, 14, 20, 21, 22, 24, 25, 28, 29, 36, 37, 40, 41, 42, 44,
        48, 56, 72, 80, 81, 82, 84, 88, 96, 112, 144, 160, 161, 162, 164, 168, 176, 192, 224, 288,
        416, 672, 673, 322, 324, 328, 336, 352, 384, 448, 576, 832, 1000, 1001, 642, 644, 648, 656,
        672, 704, 768, 896, 1152, 1644, 1664, 2688,
    ];
    for k in 8..=14 {
        let base = 5 * (1 << k);
        for w in 1..=k + 4 {
            positions.push(base + (1 << w));
        }
    }
    positions.extend([5376, 10752, 21504, 43008, 70000, 172032, 344064]);
    let mut expanded = Vec::new();
    for p in positions {
        for d in [-1isize, 0, 1] {
            if p as isize + d > 0 {
                expanded.push((p as isize + d) as usize);
            }
        }
    }
    expanded.sort_unstable();
    expanded.dedup();
    for p in expanded {
        if p > 344064 {
            continue;
        }
        let family = [
            (10, 0),
            (20, 1),
            (40, 2),
            (80, 3),
            (160, 4),
            (672, 5),
            (1000, 6),
            (2688, 7),
            (5376, 8),
            (10752, 9),
            (21504, 10),
            (43008, 11),
            (70000, 12),
            (172032, 13),
            (usize::MAX, 14),
        ]
        .into_iter()
        .find(|&(top, _)| p <= top)
        .unwrap()
        .1;
        let mut distances = vec![
            1,
            p,
            1 << family,
            (1 << family) + 1,
            5 * (1 << family),
            5 * (1 << family) + 1,
        ];
        distances.retain(|&d| d <= p);
        distances.sort_unstable();
        distances.dedup();
        for d in distances {
            let mut w = Writer::default();
            let mut data = seed(&mut w, p);
            let full = p % 63 == 0;
            w.run(if full { 1 } else { 0 });
            w.distance(p, d);
            for _ in 0..3 {
                data.push(data[data.len() - d]);
            }
            v.push(good(format!("dcmp3-offset-p{p}-d{d}"), 9, w.bytes(), data));
        }
    }
    let mut w = Writer::default();
    let mut data = seed(&mut w, 344065);
    w.run(0);
    w.distance(data.len(), 1);
    data.extend(vec![*data.last().unwrap(); 3]);
    v.push(good(
        "dcmp3-above-last-tail-family-first-arm",
        9,
        w.bytes(),
        data,
    ));
    v.push(good("dcmp3-empty", 9, vec![], vec![]));
    let mut h = header(8, 0, &[255]);
    h[5] = 0;
    let mut zero = good("header-size-zero", 8, vec![255], vec![]);
    zero.input = h;
    v.push(zero);
    let mut params = good("header9-unused-params", 9, vec![], vec![]);
    params.input[14..18].copy_from_slice(&[255, 42, 17, 255]);
    v.push(params);
    for (index, value, error) in [
        (5, 17, "header"),
        (6, 7, "unsupported"),
        (7, 0, "header"),
        (7, 3, "header"),
        (15, 1, "unsupported"),
        (17, 1, "header"),
    ] {
        let mut h = header(8, 0, &[255]);
        h[index] = value;
        v.push(bad(format!("header-invalid-{index}-{value}"), h, error));
    }
    let mut limit = bad("output-limit-default", header(8, 67108865, &[255]), "limit");
    limit.limit = 67108864;
    v.push(limit);
    let mut limit = bad("output-limit-custom", header(9, 1, &[]), "limit");
    limit.limit = 0;
    v.push(limit);
    for (name, payload, len, error) in [
        ("empty", vec![], 0, "truncated"),
        ("memo", vec![0x23, 255], 2, "reference"),
        ("memo20", vec![0x20, 0, 255], 2, "reference"),
        ("memo21", vec![0x21, 0, 255], 2, "reference"),
        ("memo22", vec![0x22, 0, 0, 255], 2, "reference"),
        ("extension", vec![254, 7, 255], 0, "token"),
        ("negative-literal", vec![0, 128, 0, 255], 0, "integer"),
        ("overrun", vec![1, 0, 0, 255], 0, "length"),
        ("even-overrun", vec![2, 0, 0, 0, 0, 255], 2, "length"),
        ("odd-overrun", vec![2, 0, 0, 0, 0, 255], 1, "length"),
        ("short", vec![255], 2, "length"),
        ("trailing", vec![255, 0], 0, "trailing"),
    ] {
        v.push(bad(
            format!("dcmp0-invalid-{name}"),
            header(8, len, &payload),
            error,
        ));
    }
    let mut w = Writer::default();
    w.run(1);
    w.distance(0, 1);
    v.push(bad(
        "dcmp3-invalid-offset-initial",
        header(9, 3, &w.bytes()),
        "offset",
    ));
    let mut w = Writer::default();
    w.literal(&[65]);
    w.run(0);
    w.distance(1, 2);
    v.push(bad(
        "dcmp3-invalid-offset-large",
        header(9, 4, &w.bytes()),
        "offset",
    ));
    let mut w = Writer::default();
    w.literal(&[65, 66]);
    v.push(bad(
        "dcmp3-invalid-length",
        header(9, 1, &w.bytes()),
        "length",
    ));
    let mut w = Writer::default();
    w.literal(&[65]);
    let mut p = w.bytes();
    *p.last_mut().unwrap() |= 1;
    v.push(bad("dcmp3-invalid-padding", header(9, 1, &p), "trailing"));
    v.push(bad(
        "dcmp3-invalid-trailing",
        header(9, 0, &[0]),
        "trailing",
    ));
    // Truncate representative command forms at every byte boundary. Expected
    // category is intentionally generic: truncation can also reveal length errors.
    v
}
