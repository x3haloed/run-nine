#!/usr/bin/env python3
"""Falsification checks of the comparison tool, not audio implementation tests."""
import importlib.util
import json
import tempfile
import wave
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('pcm_compare', ROOT / 'tools/compare-pcm-capture.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def write(path, data):
    with wave.open(str(path), 'wb') as stream:
        stream.setparams((2, 2, 48000, 0, 'NONE', 'not compressed'))
        stream.writeframes(data.astype('<i2').tobytes())

rows = []
with tempfile.TemporaryDirectory(prefix='classic-pcm-comparator-') as temp:
    temp = Path(temp)
    # Different channels, isolated impulses and an asymmetric ramp prevent
    # a channel permutation or periodic correlation ambiguity from passing.
    expected = np.zeros((1024, 2), dtype=np.int16)
    expected[99, 0] = 8192
    expected[231, 1] = -16384
    expected[400:700, 0] = np.arange(300) * 32 - 4000
    expected[400:700, 1] = np.arange(300) * -16 + 2000
    write(temp / 'expected.wav', expected)
    correct = np.pad(expected, ((37, 19), (0, 0)))
    damaged = correct.copy(); damaged[37+1000, 0] = 1
    cases = [('offset', correct), ('one_sample', damaged),
             ('half_gain', correct // 2), ('swapped_channels', correct[:, ::-1]),
             ('silence', np.zeros_like(correct)), ('truncated', expected[:-1])]
    for name, capture in cases:
        write(temp / 'capture.wav', capture)
        result = module.compare(temp / 'expected.wav', temp / 'capture.wav', 'comparison-self-test')
        assert result['sample_exact'] == (name == 'offset'), (name, result)
        if name in ('offset', 'one_sample', 'half_gain'):
            assert result['alignment_frames'] == 37, (name, result)
        if name == 'one_sample':
            assert result['different_samples'] == 1 and result['max_abs_sample_error'] == 1
        if name == 'silence':
            assert result['alignment_frames'] is None and not result['alignment_available']
        if name == 'truncated':
            assert not result['comparison_available']
        rows.append({'case': name, 'passed': True, 'result': result})

output = ROOT / 'outputs/SoundPCM-comparator-self-test.json'
output.write_text(json.dumps({'scope': 'Comparison harness only; not evidence of native or runtime audio',
                              'cases': rows}, indent=2) + '\n')
print(f'{len(rows)} comparison falsification checks passed; {output}')
