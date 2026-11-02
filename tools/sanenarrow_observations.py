"""Exact signed16/32 conversion profile; finite |x|>=2^63 explicitly excluded.

Scope comes from the packed source's numeric exponent, independently of runtime
results. Raw captures retain excluded native cases for later implementation.
"""
import copy,re
from pathlib import Path
from sane_observations import MODES
ROOT=Path(__file__).resolve().parents[1]
HEX={'source_before_hex','source_after_hex','dest_before_hex','dest_after_hex'}
FIELDS=HEX|{'dataset','selector','kind','dest_width','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards'}
FILES=['probes/saneround/operands.bin','probes/saneintflags/operands.bin']
def supported(b):
    e=int.from_bytes(b[:2],'big')&32767;s=int.from_bytes(b[2:],'big')
    return e==32767 or s==0 or e-16383+s.bit_length()-64<63
def contexts():
    for dataset,f in enumerate(FILES):
        blob=(ROOT/f).read_bytes()
        for selector,width in [(8208,2),(10256,4)]:
            for kind in range(len(blob)//10):
                b=blob[kind*10:kind*10+10]
                if not supported(b):continue
                for mode in MODES:
                    for ccr in [16,31,0]:yield dataset,selector,kind,width,mode,ccr,b.hex()
def validate(value):
    if not isinstance(value,dict)or set(value)!={'schema','probe','math'}or value['schema']!=1 or value['probe']!='sane-narrow-semantics-v1':raise ValueError('Wrong signed integer conversion profile')
    if not isinstance(value['math'],dict)or set(value['math'])!={'records'}:raise ValueError('Missing integer records')
    rows=value['math']['records'];expected=list(contexts())
    if not isinstance(rows,list)or len(rows)!=len(expected)or len(expected)!=26010:raise ValueError('Incomplete integer matrix')
    for r,c in zip(rows,expected):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing integer field')
        if any(type(r[k])is not int for k in FIELDS-HEX-{'guards'}):raise ValueError('Invalid integer scalar')
        if tuple(r[k]for k in ['dataset','selector','kind','dest_width','input_env','input_ccr'])!=c[:6]:raise ValueError('Changed integer context')
        for key in HEX:
            length=20 if key.startswith('source')else c[3]*2
            if not isinstance(r[key],str)or re.fullmatch('[0-9a-f]{'+str(length)+'}',r[key])is None:raise ValueError('Invalid integer operand bytes')
        if r['source_before_hex']!=c[6]or r['dest_before_hex']!='a5'*c[3]:raise ValueError('Changed integer input')
        if not isinstance(r['guards'],list)or len(r['guards'])!=4 or any(type(x)is not int for x in r['guards']):raise ValueError('Invalid integer guards')
def native_project(raws):
    rows=[]
    for dataset,raw in enumerate(raws):
        name=['sane-round-raw-v1','sane-intflags-raw-v1'][dataset];count=[34020,14040][dataset]
        if set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']!=name or len(raw['records'])!=count:raise ValueError('Wrong native integer capture')
        for r in raw['records']:
            if r['selector']in [8208,10256]and supported(bytes.fromhex(r['source_before_hex'])):rows.append(dict(copy.deepcopy(r),dataset=dataset))
    v={'schema':1,'probe':'sane-narrow-semantics-v1','math':{'records':rows}};validate(v);return v
def runtime_project(raw):
    if set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']!='sane-narrow-runtime-raw-v1':raise ValueError('Wrong runtime integer capture')
    v={'schema':1,'probe':'sane-narrow-semantics-v1','math':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
