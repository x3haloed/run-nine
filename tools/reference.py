#!/usr/bin/env python3
"""Local QMP diagnostics for the OS 9 reference machine; no network service."""
import argparse
import json
from pathlib import Path
import socket
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOCKET = ROOT / 'work/os9/run/qmp.sock'

class QMP:
    def __enter__(self):
        self.events = []
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.settimeout(15)
        self.socket.connect(str(SOCKET))
        self.stream = self.socket.makefile('rwb', buffering=0)
        greeting = json.loads(self.stream.readline())
        if 'QMP' not in greeting:
            raise RuntimeError('Missing QMP greeting')
        self.command('qmp_capabilities')
        return self

    def command(self, name, arguments=None):
        request = {'execute': name, 'id': name}
        if arguments is not None:
            request['arguments'] = arguments
        self.stream.write((json.dumps(request) + '\n').encode())
        while True:
            line = self.stream.readline()
            if not line:
                raise RuntimeError('QEMU closed the control connection')
            response = json.loads(line)
            if 'event' in response:
                self.events.append(response)
            if response.get('id') == name:
                if 'error' in response:
                    raise RuntimeError(response['error'])
                return response['return']

    def __exit__(self, *args):
        self.stream.close()
        self.socket.close()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['status', 'capture'])
    args = parser.parse_args()
    with QMP() as qmp:
        if args.action == 'status':
            print(json.dumps(qmp.command('query-status'), indent=2))
        else:
            ppm = ROOT / 'work/os9/run/current.ppm'
            png = ROOT / 'outputs/os9-current.png'
            qmp.command('screendump', {'filename': str(ppm)})
            subprocess.run(['sips', '-s', 'format', 'png', str(ppm), '--out', str(png)], check=True, capture_output=True)
            print(png)

if __name__ == '__main__':
    main()
