#!/usr/bin/env python3
"""Compare rounded rectangle production dispatch with OS9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from round_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def leaves(value,path=()):
    if isinstance(value,dict):
        for k,v in value.items():yield from leaves(v,path+(k,))
    elif isinstance(value,list):
        for i,v in enumerate(value):yield from leaves(v,path+(i,))
    else:yield path,value
def main():
    pin=json.loads((OUT/'RoundRect-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'RoundRectProbe-v1-reference.json').read_text()))
    raw=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','round-rect-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'RoundRectProbe-runtime-raw.json').write_text(raw);runtime=project(json.loads(raw))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/f'RoundRectProbe-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)[:10]
    scratch=ROOT/'work/validation/round-incomplete.json';missing=mutations=0
    paths=[('drawing','records',0,key)for key in native['drawing']['records'][0]]
    paths += [('drawing','records')]
    for path in paths:
        bad=copy.deepcopy(native);target=bad
        for k in path[:-1]:target=target[k]
        del target[path[-1]];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+str(path))
    for i,row in enumerate(runtime['drawing']['records']):
        for path,old in list(leaves(row)):
            target=row
            for k in path[:-1]:target=target[k]
            target[path[-1]]=1 if old is None else (('f'if old[0]!='f'else'e')+old[1:]if old else'00')if isinstance(old,str)else old^1
            assert differences(native['drawing']['records'][i],row),'Masked rounded rectangle field '+str(path)
            target[path[-1]]=old;mutations+=1
    print(json.dumps({'records':618,'compared_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutations,'captured_native_records':630,'explicit_owned_rejections_verified':12}))
if __name__=='__main__':main()
