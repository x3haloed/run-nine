#!/usr/bin/env python3
"""JSON-lines adapter for the separately pinned MIT python-rsrcfork decoder.

Supply its source directory explicitly. No reference code is linked into RUN9.
This reference supports dcmp0/1/2; dcmp3 needs the separate ResourceDASM oracle.
"""
import argparse
import json
from pathlib import Path
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference_checkout', type=Path)
    args = parser.parse_args()
    sys.path.insert(0, str(args.reference_checkout.resolve() / 'src'))
    from rsrcfork.compress import decompress
    for line in sys.stdin:
        request = json.loads(line)
        data = bytes.fromhex(request['hex'])
        if not request['attrs'] & 1 or not data.startswith(bytes.fromhex('a89f6572')):
            result = {'status': 'passthrough'}
        else:
            try:
                result = {'status': 'expanded', 'hex': decompress(data).hex()}
            except Exception as error:
                result = {'status': 'error', 'error': str(error)}
        print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
