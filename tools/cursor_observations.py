"""Strict native cursor nesting and real-motion observations, no normalization."""
import copy,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OPS=[0,2,2,1,1,2,2,2,1,3,2,1,2,0,3,1,1,2,2,2,0]
COMMON={'before_state','before_visible','before_8d2','after_state','after_visible','after_8d2'}
STATE_FIELDS=COMMON|{'input_d0','input_ccr','step','op','d0','ccr','stack_delta','guard','mem_error','res_error'}
MOTION_FIELDS=COMMON|{'hidden','start','end'}
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','cursors'}or v['schema']!=1 or v['probe']not in['cursor-state-v1','cursor-motion-v1']:raise ValueError('Wrong cursor profile')
    if not isinstance(v['cursors'],dict)or set(v['cursors'])!={'records'}:raise ValueError('Missing cursor records')
    motion=v['probe']=='cursor-motion-v1';fields=MOTION_FIELDS if motion else STATE_FIELDS;rows=v['cursors']['records']
    if not isinstance(rows,list)or len(rows)!=(2 if motion else 126):raise ValueError('Incomplete cursor matrix')
    inputs=json.loads((ROOT/'probes/cursormotion/movements.json').read_text())['movements']if motion else None
    for i,r in enumerate(rows):
        if not isinstance(r,dict)or set(r)!=fields:raise ValueError('Missing cursor field')
        if any(type(r[k])is not int for k in fields-{'start','end'}):raise ValueError('Invalid cursor scalar')
        if motion:
            for key in ['hidden','start','end']:
                if r[key]!=inputs[i][key]:raise ValueError('Changed native movement input')
            for key in ['start','end']:
                if not isinstance(r[key],list)or len(r[key])!=2 or any(type(n)is not int for n in r[key]):raise ValueError('Invalid cursor point')
            if r['start']==r['end']:raise ValueError('Missing real native movement')
        elif(r['input_d0'],r['input_ccr'],r['step'],r['op'])!=([0x12345678,0][i//63],[16,31,0][i//21%3],i%21,OPS[i%21]):raise ValueError('Changed cursor context')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1:raise ValueError('Wrong raw cursor capture')
    profiles={'cursor-state-raw-v1':'cursor-state-v1','cursor-state-runtime-raw-v1':'cursor-state-v1','cursor-motion-raw-v1':'cursor-motion-v1','cursor-motion-runtime-raw-v1':'cursor-motion-v1'}
    if raw['probe']not in profiles:raise ValueError('Wrong raw cursor profile')
    v={'schema':1,'probe':profiles[raw['probe']],'cursors':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
