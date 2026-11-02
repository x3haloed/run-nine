"""Strict, bit-exact FP68K long widening observations. No numeric tolerance."""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
HEX={'source_before_hex','source_after_hex','dest_hex'}
FIELDS=HEX|{'kind','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards'}
def validate(value):
    if not isinstance(value,dict) or set(value)!={'schema','probe','math'} or value['schema']!=1 or value['probe']!='sane-long-widen-semantics-v1':raise ValueError('Wrong SANE long widening profile')
    math=value['math']
    if not isinstance(math,dict) or set(math)!={'records'}:raise ValueError('Missing long widening records')
    rows=math['records'];operands=(ROOT/'probes/sanei32widen/operands.bin').read_bytes()
    if not isinstance(rows,list) or len(rows)!=6885:raise ValueError('Incomplete long widening matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict) or set(r)!=FIELDS:raise ValueError('Missing long widening field')
        if any(type(r[k]) is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Missing long widening scalar')
        kind=i//45
        if (r['kind'],r['input_env'],r['input_ccr'])!=(kind,MODES[i//3%15],[16,31,0][i%3]):raise ValueError('Changed conversion context')
        for k in HEX:
            if not isinstance(r[k],str) or re.fullmatch('[0-9a-f]{20}' if k=='dest_hex' else '[0-9a-f]{8}',r[k]) is None:raise ValueError('Invalid packed operand')
        if r['source_before_hex']!=operands[kind*4:kind*4+4].hex():raise ValueError('Changed owned conversion operand')
        if not isinstance(r['guards'],list) or len(r['guards'])!=4 or any(type(x) is not int for x in r['guards']):raise ValueError('Missing long widening guards')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe'] not in ['sane-long-widen-raw-v1','sane-long-widen-runtime-raw-v1']:raise ValueError('Wrong raw long widening capture')
    v={'schema':1,'probe':'sane-long-widen-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
