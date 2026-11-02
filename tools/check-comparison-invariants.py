#!/usr/bin/env python3
"""Check that idle normalization cannot hide a changed outcome or lifecycle."""
import copy
import json
from pathlib import Path
import tempfile
from compare import read_probe,differences
ROOT=Path(__file__).resolve().parents[1]
def main():
    raw=json.loads((ROOT/'outputs/SaveConfirmProbe-reference.json').read_text())
    scratch=ROOT/'work/comparison-invariants';scratch.mkdir(exist_ok=True,parents=True)
    with tempfile.TemporaryDirectory(dir=scratch) as temp:
        a=Path(temp)/'reference.json';b=Path(temp)/'candidate.json';a.write_text(json.dumps(raw))
        def matches(value):
            b.write_text(json.dumps(value));return not differences(read_probe(a),read_probe(b))
        assert matches(raw)
        value=copy.deepcopy(raw);hook=value['save']['new']['hook'];index=next(i for i,v in enumerate(hook) if v[1]==100);hook.insert(index,hook[index].copy());assert matches(value)
        value=copy.deepcopy(raw);value['save']['replace_accept']['good']=0;assert not matches(value)
        value=copy.deepcopy(raw);hook=value['save']['replace_accept']['hook'];del hook[next(i for i,v in enumerate(hook) if v==[int.from_bytes(b'rplc','big'),-1])];assert not matches(value)
        value=copy.deepcopy(raw);value['save']['new']['hook']=[v for v in value['save']['new']['hook'] if v[1]!=100];assert not matches(value)
        value=copy.deepcopy(raw);value['save']['replace_dialog']['items'][0]['rect'][0]+=1;assert not matches(value)
    print(json.dumps({'checks':6,'passed':True}))
if __name__=='__main__':main()
