"""Strict, bit-exact FP68K extended classification observations. No numeric tolerance."""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
HEX={'source_before_hex','source_after_hex','dest_hex'}
FIELDS=HEX|{'selector','kind','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards'}
def validate(value):
    if not isinstance(value,dict) or set(value)!={'schema','probe','math'} or value['schema']!=1 or value['probe']!='sane-class-semantics-v1':raise ValueError('Wrong SANE extended classification profile')
    math=value['math']
    if not isinstance(math,dict) or set(math)!={'records'}:raise ValueError('Missing extended classification records')
    rows=math['records'];operands=(ROOT/'probes/saneconvert/operands.bin').read_bytes()
    if not isinstance(rows,list) or len(rows)!=10350:raise ValueError('Incomplete extended classification matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict) or set(r)!=FIELDS:raise ValueError('Missing extended classification field')
        if any(type(r[k]) is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Missing extended classification scalar')
        slot=i;kind=slot//45
        if (r['selector'],r['kind'],r['input_env'],r['input_ccr'])!=(28,kind,MODES[slot//3%15],[16,31,0][slot%3]):raise ValueError('Changed conversion context')
        for k in HEX:
            if not isinstance(r[k],str) or re.fullmatch('[0-9a-f]{4}' if k=='dest_hex' else '[0-9a-f]{20}',r[k]) is None:raise ValueError('Invalid packed operand')
        if r['source_before_hex']!=operands[kind*10:kind*10+10].hex():raise ValueError('Changed owned conversion operand')
        if not isinstance(r['guards'],list) or len(r['guards'])!=4 or any(type(x) is not int for x in r['guards']):raise ValueError('Missing extended classification guards')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe'] not in ['sane-class-raw-v1','sane-class-runtime-raw-v1']:raise ValueError('Wrong raw extended classification capture')
    v={'schema':1,'probe':'sane-class-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
