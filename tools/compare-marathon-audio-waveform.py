#!/usr/bin/env python3
"""Independently reconstruct the observed one-shot game path from guest PCM.

Uses captured source bytes, rates, amplitude and frame events, not renderer code.
The event schedule is an input: this does not prove native scheduling parity.
Rejects commands outside this measured path instead of assuming no effect.
Usage: compare-marathon-audio-waveform.py CAPTURE_DIRECTORY NEW_REPORT.json
"""
import argparse
import hashlib
import json
import wave
from pathlib import Path
import numpy as np


def compare(directory):
    rows = list(map(json.loads, (directory / 'sound.jsonl').read_text().splitlines()))
    results = [r for r in rows if r['event'] == 'api_result']
    assert results and all(r['error'] == 0 for r in results), 'failed/unobserved guest API results'
    with wave.open(str(directory / 'capture.wav')) as stream:
        assert stream.getparams()[:3] == (2, 2, 48000)
        captured = np.frombuffer(stream.readframes(stream.getnframes()), dtype='<i2').reshape(-1, 2)
    sources, channels, quiesced = {}, {}, {}
    starts, ends = {}, {}
    last_command = None
    for row in rows:
        event = row['event']
        if event == 'new_channel':
            assert row['synth'] == 5 and row['init'] == 128
            channels[row['channel']] = 255
            quiesced[row['channel']] = True
        elif event == 'command':
            assert row['code'] in (0, 3, 4, 13, 43, 81), 'unmodeled sound command'
            if row['code'] == 43:
                assert row['queued'] and 0 <= row['param1'] <= 255
                assert quiesced[row['channel']], 'amplitude queued behind active audio is outside this model'
                channels[row['channel']] = row['param1']
            elif row['code'] == 3:
                quiesced[row['channel']] = True
            elif row['code'] == 81:
                assert quiesced[row['channel']], 'multiple queued buffers are outside this game-path model'
                quiesced[row['channel']] = False
            last_command = row
        elif event == 'input':
            assert last_command and last_command['code'] == 81
            assert last_command['param2'] == row['header']
            source = np.frombuffer((directory / 'inputs' / Path(row['input_file']).name).read_bytes(),
                                   dtype=np.uint8).astype(np.int64)
            assert len(source) == row['frames']
            sources[row['token']] = (row, (source - 128) * 256,
                                     channels[last_command['channel']])
        elif event == 'started':
            assert row['token'] not in starts
            starts[row['token']] = row['frame']
        elif event in ('finished', 'cancelled'):
            assert row['token'] not in ends
            ends[row['token']] = row['frame']
    assert sources and sources.keys() == starts.keys(), 'unstarted inputs'
    assert ends.keys() <= starts.keys(), 'unowned completion event'
    metadata = json.loads((directory / 'capture.json').read_text())
    assert not metadata['device_errors'] and metadata.get('drain_error') is None
    assert not metadata['capture_overflow'] and metadata['rendered_frames'] == len(captured)
    if metadata['sink'] == 'coreaudio-audioqueue':
        assert metadata['consumed_frames'] == metadata['submitted_frames'] == len(captured)
    truncated = []
    for token in starts.keys() - ends.keys():
        row, source, _ = sources[token]
        step = row['rate_fixed'] * 65536 // 48000
        natural_end = starts[token] + ((len(source) << 32) + step - 1) // step
        # Closing the host may cut off an active voice. Do not invent a guest
        # completion/cancellation or accept a missing event earlier in PCM.
        assert starts[token] <= len(captured) < natural_end, 'missing completion inside captured audio'
        ends[token] = len(captured)
        truncated.append(token)
    mix = np.zeros(captured.shape, dtype=np.int64)
    inputs = []
    for token, (row, source, amplitude) in sources.items():
        start, end = starts[token], ends[token]
        assert 0 <= start <= end <= len(mix)
        # Independent integer model: fixed-point source rate, interpolation,
        # signed amplitude floor, wide summation, single final saturation.
        phase = np.arange(end - start, dtype=np.int64) * (row['rate_fixed'] * 65536 // 48000)
        index, fraction = phase >> 32, phase & 0xffffffff
        assert not len(index) or index[-1] < len(source)
        first = source[index]
        second = source[np.minimum(index + 1, len(source) - 1)]
        value = first + ((second - first) * fraction // (1 << 32))
        value = value * amplitude // 255
        mix[start:end] += value[:, None]
        inputs.append(dict(token=token, start=start, end=end, amplitude=amplitude,
                           source_sha256=hashlib.sha256((source // 256 + 128).astype('u1').tobytes()).hexdigest()))
    expected = np.clip(mix, -32768, 32767).astype('<i2')
    delta = expected.astype(np.int64) - captured
    different = int(np.count_nonzero(delta))
    return dict(scope='Original guest buffer rendering at recorded runtime event times; native timing parity separate',
                alignment='Absolute captured output-frame indices; no offset fitting, resampling, gain/DC correction',
                frames=len(captured), sample_rate=48000, channels=2,
                expected_pcm_sha256=hashlib.sha256(expected.tobytes()).hexdigest(),
                captured_pcm_sha256=hashlib.sha256(captured.tobytes()).hexdigest(),
                different_samples=different, max_abs_error=int(np.max(np.abs(delta))),
                rms_error=float(np.sqrt(np.mean(delta.astype(float) ** 2))),
                sample_exact=different == 0, shutdown_truncated_tokens=sorted(truncated), inputs=inputs)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture_directory', type=Path)
    parser.add_argument('report', type=Path)
    args = parser.parse_args()
    assert not args.report.exists(), 'report must be new'
    result = compare(args.capture_directory)
    args.report.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result.items() if k != 'inputs'}))
    if not result['sample_exact']:
        raise SystemExit(1)
