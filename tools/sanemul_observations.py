"""Strict, bit-exact FP68K multiplication observations. No numeric tolerance."""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
PAIRS=[(2,2),(4,4),(6,4),(11,6),(3,6),(3,7),(0,2),(1,2),(0,21),(21,0),(21,6),(23,3),(24,2),(32,33),(33,32),(35,2),(36,37),(15,4),(16,4),(17,4),(17,6),(18,4),(19,19),(20,6),(29,2),(30,21),(31,2),(8,9),(10,11),(13,13),(14,14),(20,4)]
HEX={'source_before_hex','dest_before_hex','source_after_hex','dest_after_hex'}
FIELDS=HEX|{'kind','source_index','dest_index','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards'}
def validate(value):
    if not isinstance(value,dict) or set(value)!={'schema','probe','math'} or value['schema']!=1 or value['probe']!='sane-mul-semantics-v1':raise ValueError('Wrong SANE multiplication profile')
    math=value['math']
    if not isinstance(math,dict) or set(math)!={'records'}:raise ValueError('Missing multiplication records')
    rows=math['records'];operands=(ROOT/'probes/sane/operands.bin').read_bytes()
    if not isinstance(rows,list) or len(rows)!=2880:raise ValueError('Incomplete multiplication matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict) or set(r)!=FIELDS:raise ValueError('Missing multiplication field')
        if any(type(r[k]) is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Missing multiplication scalar')
        kind=i//45;src,dst=PAIRS[kind] if kind<32 else (38+514+(kind-32)*3,38+514+((kind-32)*3+37)%128)
        if (r['kind'],r['source_index'],r['dest_index'],r['input_env'],r['input_ccr'])!=(kind,src,dst,MODES[i//3%15],[16,31,0][i%3]):raise ValueError('Changed multiplication context')
        for k in HEX:
            if not isinstance(r[k],str) or re.fullmatch('[0-9a-f]{20}',r[k]) is None:raise ValueError('Invalid packed operand')
        for key,index in [('source_before_hex',src),('dest_before_hex',dst)]:
            if r[key]!=operands[index*10:index*10+10].hex():raise ValueError('Changed owned multiplication operand')
        if not isinstance(r['guards'],list) or len(r['guards'])!=4 or any(type(x) is not int for x in r['guards']):raise ValueError('Missing multiplication guards')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe'] not in ['sane-mul-raw-v1','sane-mul-runtime-raw-v1']:raise ValueError('Wrong raw multiplication capture')
    v={'schema':1,'probe':'sane-mul-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
