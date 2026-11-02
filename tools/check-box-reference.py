#!/usr/bin/env python3
"""Compare TETextBox production dispatch and renderer observations with OS9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from box_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def leaves(value,path=()):
    if isinstance(value,dict):
        for k,v in value.items():yield from leaves(v,path+(k,))
    elif isinstance(value,list):
        for i,v in enumerate(value):yield from leaves(v,path+(i,))
    else:yield path,value
def main():
    pin=json.loads((OUT/'TextBox-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'TextBoxProbe-v3-reference.json').read_text()))
    raw=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','text-box-runtime-raw-v3'],cwd=ROOT,text=True)
    (OUT/'TextBoxProbe-runtime-raw.json').write_text(raw);runtime=project(json.loads(raw))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/f'TextBoxProbe-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)[:10]
    scratch=ROOT/'work/validation/box-incomplete.json';missing=mutations=0
    paths=[('text','records',0,key)for key in native['text']['records'][0]]
    paths += [('text','records',0,'calls',0,key)for key in native['text']['records'][0]['calls'][0]]
    paths += [('text','records')]
    for path in paths:
        bad=copy.deepcopy(native);target=bad
        for k in path[:-1]:target=target[k]
        del target[path[-1]];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+str(path))
    for i,row in enumerate(runtime['text']['records']):
        for path,old in list(leaves(row)):
            target=row
            for k in path[:-1]:target=target[k]
            target[path[-1]]=1 if old is None else (('f'if old[0]!='f'else'e')+old[1:]if old else'00')if isinstance(old,str)else old^1
            assert differences(native['text']['records'][i],row),'Masked box field '+str(path)
            target[path[-1]]=old;mutations+=1
    print(json.dumps({'records':1152,'compared_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutations,'raster_scope':'Full empty-box raster; nonempty glyph pixels retained raw,unclaimed','callback_scope':'Native textProc arguments versus owned renderer observations; guest QDProcs execution is not claimed'}))
if __name__=='__main__':main()
