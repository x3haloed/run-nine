#!/usr/bin/env python3
"""Inventory private reference forks through a separate JSON-lines decoder.

Only the script is public. Application bytes, expanded bytes, and reports belong
in ignored local storage. This reader follows the classic resource-fork layout.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def entries(blob):
    def span(offset, size):
        if offset < 0 or size < 0 or offset + size > len(blob):
            raise ValueError('resource fork bounds')
        return blob[offset:offset + size]

    def word(offset):
        return int.from_bytes(span(offset, 2), 'big')

    data, mapping, data_size, map_size = struct.unpack('>IIII', span(0, 16))
    span(data, data_size)
    span(mapping, map_size)
    types = mapping + word(mapping + 24)
    type_count = word(types)
    for i in range(0 if type_count == 65535 else type_count + 1):
        at = types + 2 + i * 8
        kind = span(at, 4)
        refs = types + word(at + 6)
        for j in range(word(at + 4) + 1):
            ref = refs + j * 12
            rid = int.from_bytes(span(ref, 2), 'big', signed=True)
            attrs = span(ref + 4, 1)[0]
            offset = data + int.from_bytes(span(ref + 5, 3), 'big')
            size = int.from_bytes(span(offset, 4), 'big')
            if offset + 4 + size > data + data_size:
                raise ValueError('resource payload exceeds data section')
            yield kind, rid, attrs, span(offset + 4, size)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('decoder', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    forks = [(name, ROOT / f'work/apps/{name}/{name}.rsrc')
             for name in ('TeachText', 'SimpleText', 'Marathon')]
    provenance = json.loads((ROOT / 'outputs/Marathon-provenance.json').read_text())
    for asset in provenance['assets']:
        name = asset['name']
        if name == 'Marathon v1.2':
            continue
        path = ROOT / 'work/volumes/marathon-source-v1/files' / name.lower().encode().hex() / 'resource'
        forks.append(('Marathon-sibling-' + name.encode().hex(), path))
    decoder = args.decoder.resolve()
    process = subprocess.Popen([str(decoder)], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, text=True)
    reports = []
    try:
        for label, path in forks:
            blob = path.read_bytes()
            rows = []
            for kind, rid, attrs, payload in entries(blob):
                process.stdin.write(json.dumps({'attrs': attrs, 'hex': payload.hex()}) + '\n')
                process.stdin.flush()
                line = process.stdout.readline()
                if not line:
                    raise RuntimeError('comparison decoder stopped')
                response = json.loads(line)
                row = {'kind_hex': kind.hex(), 'id': rid, 'attrs': attrs,
                       'input_bytes': len(payload), 'input_sha256': sha(payload),
                       'status': response['status']}
                if payload.startswith(bytes.fromhex('a89f6572')) and len(payload) >= 18:
                    header_type = payload[6]
                    row['header_type'] = header_type
                    row['decoder_id'] = int.from_bytes(payload[14:16] if header_type == 8
                                                       else payload[12:14], 'big', signed=True)
                if response['status'] == 'expanded':
                    expanded = bytes.fromhex(response['hex'])
                    target = out / 'expanded' / label / f'{kind.hex()}-{rid}.bin'
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(expanded)
                    row.update(output_bytes=len(expanded), output_sha256=sha(expanded),
                               output_file=str(target.relative_to(out)))
                elif response['status'] == 'error':
                    row['error'] = response['error']
                rows.append(row)
            reports.append({'label': label, 'path': str(path.relative_to(ROOT)),
                            'fork_sha256': sha(blob), 'resources': rows,
                            'status_counts': dict(collections.Counter(r['status'] for r in rows))})
        process.stdin.close()
        if process.wait() != 0:
            raise RuntimeError('decoder exit failure')
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait()
    report = {'decoder_sha256': sha(decoder.read_bytes()), 'forks': reports,
              'resource_count': sum(len(f['resources']) for f in reports)}
    (out / 'inventory.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'resources': report['resource_count'], 'forks': len(reports),
                      'counts': {f['label']: f['status_counts'] for f in reports}}, indent=2))


if __name__ == '__main__':
    main()
