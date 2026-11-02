#!/usr/bin/env python3
"""Match captured guest PCM bytes to original standard snd resources.

This verifies asset identity, not native amplitude, timing or audibility.
Usage: verify-marathon-audio-inputs.py CAPTURE_DIRECTORY ORIGINAL_SOUND_FORK
"""
import argparse
import hashlib
import json
import runpy
import struct
from collections import defaultdict
from pathlib import Path


def verify(directory, fork):
    parse = runpy.run_path(str(Path(__file__).with_name('build-alert-resources.py')),
                          run_name='resource_parser')['resources']
    original = fork.read_bytes()
    index = defaultdict(list)
    for (kind, rid), data in parse(original).items():
        if kind != b'snd ':
            continue
        fmt, modifiers = struct.unpack_from('>HH', data)
        assert fmt in (1, 2), 'unsupported resource format'
        command = 4 + 6 * modifiers if fmt == 1 else 4
        count, code, _, offset = struct.unpack_from('>HHHI', data, command)
        assert count == 1 and code == 0x8051, 'unsupported command list'
        pointer, length, _, _, _, encoding, _ = struct.unpack_from('>IIIIIBB', data, offset)
        assert pointer == 0 and encoding == 0, 'unsupported sample header'
        raw = data[offset + 22:offset + 22 + length]
        assert len(raw) == length
        index[hashlib.sha256(raw).hexdigest()].append(rid)
    matches = []
    for row in map(json.loads, (directory / 'sound.jsonl').read_text().splitlines()):
        if row['event'] != 'input':
            continue
        # Saved runs remain relocatable: only the capture file basename matters.
        raw = (directory / 'inputs' / Path(row['input_file']).name).read_bytes()
        sha = hashlib.sha256(raw).hexdigest()
        assert len(raw) == row['frames'], 'captured input length differs'
        assert index[sha], f"token {row['token']} does not match original assets"
        matches.append(dict(token=row['token'], frames=len(raw), sha256=sha,
                            matching_original_resource_ids=index[sha]))
    assert matches, 'no guest sound buffers were captured'
    return dict(scope='Guest PCM identity only; not output waveform acceptance',
                source_fork_sha256=hashlib.sha256(original).hexdigest(),
                matched_original_inputs=matches)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture_directory', type=Path)
    parser.add_argument('original_sound_fork', type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.capture_directory, args.original_sound_fork), indent=2))
