#!/usr/bin/env python3
"""Run owned-vector buffers through the actual headless and optional live sinks.

This verifies the shared renderer/output adapter, not guest or game integration.
"""
import argparse
import hashlib
import importlib.util
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('pcm', ROOT/'tools/compare-pcm-capture.py')
pcm = importlib.util.module_from_spec(spec);spec.loader.exec_module(pcm)

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable',type=Path)
    parser.add_argument('output_directory',type=Path)
    parser.add_argument('--live',action='store_true')
    args=parser.parse_args();exe=args.executable.resolve();out=args.output_directory.resolve()
    out.mkdir(parents=True,exist_ok=False)
    source=ROOT/'outputs/SoundPCM-owned-input-u8.raw'
    expected=ROOT/'outputs/SoundPCM-owned-ideal-s16.wav'
    assert digest(source)=='3f0a31cc29fe42baf0e98b51116362749053bf913fc24302a60d907a1091f976'
    results=[]
    for mode in ['headless']+(['live'] if args.live else []):
        capture=out/(mode+'.wav')
        command=[str(exe),'--audio-vector',str(source),str(capture)]+(['--live'] if mode=='live' else [])
        with (out/(mode+'.log')).open('w') as log:
            subprocess.run(command,check=True,stdout=log,stderr=subprocess.STDOUT)
        result=pcm.compare(expected,capture,'host-runtime','sound-pcm-owned-v1')
        (out/(mode+'-comparison.json')).write_text(json.dumps(result,indent=2)+'\n')
        metadata=json.loads(capture.with_suffix('.json').read_text())
        events=json.loads(capture.with_suffix('.events.json').read_text())
        assert result['sample_exact'] and result['alignment_frames']==0,(mode,result)
        assert metadata['rendered_frames']==57600 and metadata['nonzero_samples']==47906
        assert not metadata['capture_overflow'] and not metadata['device_errors']
        assert [(e['event'],e['frame']) for e in events]==[('started',0),('finished',48000),('callback',48000)]
        assert events[-1]['command']==13 and events[-1]['param1']==123 and events[-1]['param2']==0x12345678
        # Also compare the entire file, including the explicit0.2s silence tail,
        # rather than allowing correlation to overlook extra/dropout samples.
        data,rate=pcm.read(capture);ideal,ideal_rate=pcm.read(expected)
        assert rate==ideal_rate==48000 and len(data)==57600
        assert (data[:48000]==ideal).all() and not data[48000:].any()
        if mode=='live':
            assert metadata['sink']=='coreaudio-audioqueue'
            assert metadata['consumed_frames']==metadata['submitted_frames']==57600
            assert metadata['device_callbacks']==120
            assert capture.read_bytes()==(out/'headless.wav').read_bytes()
        results.append({'mode':mode,'capture_sha256':digest(capture),'sample_exact':True,
                        'metadata':metadata,'events':events})
    guard=out/'guard.events.json';guard.write_bytes(b'preserved evidence')
    rejected=subprocess.run([str(exe),'--audio-vector',str(source),str(out/'guard.wav')],capture_output=True,text=True)
    (out/'guard-rejection.log').write_text(rejected.stdout+rejected.stderr)
    assert rejected.returncode!=0 and 'already exists' in rejected.stderr
    assert guard.read_bytes()==b'preserved evidence' and not (out/'guard.wav').exists() and not (out/'guard.json').exists()
    report={'scope':'Owned renderer and sink verification only; no guest/game playback or physical-speaker recording claim',
            'executable':str(exe),'executable_sha256':digest(exe),'cases':results,'passed':True}
    report['existing_event_report_preserved']=True
    (out/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'passed':True,'modes':[x['mode'] for x in results],'result':str(out/'result.json')}))

if __name__=='__main__':main()
