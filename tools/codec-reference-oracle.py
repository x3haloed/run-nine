#!/usr/bin/env python3
"""JSON-lines adapter for the separately pinned MIT python-rsrcfork decoder.

Supply its source directory explicitly. No reference code is linked into RUN9.
This reference supports dcmp0/1/2; dcmp3 needs the separate ResourceDASM oracle.
"""
import argparse
import json
from pathlib import Path
import sys
import struct
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference_checkout', type=Path)
    parser.add_argument('--resource-dasm', action='store_true',
                        help='positional path is the compiled framed ResourceDASM adapter')
    args = parser.parse_args()
    process = None
    if args.resource_dasm:
        process = subprocess.Popen([str(args.reference_checkout.resolve())],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    else:
        sys.path.insert(0, str(args.reference_checkout.resolve() / 'src'))
        from rsrcfork.compress import decompress
    for line in sys.stdin:
        request = json.loads(line)
        data = bytes.fromhex(request['hex'])
        if process:
            process.stdin.write(struct.pack('>BI', request['attrs'], len(data)) + data)
            process.stdin.flush()
            header = process.stdout.read(5)
            if len(header) != 5:
                raise RuntimeError('reference adapter stopped')
            status, length = struct.unpack('>BI', header)
            if length > 64 * 1024 * 1024:
                raise RuntimeError('reference response exceeds limit')
            payload = process.stdout.read(length)
            if len(payload) != length:
                raise RuntimeError('short reference response')
            result = ({'status': 'passthrough'} if status == 0 else
                      {'status': 'expanded', 'hex': payload.hex()} if status == 1 else
                      {'status': 'error', 'error': payload.decode()})
        elif not request['attrs'] & 1 or not data.startswith(bytes.fromhex('a89f6572')):
            result = {'status': 'passthrough'}
        else:
            try:
                result = {'status': 'expanded', 'hex': decompress(data).hex()}
            except Exception as error:
                result = {'status': 'error', 'error': str(error)}
        print(json.dumps(result), flush=True)
    if process:
        process.stdin.close()
        if process.wait() != 0:
            raise RuntimeError('reference adapter exit failure')


if __name__ == '__main__':
    main()
