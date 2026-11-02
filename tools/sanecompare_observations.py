"""Strict, bit-exact quiet/signaling FP68K comparisons. No numeric tolerance."""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
HEX={'source_before_hex','dest_before_hex','source_after_hex','dest_after_hex'}
FIELDS=HEX|{'selector','kind','source_width','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards','source_padding'}
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','math'}or v['schema']!=1 or v['probe']!='sane-compare-semantics-v1':raise ValueError('Wrong comparison profile')
    if not isinstance(v['math'],dict)or set(v['math'])!={'records'}:raise ValueError('Missing comparison records')
    rows=v['math']['records'];x=(ROOT/'probes/sanecompare/sources_x.bin').read_bytes();d=(ROOT/'probes/sanecompare/sources_d.bin').read_bytes();dst=(ROOT/'probes/sanecompare/destinations.bin').read_bytes()
    if not isinstance(rows,list)or len(rows)!=23040:raise ValueError('Incomplete comparison matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing comparison field')
        if any(type(r[k])is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Invalid comparison scalar')
        slot=i%5760;kind=slot//45;selector=[8,2056,10,2058][i//5760];width=8 if selector&2048 else 10;source=d if width==8 else x
        if(r['selector'],r['kind'],r['source_width'],r['input_env'],r['input_ccr'])!=(selector,kind,width,MODES[slot//3%15],[16,31,0][slot%3]):raise ValueError('Changed comparison context')
        for key in HEX:
            length=width*2 if key.startswith('source')else 20
            if not isinstance(r[key],str)or re.fullmatch('[0-9a-f]{'+str(length)+'}',r[key])is None:raise ValueError('Invalid comparison bytes')
        if r['source_before_hex']!=source[kind*width:(kind+1)*width].hex()or r['dest_before_hex']!=dst[kind*10:(kind+1)*10].hex():raise ValueError('Changed comparison input')
        if not isinstance(r['guards'],list)or len(r['guards'])!=4 or any(type(x)is not int for x in r['guards']):raise ValueError('Invalid comparison guards')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in ['sane-compare-raw-v1','sane-compare-runtime-raw-v1']:raise ValueError('Wrong raw comparison capture')
    v={'schema':1,'probe':'sane-compare-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
