#!/usr/bin/env python3
"""Project explicit FCB identity roles while preserving every logical/physical observation."""
import copy
import hashlib
import json
from pathlib import Path
from compare import read_probe
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'FCB-provenance.json').read_text())
    assert hashlib.sha256((ROOT/pin['source']).read_bytes()).hexdigest()==pin['source_sha256']
    assert hashlib.sha256((OUT/pin['reference']).read_bytes()).hexdigest()==pin['reference_sha256']
    fixture=ROOT/'work/probes/fcbinfo/FCBInfoProbe.APPL/..namedfork/rsrc'
    assert fixture.is_file(),'Build the pinned fixture:sh tools/build-probe.sh fcbinfo'
    assert hashlib.sha256(fixture.read_bytes()).hexdigest()==pin['resource_sha256']
    raw=json.loads((OUT/'FCBStateProbe-reference.json').read_text());assert raw['schema']==1 and raw['probe']=='fcb-info-v1'
    assert set(raw)=={'schema','probe','raw_environment','files'}
    env=raw['raw_environment'];assert set(env)=={'volume','directory','app_ref','global_ref_same','global_name_same'} and all(type(n) is int for n in env.values())
    rows=[]
    for row in raw['files']['cases']:
        assert isinstance(row['raw_ids'],list) and len(row['raw_ids'])==2 and all(type(n) is int for n in row['raw_ids'])
        rows.append({k:v for k,v in row.items() if k!='raw_ids'})
    value={'schema':1,'probe':'fcb-semantics-v1','startup':{k:env[k] for k in ['global_ref_same','global_name_same']},'files':{'cases':rows}}
    out=OUT/'FCBSemanticsProbe-reference.json';out.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(out)==value
    scratch=ROOT/'work/validation/fcb-missing.json';keys=['name33_hex','flags','file_id','parent_same','volume_out','raw_physical','guards']
    for key in keys:
        broken=copy.deepcopy(value);del broken['files']['cases'][0][key];scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:pass
        else:raise AssertionError('Accepted missing '+key)
    print(json.dumps({'cases':18,'missing_observation_rejections':len(keys),'scope':'Raw volatile IDs retained separately and compared through explicit relations; every captured error, buffer byte, status, mark, length and allocation field retained. Startup ref/name relations included.'}))
if __name__=='__main__':main()
