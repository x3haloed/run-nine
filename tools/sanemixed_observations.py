"""Strict, bit-exact FP68K mixed arithmetic observations. No numeric tolerance."""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
PAIRS=[(2,2),(4,4),(6,4),(11,6),(3,6),(3,7),(0,2),(1,2),(0,21),(21,0),(21,6),(23,3),(24,2),(32,33),(33,32),(35,2),(36,37),(15,4),(16,4),(17,4),(17,6),(18,4),(19,19),(20,6),(29,2),(30,21),(31,2),(8,9),(10,11),(13,13),(14,14),(20,4),(0,0),(0,2),(1,3),(21,21),(23,21),(21,23),(2,0),(3,1)]
HEX={'source_before_hex','dest_before_hex','source_after_hex','dest_after_hex'}
FIELDS=HEX|{'selector','kind','source_index','dest_index','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards'}
def validate(value):
    if not isinstance(value,dict) or set(value)!={'schema','probe','math'} or value['schema']!=1 or value['probe']!='sane-mixed-semantics-v1':raise ValueError('Wrong SANE mixed arithmetic profile')
    math=value['math']
    if not isinstance(math,dict) or set(math)!={'records'}:raise ValueError('Missing mixed arithmetic records')
    rows=math['records'];operands=(ROOT/'probes/sane/operands.bin').read_bytes();doubles=(ROOT/'probes/sanewiden/operands.bin').read_bytes()
    if not isinstance(rows,list) or len(rows)!=5760:raise ValueError('Incomplete mixed arithmetic matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict) or set(r)!=FIELDS:raise ValueError('Missing mixed arithmetic field')
        if any(type(r[k]) is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Missing mixed arithmetic scalar')
        slot=i%2880;kind=slot//45
        destinations=[2,2,4,6,2,3,20,17,0,1,2,3,2,2,6,7,15,16,20,6,2,3,36,32,2,27,35,24,37,2,24,36]
        src=kind if kind<32 else 80+((kind-32)*3)%128;dst=destinations[kind]if kind<32 else 38+514+((kind-32)*3+37)%128
        if (r['selector'],r['kind'],r['source_index'],r['dest_index'],r['input_env'],r['input_ccr'])!=([2052,2054][i//2880],kind,src,dst,MODES[slot//3%15],[16,31,0][slot%3]):raise ValueError('Changed mixed arithmetic context')
        for k in HEX:
            if not isinstance(r[k],str) or re.fullmatch('[0-9a-f]{16}' if k.startswith('source') else '[0-9a-f]{20}',r[k]) is None:raise ValueError('Invalid packed operand')
        if r['source_before_hex']!=doubles[src*8:src*8+8].hex():raise ValueError('Changed mixed double operand')
        if r['dest_before_hex']!=operands[dst*10:dst*10+10].hex():raise ValueError('Changed mixed extended operand')
        if not isinstance(r['guards'],list) or len(r['guards'])!=4 or any(type(x) is not int for x in r['guards']):raise ValueError('Missing mixed arithmetic guards')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe'] not in ['sane-mixed-raw-v1','sane-mixed-runtime-raw-v1']:raise ValueError('Wrong raw mixed arithmetic capture')
    v={'schema':1,'probe':'sane-mixed-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
