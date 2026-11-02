#!/usr/bin/env python3
"""Validate raw native capture and derive its explicit deterministic semantic profile."""
import copy
import importlib.util
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
p=ROOT/'outputs/ControlThumbProbe-reference.json';raw=json.loads(p.read_text())
assert raw['schema']==1 and raw['probe']=='control-thumb-v1'
rows=raw['controls']['cases'];assert len(rows)==6
keys={'action','start','entry_mouse','entry_button','event_age','duration','end','result','value','hilite','hook_count','hook'}
projected=[]
for i,r in enumerate(rows):
    assert set(r)==keys and r['action']==i//3
    assert all(type(r[k]) is int for k in keys-{'start','entry_mouse','end','hook'})
    assert r['entry_button']==1 and r['event_age']>=0 and r['duration']>0
    assert r['start']==[102,560] and r['entry_mouse']==r['start'] and r['end']==[[122,560],[82,560],[102,560]][i%3]
    if r['action']:
        assert r['hook_count']>=8 and len(r['hook'])==8
        assert all(isinstance(h,list) and len(h)==5 and all(type(n) is int for n in h) for h in r['hook'])
    else:assert r['hook_count']==0 and r['hook']==[]
    projected.append({**{k:r[k] for k in ['action','start','entry_mouse','entry_button','end','result','value','hilite']},'first_callback':r['hook'][0][:4] if r['hook'] else []})
value={'schema':1,'probe':'control-thumb-semantics-v1','controls':{'cases':projected}}
out=ROOT/'outputs/ControlThumbSemanticsProbe-reference.json';out.write_text(json.dumps(value,indent=2)+'\n')
spec=importlib.util.spec_from_file_location('probe_compare',ROOT/'tools/compare.py');mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
assert mod.read_probe(out)==value
# Missing observations cannot silently disappear from the scoped comparison.
for key in ['entry_mouse','entry_button','end','first_callback']:
    broken=copy.deepcopy(value);del broken['controls']['cases'][3][key]
    scratch=ROOT/'work/validation/control-thumb-incomplete.json';scratch.parent.mkdir(parents=True,exist_ok=True);scratch.write_text(json.dumps(broken))
    try:mod.read_probe(scratch)
    except ValueError:pass
    else:raise AssertionError('Accepted missing '+key)
print(json.dumps({'cases':6,'projection':'All deterministic gesture state plus first callback; raw timing/repeats retained','missing_observation_rejections':4}))
