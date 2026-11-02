#!/usr/bin/env python3
"""Compare production dispatch with pinned native observations."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from osevent_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'OSEvent-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native input: '+f
    assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'OSEventProbe-v2-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','os-events-runtime-raw-v2'],cwd=ROOT,text=True)
    (OUT/'OSEventProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/f'OSEventProbe-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)[:10]
    scratch=ROOT/'work/validation/osevent-incomplete.json';missing=mutations=0
    for key in native['events']['records'][0]:
        bad=copy.deepcopy(native);del bad['events']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
    bad=copy.deepcopy(native);del bad['events']['records'];scratch.write_text(json.dumps(bad))
    try:read_probe(scratch)
    except ValueError:missing+=1
    else:raise AssertionError('Accepted missing records')
    # Full matrix compared above; exercise every field at empty/success boundaries.
    for index in [0,72,len(runtime['events']['records'])-1]:
        row=runtime['events']['records'][index]
        for key,old in list(row.items()):
            row[key]=old+[0] if isinstance(old,list) else 1 if old is None else ('f' if old[0]!='f' else 'e')+old[1:] if isinstance(old,str) else old^1
            assert differences(native,runtime),'Masked field '+key
            row[key]=old;mutations+=1
    print(json.dumps({'records':480,'compared_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutations}))
if __name__=='__main__':main()
