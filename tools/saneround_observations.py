"""Strict unary integral rounding/truncation profile. Captured integer destinations remain separate and unimplemented. No tolerance."""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
HEX={'source_before_hex','source_after_hex','dest_before_hex','dest_after_hex'}
FIELDS=HEX|{'selector','kind','dest_width','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards'}
def validate(value):
    if not isinstance(value,dict)or set(value)!={'schema','probe','math'}or value['schema']!=1 or value['probe']!='sane-round-semantics-v1':raise ValueError('Wrong integral-rounding profile')
    if not isinstance(value['math'],dict)or set(value['math'])!={'records'}:raise ValueError('Missing rounding records')
    rows=value['math']['records'];operands=(ROOT/'probes/saneround/operands.bin').read_bytes()
    if not isinstance(rows,list)or len(rows)!=17010:raise ValueError('Incomplete rounding matrix')
    for index,r in enumerate(rows):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing rounding field')
        if any(type(r[k])is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Invalid rounding scalar')
        op=index//8505;slot=index%8505;kind=slot//45;width=[10,10,2,4][op]
        if (r['selector'],r['kind'],r['dest_width'],r['input_env'],r['input_ccr'])!=([20,22,8208,10256][op],kind,width,MODES[slot//3%15],[16,31,0][slot%3]):raise ValueError('Changed rounding context')
        source=operands[kind*10:kind*10+10].hex()
        for key in HEX:
            length=20 if key.startswith('source')else width*2
            if not isinstance(r[key],str)or re.fullmatch('[0-9a-f]{'+str(length)+'}',r[key])is None:raise ValueError('Invalid rounding bytes')
        if r['source_before_hex']!=source or r['dest_before_hex']!=(source if op<2 else 'a5'*width):raise ValueError('Changed rounding input')
        if not isinstance(r['guards'],list)or len(r['guards'])!=4 or any(type(x)is not int for x in r['guards']):raise ValueError('Invalid rounding guards')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in ['sane-round-raw-v1','sane-round-runtime-raw-v1']:raise ValueError('Wrong raw rounding capture')
    if len(raw['records']) != (34020 if raw['probe']=='sane-round-raw-v1' else 17010):raise ValueError('Incomplete raw rounding capture')
    value={'schema':1,'probe':'sane-round-semantics-v1','math':{'records':copy.deepcopy(raw['records'][:17010])}};validate(value);return value
