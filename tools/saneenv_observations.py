"""Strict full-word FP68K procedure environment observations."""
import copy
MODES=[0,0x20,0x40,0x60,0x2000,0x4000,0x6000,0x80,0x100,0x200,0x400,0x800,0x1000,0x1f00,0x1f,0xffff,0x5a5a,0xa5a5,0x7f80,0xff00]
HEX=set()
FIELDS={'selector','input_env','input_word','input_ccr','word','output_env','d0','ccr','stack_delta','guards'}
def contexts():
    for phase in range(2):
        for saved in MODES:
            for current in ([saved]if phase==0 else MODES):
                if phase and(saved&31 or((current&31)&(current>>8)&31)):continue
                for flags in [16,31,0]:yield(23 if phase==0 else 25,current,0x5a5a if phase==0 else saved,flags)
def validate(value):
    if not isinstance(value,dict)or set(value)!={'schema','probe','math'}or value['schema']!=1 or value['probe']!='sane-env-semantics-v1':raise ValueError('Wrong procedure environment profile')
    if not isinstance(value['math'],dict)or set(value['math'])!={'records'}:raise ValueError('Missing procedure records')
    rows=value['math']['records'];expected=list(contexts())
    if not isinstance(rows,list)or len(rows)!=len(expected)or len(rows)!=876:raise ValueError('Incomplete procedure environments')
    for r,context in zip(rows,expected):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing procedure field')
        if any(type(r[k])is not int for k in FIELDS-{'guards'}):raise ValueError('Missing procedure scalar')
        if tuple(r[k]for k in ['selector','input_env','input_word','input_ccr'])!=context:raise ValueError('Changed procedure context')
        if not isinstance(r['guards'],list)or len(r['guards'])!=2 or any(type(v)is not int for v in r['guards']):raise ValueError('Missing procedure guard')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in['sane-env-raw-v1','sane-env-runtime-raw-v1']:raise ValueError('Wrong raw procedure capture')
    v={'schema':1,'probe':'sane-env-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
