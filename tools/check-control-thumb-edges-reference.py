#!/usr/bin/env python3
"""Validate two raw native captures and project all deterministic observations."""
import copy
import importlib.util
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
rows=[]
keys={'case','rect','initial_value','start','entry_mouse','entry_button','event_age','duration','end','result','value','hilite'}
for run,name in enumerate(['ControlThumbEdgesProbe','ControlThumbSlopProbe']):
    raw=json.loads((ROOT/f'outputs/{name}-reference.json').read_text())
    assert raw['schema']==1 and raw['probe']=='control-thumb-edges-v1'
    cases=raw['controls']['cases'];assert len(cases)==8
    for i,r in enumerate(cases):
        assert set(r)==keys and r['case']==i
        assert all(type(r[k]) is int for k in keys-{'rect','start','entry_mouse','end'})
        for k,n in [('rect',4),('start',2),('entry_mouse',2),('end',2)]:
            assert isinstance(r[k],list) and len(r[k])==n and all(type(v) is int for v in r[k])
        assert r['entry_mouse']==r['start'] and r['entry_button']==1
        assert r['initial_value']==25 and r['event_age']>=0 and r['duration']>0
        rows.append({'run':run,**{k:v for k,v in r.items() if k not in {'duration','event_age'}}})
value={'schema':1,'probe':'control-thumb-edges-semantics-v1','controls':{'cases':rows}}
out=ROOT/'outputs/ControlThumbEdgesSemanticsProbe-reference.json'
out.write_text(json.dumps(value,indent=2)+'\n')
spec=importlib.util.spec_from_file_location('probe_compare',ROOT/'tools/compare.py')
mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
assert mod.read_probe(out)==value
for key in keys-{'event_age','duration'}:
    broken=copy.deepcopy(value);del broken['controls']['cases'][0][key]
    scratch=ROOT/'work/validation/control-thumb-edges-incomplete.json'
    scratch.write_text(json.dumps(broken))
    try:mod.read_probe(scratch)
    except ValueError:pass
    else:raise AssertionError('Accepted missing '+key)
print(json.dumps({'cases':16,'missing_observation_rejections':10,'projection':'All deterministic observations; raw event age/duration retained separately'}))
