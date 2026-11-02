#!/usr/bin/env python3
"""Check native amplitude ABI and quantify this fixture's raw audio transport.

The ratio envelope is fixture-specific: one unsigned8 source quantum / 32
signal units plus a 1% transport margin. It is not full native PCM equivalence.
No gain fitting, DC correction or resampling is applied to any samples.
"""
import argparse
import hashlib
import json
from pathlib import Path
import wave
import numpy as np


def check(reference, capture):
    rows = list(map(json.loads, reference.read_text().splitlines()))
    amplitude = [r for r in rows if r.get('phase') == 'amplitude']
    volume = [r for r in rows if r.get('phase') == 'volume']
    play = [r for r in rows if r.get('phase') == 'play']
    assert len(amplitude) == 256 and len(volume) == 5 and len(play) == 5
    for value, row in enumerate(amplitude):
        assert row['value'] == value
        assert row['error'] == row['get_amp_error'] == row['get_volume_error'] == 0
        assert row['amp'] == (value << 16) | 0x5555 and row['volume'] == 0x01000100
    for row in volume:
        assert row['error'] == row['get_amp_error'] == row['get_volume_error'] == 0
        assert row['volume'] == row['value'] and row['amp'] == 0xff5555
    assert all(r['error'] == r['buffer_error'] == r['callback_error'] == 0 and r['completed'] == 1 for r in play)
    assert rows[-1] == dict(complete=True, dispose_error=0)
    with wave.open(str(capture)) as stream:
        assert stream.getparams()[:3] == (2, 2, 48000)
        data = np.frombuffer(stream.readframes(stream.getnframes()), dtype='<i2').reshape(-1, 2).astype(float)
    energy = np.sqrt(np.mean(data[:len(data)//480*480].reshape(-1, 480, 2)**2, axis=(1, 2)))
    hot = np.flatnonzero(energy > 20)
    groups = [g for g in np.split(hot, np.flatnonzero(np.diff(hot) > 4)+1) if len(g) >= 10]
    assert len(groups) == 3, 'expected three detectable nonzero level sections'
    sections = []
    for level, group in zip([255, 127, 64], groups):
        middle = int((group[0]+group[-1]+1)*240)
        window = data[middle-2400:middle+2400]
        rms = np.sqrt(np.mean(window**2, axis=0))
        crossings = np.flatnonzero((window[:-1, 0] <= 0) & (window[1:, 0] > 0))
        frequency = float(48000 / np.median(np.diff(crossings)))
        ratio = rms / (sections[0]['rms'] if sections else rms)
        sections.append(dict(amplitude=level, window=[middle-2400, middle+2400],
                             rms=rms.tolist(), frequency_hz=frequency,
                             raw_min=window.min(axis=0).tolist(), raw_max=window.max(axis=0).tolist(),
                             ratio_to_255=ratio.tolist(), ideal_ratio=level/255,
                             ratio_error=(ratio-level/255).tolist()))
    thresholds = dict(frequency_hz=750, maximum_frequency_error_hz=1,
                      maximum_absolute_amplitude_ratio_error=1/32+0.01)
    passed = all(abs(r['frequency_hz']-750) <= 1 and
                 max(map(abs, r['ratio_error'])) <= thresholds['maximum_absolute_amplitude_ratio_error'] for r in sections)
    return dict(scope='Native ABI and fixture-specific pitch/amplitude-ratio checks; not full waveform equivalence',
                scalar_passed=True, reference_sha256=hashlib.sha256(reference.read_bytes()).hexdigest(),
                capture_sha256=hashlib.sha256(capture.read_bytes()).hexdigest(), thresholds=thresholds,
                calibration='Envelope declared after initial characterization; derived from source quantum and transport margin, not a blind prospective validation.',
                processing='Original PCM unchanged. Central 4800-frame windows; ratios are reported metrics only, never applied to samples.',
                sections=sections, passed=passed,
                limits='Native low-level1 and0 are both undetectable; signed16 runtime preserves low amplitudes. Absolute PCM errors and older failed8kHz transport correlation are not waived by this check.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    assert not args.output.exists()
    result = check(args.reference, args.capture)
    args.output.write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(dict(passed=result['passed'], output=str(args.output))))
    if not result['passed']:
        raise SystemExit(1)
