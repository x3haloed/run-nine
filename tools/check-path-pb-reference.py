#!/usr/bin/env python3
"""Retain raw native IDs, project all deterministic path/PB state, reject missing fields."""
import copy
import hashlib
import json
from pathlib import Path
from compare import read_probe
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'PathPB-provenance.json').read_text())
    assert hashlib.sha256((ROOT/pin['source']).read_bytes()).hexdigest()==pin['source_sha256']
    assert hashlib.sha256((OUT/pin['reference']).read_bytes()).hexdigest()==pin['reference_sha256']
    raw=json.loads((OUT/'PathPBProbe-reference.json').read_text())
    assert raw['schema']==1 and raw['probe']=='path-files-pb-v1'
    assert set(raw)=={'schema','probe','raw_environment','files'}
    assert set(raw['raw_environment'])=={'volume','base','child','other','original_dir'}
    assert all(type(n) is int for n in raw['raw_environment'].values())
    value={'schema':1,'probe':'path-files-pb-semantics-v1','files':raw['files']}
    out=OUT/'PathPBSemanticsProbe-reference.json';out.write_text(json.dumps(value,indent=2)+'\n')
    assert read_probe(out)==value
    scratch=ROOT/'work/validation/path-pb-missing.json';scratch.parent.mkdir(parents=True,exist_ok=True)
    keys=['spec_name_hex','spec_parent','read','pb_ref','pb_io_result','failed_ref_preserved']
    for key in keys:
        broken=copy.deepcopy(value);del broken['files']['cases'][0][key];scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:pass
        else:raise AssertionError('Accepted missing '+key)
    print(json.dumps({'cases':28,'missing_field_rejections':len(keys),'projection':'Only raw volatile environment IDs excluded; all per-case state retained. Successful PB refnum captured as positive-validity relation; failure raw refnum exact.'}))
if __name__=='__main__':main()
