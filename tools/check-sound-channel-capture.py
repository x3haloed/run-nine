#!/usr/bin/env python3
"""Check the native v2 queue fixture's pitch and half-right volume in raw PCM.

Sanity thresholds apply only to the measurement transport, not runtime parity.
No gain fitting is applied. Retain full-buffer comparison errors separately.
"""
import importlib.util
import json
import struct
import wave
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('pcm',ROOT/'tools/compare-pcm-capture.py')
pcm = importlib.util.module_from_spec(spec);spec.loader.exec_module(pcm)
expected_path = ROOT/'outputs/SoundCommandsProbe-v2-ideal.wav'
capture_path = ROOT/'outputs/SoundCommandsProbe-v2-finalized.wav'
with wave.open(str(expected_path),'wb') as stream:
    stream.setparams((2,2,48000,0,'NONE','not compressed'))
    # Independent zero-order-hold model of the owned 8kHz/8000-byte square
    # input. 6 output frames per input sample,32 input samples per half-cycle.
    stream.writeframes(b''.join(struct.pack('<hh',s*256,s*128)
        for i in range(48000) for s in [32 if (i//192)&1 else -32]))
thresholds = {'frequency_hz':125, 'max_frequency_error_hz':1,
              'right_left_rms_ratio':0.5, 'max_ratio_error':0.01,
              'minimum_raw_correlation':0.98}
result = pcm.compare(expected_path,capture_path,'native-qemu')
data,rate = pcm.read(capture_path)
lag = result['alignment_frames']
if lag is None:raise ValueError('No measured alignment')
aligned = data[lag:lag+48000]
steady = aligned[6000:42000]
rms = np.sqrt(np.mean(steady**2,axis=0))
crossings = np.flatnonzero((steady[:-1,0]<=0)&(steady[1:,0]>0))
period = float(np.median(np.diff(crossings))) if len(crossings)>1 else 0
frequency = rate/period if period else 0
ratio = float(rms[1]/rms[0]) if rms[0] else 0
passed = (abs(frequency-125)<=thresholds['max_frequency_error_hz'] and
          abs(ratio-0.5)<=thresholds['max_ratio_error'] and
          result['correlation_coefficient']>=thresholds['minimum_raw_correlation'])
report = {'scope':'Reference transport sanity only; not independent runtime audio or native sample-exact equivalence',
          'ideal_model':'8000Hz unsigned8 square, zero-order hold to48000Hz; left unity,right one-half',
          'thresholds':thresholds, 'stationary_window_frames':[6000,42000],
          'frequency_hz':frequency, 'median_period_frames':period,
          'rms_per_channel':rms.tolist(), 'right_left_rms_ratio':ratio,
          'full_comparison':result, 'passed':bool(passed)}
(ROOT/'outputs/SoundCommandsProbe-v2-comparison.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
if not passed:raise SystemExit(1)
