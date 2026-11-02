#!/usr/bin/env python3
"""Compare one identical68K caller's native/API results and actual sink buffers.

The probe runs guest machine code and shared trap dispatch. Game coverage is a
separate requirement; a passing probe never implies Marathon playback.
"""
import argparse,hashlib,importlib.util,json,struct,subprocess,wave
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('pcm',ROOT/'tools/compare-pcm-capture.py')
pcm=importlib.util.module_from_spec(spec);spec.loader.exec_module(pcm)

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('executable',type=Path);p.add_argument('reference',type=Path);p.add_argument('output',type=Path);p.add_argument('--live',action='store_true');a=p.parse_args()
    exe=a.executable.resolve();out=a.output.resolve();out.mkdir(parents=True,exist_ok=False)
    reference=json.loads(a.reference.read_text())
    data=bytes(128 if i<1200 or i>=3600 else 160 if ((i-1200)//32)&1 else 96 for i in range(4800))
    (out/'expected.raw').write_bytes(data)
    expected=out/'expected.wav'
    with wave.open(str(expected),'wb') as stream:
        stream.setparams((2,2,48000,0,'NONE','not compressed'));stream.writeframes(b''.join(struct.pack('<hh',(b-128)*256,(b-128)*128)for b in data))
    cases=[]
    for mode in ['headless']+(['coreaudio']if a.live else []):
        directory=out/mode;directory.mkdir();capture=directory/'output.wav'
        command=[str(exe),'--probe','sound-guest-v1','--audio-output',mode,'--capture-audio',str(capture),'--sound-log',str(directory/'trace.jsonl'),'--capture-audio-inputs',str(directory/'inputs')]
        run=subprocess.run(command,capture_output=True,text=True);(directory/'stdout.txt').write_text(run.stdout);(directory/'stderr.txt').write_text(run.stderr)
        assert run.returncode==0,(mode,run.returncode,run.stderr)
        observation=json.loads(run.stdout);assert observation==reference,(observation,reference)
        assert (directory/'inputs/buffer-000001.raw').read_bytes()==data
        events=[json.loads(s)for s in (directory/'trace.jsonl').read_text().splitlines()]
        starts=[e for e in events if e['event']=='started'];ends=[e for e in events if e['event']=='finished'];callbacks=[e for e in events if e['event']=='callback']
        assert len(starts)==len(ends)==len(callbacks)==1
        start=starts[0]['frame'];end=ends[0]['frame'];assert end-start==4800 and callbacks[0]['frame']==end
        result=pcm.compare(expected,capture,'host-runtime');assert result['sample_exact'] and result['alignment_frames']==start,result
        samples,rate=pcm.read(capture);assert rate==48000 and len(samples)>=end
        assert not samples[:start].any() and not samples[end:].any()
        metadata=json.loads(capture.with_suffix('.json').read_text())
        assert metadata['device_errors']==[] and not metadata['capture_overflow'] and metadata.get('drain_error') is None
        assert metadata['nonzero_samples']==4800
        if mode=='coreaudio':assert metadata['consumed_frames']==metadata['submitted_frames']==metadata['rendered_frames']==len(samples)
        (directory/'comparison.json').write_text(json.dumps(result,indent=2)+'\n')
        cases.append({'mode':mode,'native_results_equal':True,'sample_exact':True,'buffer_start_frame':start,'buffer_end_frame':end,'capture_sha256':hashlib.sha256(capture.read_bytes()).hexdigest(),'sink':metadata})
    report={'scope':'Identical owned68K program executes in native OS9 and independent CPU/guest dispatch; original game playback remains separate',
        'executable_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'program_sha256':hashlib.sha256((ROOT/'probes/soundguest/program.bin').read_bytes()).hexdigest(),'native_reference_sha256':hashlib.sha256(a.reference.read_bytes()).hexdigest(),'passed':True,'cases':cases}
    (out/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'passed':True,'result':str(out/'result.json')}))

if __name__=='__main__':main()
