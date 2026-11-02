#!/usr/bin/env python3
"""Production shared cursor state against native CPU and real-motion captures."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import differences,read_probe
from cursor_observations import project,STATE_FIELDS,MOTION_FIELDS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    for name in['CursorState','CursorMotion']:
        pin=json.loads((OUT/(name+'-provenance.json')).read_text())
        for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed cursor input: '+f
        assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    subprocess.run(['cargo','test','--quiet','--manifest-path',str(ROOT/'compat-core/Cargo.toml'),'--features','hosted-cpu','cursor::tests'],check=True,stdout=subprocess.DEVNULL)
    summary=[]
    for name,probe,fields in[('CursorStateProbe','cursor-state-runtime-raw-v1',STATE_FIELDS),('CursorMotionProbe','cursor-motion-runtime-raw-v1',MOTION_FIELDS)]:
        native=project(json.loads((OUT/(name+'-v1-reference.json')).read_text()))
        text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe',probe],cwd=ROOT,text=True)
        (OUT/(name+'-runtime-raw.json')).write_text(text);runtime=project(json.loads(text))
        for label,v in[('reference',native),('runtime',runtime)]:
            p=OUT/(name+'-'+label+'.json');p.write_text(json.dumps(v,indent=2)+'\n');assert read_probe(p)==v
        assert not differences(native,runtime),differences(native,runtime)
        missing=mutated=0;scratch=ROOT/'work/validation/cursor-incomplete.json'
        for key in fields:
            bad=copy.deepcopy(native);del bad['cursors']['records'][0][key];scratch.write_text(json.dumps(bad))
            try:read_probe(scratch)
            except ValueError:missing+=1
            else:raise AssertionError('Accepted missing '+key)
        for i in range(len(native['cursors']['records'])):
            for key in fields:
                bad=copy.deepcopy(native);row=bad['cursors']['records'][i]
                if isinstance(row[key],list):row[key][0]^=1
                else:row[key]^=1
                assert differences(native,bad),'Masked '+key;mutated+=1
        summary.append({'probe':name,'records':len(native['cursors']['records']),'all_fields_bit_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutated})
    print(json.dumps({'profiles':summary,'deep_nested_hide_and_motion_test':True,'native_pixel_equivalence':False}))
if __name__=='__main__':main()
