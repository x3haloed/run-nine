"""Strict retained fields for owned two-extended-operand native power capture."""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
HEX={'source_before_hex','source_after_hex','dest_before_hex','dest_after_hex'}
FIELDS=HEX|{'kind','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards'}
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','math'}or v['schema']!=1 or v['probe']!='sane-power-semantics-v1':raise ValueError('Wrong power profile')
    if not isinstance(v['math'],dict)or set(v['math'])!={'records'}:raise ValueError('Missing power records')
    rows=v['math']['records'];operands=(ROOT/'probes/sanepower/operands.bin').read_bytes()
    if not isinstance(rows,list)or len(rows)!=10671:raise ValueError('Incomplete power matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing power field')
        if any(type(r[k])is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Missing power scalar')
        kind=i//45 if i<9900 else 220+(i-9900)//3
        mode=MODES[i//3%15]if i<9900 else 0
        if (r['kind'],r['input_env'],r['input_ccr'])!=(kind,mode,[16,31,0][i%3]):raise ValueError('Changed power context')
        for k in HEX:
            if not isinstance(r[k],str)or re.fullmatch('[0-9a-f]{20}',r[k])is None:raise ValueError('Invalid power packed operand')
        if r['source_before_hex']!=operands[kind*20:kind*20+10].hex()or r['dest_before_hex']!=operands[kind*20+10:kind*20+20].hex():raise ValueError('Changed power operand')
        if not isinstance(r['guards'],list)or len(r['guards'])!=4 or any(type(x)is not int for x in r['guards']):raise ValueError('Missing power guards')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in ['sane-power-raw-v1','sane-power-runtime-raw-v1']:raise ValueError('Wrong raw power capture')
    v={'schema':1,'probe':'sane-power-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
