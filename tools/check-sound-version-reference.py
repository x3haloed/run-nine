#!/usr/bin/env python3
"""Pin the native discovery ABI; production still declares no sound backend."""
import copy
import hashlib
import json
from pathlib import Path
from compare import read_probe,differences
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def project(raw):
    assert set(raw)=={'schema','probe','cases'} and raw['schema']==1 and raw['probe']=='sound-version-raw-v3'
    rows=raw['cases'];assert len(rows)==3
    result=[]
    for i,row in enumerate(rows):
        assert set(row)=={'input_ccr','d0','raw_a0','sp_delta','stack_value','stack_tail','ccr','record_hex'}
        assert row['input_ccr']==[16,31,0][i]
        assert all(type(row[k]) is int for k in ['input_ccr','d0','raw_a0','sp_delta','stack_value','ccr'])
        # D0 and A0 are documented caller-clobbered Toolbox scratch registers.
        # Their raw native values remain in the capture, never compared as
        # pointers into the independent runtime. All result/guard bytes remain.
        result.append({k:v for k,v in row.items() if k not in ['d0','raw_a0']})
    assert all(row['stack_value']==rows[0]['stack_value'] for row in rows)
    return {'schema':1,'probe':'sound-version-abi-v1','sound':{'reference_profile_version':rows[0]['stack_value'],'cases':result}}
def main():
    pin=json.loads((OUT/'SoundVersion-provenance.json').read_text())
    for key,hashkey in [('source','source_sha256'),('reference_path','reference_sha256'),('fixture','resource_sha256')]:
        assert hashlib.sha256((ROOT/pin[key]).read_bytes()).hexdigest()==pin[hashkey]
    raw=json.loads((ROOT/pin['reference_path']).read_text());value=project(raw)
    path=OUT/'SoundVersionABIProbe-reference.json';path.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(path)==value
    scratch=ROOT/'work/validation/sound-version-incomplete.json'
    for key in ['input_ccr','sp_delta','stack_value','stack_tail','ccr','record_hex']:
        broken=copy.deepcopy(value);del broken['sound']['cases'][0][key];scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:pass
        else:raise AssertionError('Accepted missing '+key)
    changed=copy.deepcopy(raw);changed['cases'][0]['stack_tail'][0]^=1
    assert differences(value,project(changed)),'Lost stack overwrite'
    print(json.dumps({'cases':3,'missing_observation_rejections':6,'raw_stack_mutation_detected':True,'scope':'Explicit native version profile for ABI only; production version zero and no sound backend.'}))
if __name__=='__main__':main()
