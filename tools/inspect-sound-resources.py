#!/usr/bin/env python3
"""Inventory standard sampled-sound resources without executing an application."""
import argparse
import hashlib
import importlib.util
import json
import struct
from pathlib import Path

def inspect(path):
    spec = importlib.util.spec_from_file_location('resource_parser', Path(__file__).with_name('build-alert-resources.py'))
    parser = importlib.util.module_from_spec(spec);spec.loader.exec_module(parser)
    blob = path.read_bytes();rows = []
    for (kind, rid), data in parser.resources(blob).items():
        if kind != b'snd ':continue
        fmt = struct.unpack_from('>H', data)[0]
        if fmt == 1:
            modifiers = struct.unpack_from('>H', data, 2)[0]
            commands = 4 + 6 * modifiers
        elif fmt == 2:
            modifiers = 0;commands = 4
        else:raise ValueError(f'Unsupported resource format {fmt} in {rid}')
        count = struct.unpack_from('>H', data, commands)[0]
        if count != 1:raise ValueError(f'Inventory expects one buffer command in {rid}')
        command, param1, offset = struct.unpack_from('>HHI', data, commands+2)
        if command != 0x8051:raise ValueError(f'Expected relative bufferCmd in {rid}')
        sampleptr, length, rate, loopstart, loopend, encoding, basefreq = struct.unpack_from('>IIIIIBB', data, offset)
        if encoding != 0 or sampleptr != 0 or rate == 0 or len(data)-offset-22 != length:
            raise ValueError(f'Unsupported or inconsistent standard sample header in {rid}')
        rows.append(dict(id=rid, resource_bytes=len(data), sha256=hashlib.sha256(data).hexdigest(),
            format=fmt, modifier_count=modifiers, command_count=count, command=command,
            header_offset=offset, sample_pointer=sampleptr, length=length, rate_fixed=rate,
            rate_hz=rate/65536, loop=[loopstart,loopend], encoding=encoding,
            base_frequency=basefreq, payload_bytes=len(data)-offset-22))
    return dict(source_sha256=hashlib.sha256(blob).hexdigest(),
                scope='Static resource inspection only; not executed sound behavior', entries=rows)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resource_fork',type=Path);parser.add_argument('output',type=Path)
    args = parser.parse_args();result = inspect(args.resource_fork)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(f"{len(result['entries'])} standard sampled sound resources inspected")
