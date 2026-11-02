"""Bit-exact measured FXPWRY paths for integer exponents0,+1,-1,+2,-2."""
import copy,re
from pathlib import Path
from sane_observations import MODES
from sanepower_observations import FIELDS,HEX,project as full_project
ROOT=Path(__file__).resolve().parents[1]
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','math'}or v['schema']!=1 or v['probe']!='sane-small-integer-power-semantics-v1':raise ValueError('Wrong integer power profile')
    if not isinstance(v['math'],dict)or set(v['math'])!={'records'}:raise ValueError('Missing integer power records')
    rows=v['math']['records'];operands=(ROOT/'probes/sanepower/operands.bin').read_bytes()
    if not isinstance(rows,list)or len(rows)!=4950:raise ValueError('Incomplete integer power matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing integer power field')
        if any(type(r[k])is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Missing integer power scalar')
        pair=i//45;kind=pair//5*10+pair%5
        if(r['kind'],r['input_env'],r['input_ccr'])!=(kind,MODES[i//3%15],[16,31,0][i%3]):raise ValueError('Changed integer power context')
        for k in HEX:
            if not isinstance(r[k],str)or re.fullmatch('[0-9a-f]{20}',r[k])is None:raise ValueError('Invalid packed power operand')
        if r['source_before_hex']!=operands[kind*20:kind*20+10].hex()or r['dest_before_hex']!=operands[kind*20+10:kind*20+20].hex():raise ValueError('Changed integer power operand')
        if not isinstance(r['guards'],list)or len(r['guards'])!=4 or any(type(x)is not int for x in r['guards']):raise ValueError('Missing integer power guards')
def native_project(raw):
    full=full_project(raw)
    v={'schema':1,'probe':'sane-small-integer-power-semantics-v1','math':{'records':[r for r in full['math']['records']if r['kind']<220 and r['kind']%10<5]}}
    validate(v);return v
def runtime_project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']!='sane-small-integer-power-runtime-raw-v1':raise ValueError('Wrong raw integer power capture')
    v={'schema':1,'probe':'sane-small-integer-power-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}}
    validate(v);return v
