#!/usr/bin/env python3
"""Package owned codec TSV vectors for native OS9 byte comparisons.

Generated probe binaries stay private because they include the external SDK's
runtime. Fixture names/inputs/expected bytes come only from the owned generator.
"""
import argparse
import csv
import ctypes
import hashlib
import json
import os
from pathlib import Path
import runpy
import struct

ROOT = Path(__file__).resolve().parents[1]


def fork(entries):
    groups = {}
    data = bytearray()
    for kind, rid, attrs, payload in entries:
        offset = len(data)
        if offset > 0xffffff:
            raise ValueError('classic resource offset overflow')
        data += struct.pack('>I', len(payload)) + payload
        groups.setdefault(kind, []).append(struct.pack('>hHI', rid, 0xffff, (attrs << 24) | offset) + bytes(4))
    types, refs = bytearray(), bytearray()
    type_size = 2 + 8 * len(groups)
    for kind, records in groups.items():
        types += kind + struct.pack('>HH', len(records) - 1, type_size + len(refs))
        refs += b''.join(records)
    name_offset = 30 + len(types) + len(refs)
    if name_offset > 65535:
        raise ValueError('classic resource map overflow')
    header = struct.pack('>4I', 256, 256 + len(data), len(data), name_offset)
    mapping = header + bytes(8) + struct.pack('>HHH', 28, name_offset, len(groups) - 1) + types + refs
    return header + bytes(240) + data + mapping


def resource_file(path, payload):
    path.write_bytes(b'')
    lib = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
    lib.setxattr.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p,
                            ctypes.c_size_t, ctypes.c_uint32, ctypes.c_int]
    data = ctypes.create_string_buffer(payload)
    if lib.setxattr(os.fsencode(path), b'com.apple.ResourceFork', data, len(payload), 0, 0):
        raise OSError(ctypes.get_errno(), str(path))
    if Path(str(path) + '/..namedfork/rsrc').read_bytes() != payload:
        raise ValueError('resource fork did not persist')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('vectors', type=Path)
    parser.add_argument('probe_resource', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--names', nargs='*', help='explicit positive vector names; otherwise all positives')
    parser.add_argument('--selection', type=Path, help='JSON array of explicit positive vector names')
    parser.add_argument('--embedded', action='store_true', help='small smoke corpus inside application')
    parser.add_argument('--include-hypotheses', action='store_true',
                        help='explicitly package unresolved format hypotheses as experiments')
    parser.add_argument('--native-workspace', action='store_true',
                        help='conservative native in-place workspace hints; token bytes unchanged')
    parser.add_argument('--native-padding', action='store_true',
                        help='append owned zero output to even dcmp0 cases for native in-place workspace')
    args = parser.parse_args()
    csv.field_size_limit(16 * 1024 * 1024)
    if args.names and args.selection:
        parser.error('choose --names or --selection')
    wanted = set(json.loads(args.selection.read_text())) if args.selection else set(args.names) if args.names else None
    rows = []
    with args.vectors.open() as stream:
        for row in csv.DictReader(stream, delimiter='\t'):
            if (row['outcome'] == 'decoded' or
                    (args.include_hypotheses and row['outcome'] == 'hypothesis')) and (wanted is None or row['name'] in wanted):
                rows.append(row)
    if not rows or (wanted is not None and wanted != {row['name'] for row in rows}):
        raise ValueError('missing/empty positive vector selection')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    batches, batch, size = [], [], 0
    for row in rows:
        cost = (len(row['input_hex']) + len(row['output_hex'])) // 2
        if batch and (len(batch) >= 128 or size + cost > 4 * 1024 * 1024):
            batches.append(batch)
            batch, size = [], 0
        batch.append(row)
        size += cost
    if batch:
        batches.append(batch)
    if args.embedded and len(batches) != 1:
        raise ValueError('embedded smoke corpus exceeds one batch')
    read_entries = runpy.run_path(str(ROOT / 'tools/inventory-resource-codec.py'))['entries']
    base = args.probe_resource.read_bytes()
    app = list(read_entries(base))
    manifest = []
    support = out / 'support'
    support.mkdir()
    for b, batch in enumerate(batches):
        records = []
        for i, row in enumerate(batch):
            raw, expected = bytes.fromhex(row['input_hex']), bytes.fromhex(row['output_hex'])
            original_hash = hashlib.sha256(raw).hexdigest()
            original_output_hash = hashlib.sha256(expected).hexdigest()
            padding = 0
            target_size = max(4096, 4 * len(raw))
            if args.native_padding and raw[6] == 8 and len(expected) < target_size and len(expected) % 2 == 0:
                if raw[-1] != 255:
                    raise ValueError('dcmp0 test stream lacks final marker')
                padding = target_size - len(expected)
                commands = bytearray()
                left = padding
                while left:
                    run = min(left, 65536)
                    count = run - 1
                    encoded_count = (bytes([count]) if count < 128 else
                                     (count + 0xc000).to_bytes(2, 'big') if count <= 16127 else
                                     bytes([255]) + struct.pack('>i', count))
                    commands += bytes([254, 2, 0]) + encoded_count
                    left -= run
                raw = bytearray(raw[:-1] + commands + bytes([255]))
                expected += bytes(padding)
                raw[8:12] = len(expected).to_bytes(4, 'big')
                raw = bytes(raw)
            if args.native_workspace:
                raw = bytearray(raw)
                if raw[6] == 8:
                    raw[12:14] = bytes([255, 255])
                elif raw[6] == 9:
                    raw[14:16] = bytes([255, 255])
                raw = bytes(raw)
            records.extend([(b'CMPD', 128 + i, int(row['attrs']), raw), (b'EXPT', 128 + i, 0, expected)])
            manifest.append({'batch': b, 'case': i, 'name': row['name'], 'source_outcome': row['outcome'],
                             'source_input_sha256': original_hash,
                             'source_output_sha256': original_output_hash, 'native_padding_bytes': padding,
                             'input_sha256': hashlib.sha256(raw).hexdigest(),
                             'output_sha256': hashlib.sha256(expected).hexdigest(), 'output_bytes': len(expected)})
        if args.embedded:
            app.extend(records)
        else:
            resource_file(support / f'CodecCases{b:03}', fork(records))
    if not args.embedded:
        app.append((b'BATS', 128, 0, struct.pack('>H', len(batches))))
    (out / 'ResourceCodecProbe.rsrc').write_bytes(fork(app))
    (out / 'manifest.json').write_text(json.dumps({'probe_base_sha256': hashlib.sha256(base).hexdigest(),
                                                 'workspace_policy': 'v8 fraction255/extra255; v9 extra65535' if args.native_workspace else 'unchanged input hints',
                                                 'padding_policy': 'Even dcmp0 streams append owned zero-repeat commands until output is at least4096 bytes and4x original compressed input; original decoded prefix preserved' if args.native_padding else 'unchanged token body',
                                                 'batches': len(batches), 'cases': manifest}, indent=2) + '\n')
    print('Packaged', len(rows), 'owned vectors in', len(batches), 'batches')


if __name__ == '__main__':
    main()
