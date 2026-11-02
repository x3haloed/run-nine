#!/usr/bin/env python3
"""Make a separate readable WAV when QEMU leaves canonical length fields zero.

Retain the original. Change only RIFF/data lengths, and verify identical payload.
This is container repair, never sample processing or evidence of correct audio.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

def finalize(source, destination, manifest):
    if destination.exists() or manifest.exists():
        raise FileExistsError('Derived WAV and metadata must be new files')
    original = source.read_bytes()
    if (len(original) < 44 or original[:4] != b'RIFF' or original[8:16] != b'WAVEfmt '
            or original[36:40] != b'data' or struct.unpack_from('<I', original, 16)[0] != 16):
        raise ValueError('Expected canonical 44-byte PCM WAV header')
    encoding, channels, rate, byte_rate, block, bits = struct.unpack_from('<HHIIHH', original, 20)
    if (encoding != 1 or bits != 16 or channels < 1 or block != 2 * channels
            or byte_rate != rate * block or rate == 0 or (len(original)-44) % block):
        raise ValueError('Invalid or unsupported PCM format/payload length')
    derived = bytearray(original)
    changed = []
    for offset, value, name in [(4, len(original)-8, 'RIFF byte length offset4'),
                                 (40, len(original)-44, 'data byte length offset40')]:
        old = struct.unpack_from('<I', original, offset)[0]
        if old not in (0, value):
            raise ValueError('Nonzero inconsistent size; refusing ambiguous repair')
        if old != value:
            struct.pack_into('<I', derived, offset, value)
            changed.append(name)
    assert original[44:] == derived[44:]
    assert all(a == b or i in range(4,8) or i in range(40,44)
               for i,(a,b) in enumerate(zip(original,derived)))
    destination.write_bytes(derived)
    digest = lambda b: hashlib.sha256(b).hexdigest()
    result = {'source': str(source), 'source_sha256': digest(original),
              'derived': str(destination), 'derived_sha256': digest(derived),
              'only_changed_fields': changed, 'payload_sha256': digest(original[44:]),
              'payload_bytes': len(original)-44, 'audio_samples_unchanged': True}
    manifest.write_text(json.dumps(result, indent=2)+'\n')
    return result

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('destination', type=Path)
    parser.add_argument('manifest', type=Path)
    args = parser.parse_args()
    print(json.dumps(finalize(args.source,args.destination,args.manifest)))
