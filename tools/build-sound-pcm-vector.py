#!/usr/bin/env python3
"""Independent, owned byte vector and ideal integer conversion for SoundPCMProbe."""
import hashlib
import json
import struct
import wave
from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / 'outputs'
data = bytes(128 if i < 12000 or i >= 36000 else
             (160 if ((i - 12000) // 32) & 1 else 96) if i < 24000 else
             (i - 24000) & 255 for i in range(48000))
(OUT / 'SoundPCM-owned-input-u8.raw').write_bytes(data)
with wave.open(str(OUT / 'SoundPCM-owned-ideal-s16.wav'), 'wb') as output:
    output.setnchannels(2)
    output.setsampwidth(2)
    output.setframerate(48000)
    output.writeframes(b''.join(struct.pack('<hh', (b - 128) * 256, (b - 128) * 256) for b in data))
metadata = {'schema':1, 'kind':'independent_expected_vector', 'frames':48000,
            'sample_rate':48000, 'input_channels':1, 'input_bits':8,
            'input_sha256':hashlib.sha256(data).hexdigest(),
            'sections':[{'frames':[0,12000], 'signal':'unsigned8 silence128'},
                        {'frames':[12000,24000], 'signal':'square96/160,32 frames per half cycle,750Hz'},
                        {'frames':[24000,36000], 'signal':'unsigned8 ramp modulo256,187.5Hz'},
                        {'frames':[36000,48000], 'signal':'unsigned8 silence128'}],
            'ideal_conversion':'Each byte b maps to signed16(b-128)*256 in both stereo channels. Independently generated expected data, not native or runtime output. No claim of OS9 resampling, mixing or device equivalence.'}
(OUT / 'SoundPCM-owned-vector.json').write_text(json.dumps(metadata,indent=2)+'\n')
print(json.dumps({'frames':len(data),'sample_rate':48000,'input_sha256':hashlib.sha256(data).hexdigest(),
                  'scope':'Owned independent ideal vector, not native or runtime output'}))
