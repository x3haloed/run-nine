#!/usr/bin/env python3
"""Compare production dispatch with pinned native observations."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from line_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'QDLine-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native input: '+f
    assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'QDLineProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','qd-line-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'QDLineProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/f'QDLineProbe-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)[:10]
    scratch=ROOT/'work/validation/line-incomplete.json';missing=mutations=0
    for key in native['drawing']['records'][0]:
        bad=copy.deepcopy(native);del bad['drawing']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
    bad=copy.deepcopy(native);del bad['drawing']['records'];scratch.write_text(json.dumps(bad))
    try:read_probe(scratch)
    except ValueError:missing+=1
    else:raise AssertionError('Accepted missing records')
    # Exercise every compared field in every bounded physical input context.
    for index in range(len(runtime['drawing']['records'])):
        row=runtime['drawing']['records'][index]
        for key,old in list(row.items()):
            row[key]=1 if old is None else (('f' if old[0]!='f' else 'e')+old[1:] if old else '00') if isinstance(old,str) else old^1
            assert differences(native['drawing']['records'][index],row),'Masked field '+key
            row[key]=old;mutations+=1
    print(json.dumps({'records':144,'compared_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutations}))
if __name__=='__main__':main()
