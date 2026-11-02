"""Strict, bit-exact FP68K integer widening observations. No numeric tolerance."""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
HEX={'source_before_hex','source_after_hex','dest_hex'}
FIELDS=HEX|{'kind','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards'}
def validate(value):
    if not isinstance(value,dict) or set(value)!={'schema','probe','math'} or value['schema']!=1 or value['probe']!='sane-integer-semantics-v1':raise ValueError('Wrong SANE integer widening profile')
    math=value['math']
    if not isinstance(math,dict) or set(math)!={'records'}:raise ValueError('Missing integer widening records')
    rows=math['records'];operands=(ROOT/'probes/saneinteger/operands.bin').read_bytes()
    if not isinstance(rows,list) or len(rows)!=7200:raise ValueError('Incomplete integer widening matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict) or set(r)!=FIELDS:raise ValueError('Missing integer widening field')
        if any(type(r[k]) is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Missing integer widening scalar')
        kind=i//45
        if (r['kind'],r['input_env'],r['input_ccr'])!=(kind,MODES[i//3%15],[16,31,0][i%3]):raise ValueError('Changed conversion context')
        for k in HEX:
            if not isinstance(r[k],str) or re.fullmatch('[0-9a-f]{20}' if k=='dest_hex' else '[0-9a-f]{4}',r[k]) is None:raise ValueError('Invalid packed operand')
        if r['source_before_hex']!=operands[kind*2:kind*2+2].hex():raise ValueError('Changed owned conversion operand')
        if not isinstance(r['guards'],list) or len(r['guards'])!=4 or any(type(x) is not int for x in r['guards']):raise ValueError('Missing integer widening guards')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe'] not in ['sane-integer-raw-v1','sane-integer-runtime-raw-v1']:raise ValueError('Wrong raw integer widening capture')
    v={'schema':1,'probe':'sane-integer-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
