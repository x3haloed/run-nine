#!/usr/bin/env python3
"""Align a PCM capture and report raw sample errors; never fit away gain/errors."""
import argparse
import hashlib
import json
import wave
from pathlib import Path
import numpy as np


def read(path):
    with wave.open(str(path), 'rb') as source:
        if source.getsampwidth() != 2 or source.getcomptype() != 'NONE':
            raise ValueError('Expected uncompressed signed16 WAV')
        channels, rate = source.getnchannels(), source.getframerate()
        data = np.frombuffer(source.readframes(source.getnframes()), dtype='<i2')
        return data.reshape(-1, channels).astype(np.float64), rate


def compare(expected_path, capture_path, source, profile=None):
    expected, rate = read(expected_path)
    captured, captured_rate = read(capture_path)
    if rate != captured_rate or expected.shape[1] != captured.shape[1]:
        raise ValueError('Rates/channel layouts differ; conversion must be declared separately')
    result = {'schema': 1, 'source': source, 'sample_rate': rate, 'channels': expected.shape[1],
              'expected_frames': len(expected), 'captured_frames': len(captured),
              'expected_sha256': hashlib.sha256(Path(expected_path).read_bytes()).hexdigest(),
              'captured_sha256': hashlib.sha256(Path(capture_path).read_bytes()).hexdigest(),
              'nonzero_samples': int(np.count_nonzero(captured)),
              'captured_peak': float(np.max(np.abs(captured))) if captured.size else 0,
              'comparison_available': len(captured) >= len(expected),
              'sample_exact': False,
              'normalization': 'Only whole-frame alignment by cross-correlation. No gain/DC correction, resampling, channel permutation or sample deletion.'}
    if len(captured) < len(expected):
        result['reason'] = 'Capture shorter than the independent expected buffer'
        return result
    if not expected.size:
        raise ValueError('Expected buffer must contain at least one frame')
    size = 1 << (len(captured) + len(expected) - 2).bit_length()
    correlation = sum(np.fft.irfft(np.fft.rfft(captured[:, channel], size) *
                      np.conj(np.fft.rfft(expected[:, channel], size)), size)
                      for channel in range(expected.shape[1]))
    alignable = bool(np.any(captured) and np.any(expected))
    lag = int(np.argmax(correlation[:len(captured) - len(expected) + 1])) if alignable else 0
    actual = captured[lag:lag + len(expected)]
    error = actual - expected
    energy = float(np.sum(expected * expected))
    norm = float(np.sqrt(energy * np.sum(actual * actual)))
    result.update({'alignment_frames': lag if alignable else None,
                   'alignment_seconds': lag / rate if alignable else None,
                   'alignment_available': alignable,
                   'comparison_window_start': lag,
                   'correlation_coefficient': float(np.sum(actual * expected) / norm) if norm else 0,
                   'gain_estimate_not_applied': float(np.sum(actual * expected) / energy) if energy else 0,
                   'max_abs_sample_error': float(np.max(np.abs(error))),
                   'rms_sample_error': float(np.sqrt(np.mean(error * error))),
                   'different_samples': int(np.count_nonzero(error)),
                   'sample_exact': bool(np.all(error == 0))})
    if not alignable:
        result['alignment_reason'] = 'A buffer has zero energy; offset zero is used only for diagnostic errors, not a measured alignment.'
    result['profile'] = profile
    result['sections'] = []
    for name, start, end in [('leading_silence',0,12000),('square_750Hz',12000,24000),
                             ('ramp_187.5Hz',24000,36000),('trailing_silence',36000,48000)]:
        if profile != 'sound-pcm-owned-v1':
            break
        if len(expected) != 48000 or rate != 48000:
            raise ValueError('Owned-vector profile requires the 48,000-frame, 48kHz vector')
        block = actual[start:end]
        result['sections'].append({'name': name, 'frames': [start,end],
            'mean_per_channel': np.mean(block,axis=0).tolist(),
            'rms_per_channel': np.sqrt(np.mean(block*block,axis=0)).tolist(),
            'rms_error': float(np.sqrt(np.mean(error[start:end]**2)))})
        if 'Hz' in name and alignable:
            # An explicitly reported inner window avoids boundary transients
            # only for stationary pitch measurement. Full-buffer errors above
            # still include every transition and transient sample.
            steady = actual[start+500:end-500,0]
            crossings = np.flatnonzero((steady[:-1] <= 0) & (steady[1:] > 0))
            median_period = float(np.median(np.diff(crossings))) if len(crossings)>1 else None
            result['sections'][-1].update(stationary_window_frames=[start+500,end-500],
                positive_zero_crossings=len(crossings), median_period_frames=median_period,
                measured_frequency_hz=rate/median_period if median_period else None)
    if expected.shape[1] == 2:
        result['aligned_stereo_different_samples'] = int(np.count_nonzero(actual[:,0] != actual[:,1]))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('expected', type=Path)
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--source', required=True, choices=['native-qemu','host-runtime','comparison-self-test'])
    parser.add_argument('--profile', choices=['sound-pcm-owned-v1'])
    args = parser.parse_args()
    result = compare(args.expected,args.capture,args.source,args.profile)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
