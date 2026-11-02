"""Exact SANE tangent state/single results and an explicit one-ULP extended tangent bound."""
import copy,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
MODES=[0,0x20,0x40,0x2000,0x2020,0x2040,0x4000,0x4020,0x4040,0x6000,0x6020,0x6040,0x0100,0x0800,0x1f00]
CONTROL_MODES=[0,0x20,0x40,0x2000,0x4000,0x6000,0x0100,0x1000,0x1f00,0x1f,0xffff]
CONTROL={'input_env','input_ccr','selector','word','d0','ccr','stack_delta'}
ROW={'kind','input_env','input_ccr','d0','ccr','stack_delta','output_env','guards','single_bits','single_guards','single_env','before_hex','after_hex'}
def scalar(row,fields):
    if not isinstance(row,dict) or set(row)!=fields:raise ValueError('Missing SANE field')
def records(data):
    operands=(ROOT/'probes/sanesine/operands.bin').read_bytes();base=62;dense=2178
    if type(data['initial_env']) is not int:raise ValueError('Missing initial SANE environment')
    if not isinstance(data['controls'],list) or len(data['controls'])!=66:raise ValueError('Incomplete SANE controls')
    for i,r in enumerate(data['controls']):
        scalar(r,CONTROL)
        if any(type(v) is not int for v in r.values()):raise ValueError('Missing control scalar')
        if (r['input_env'],r['input_ccr'],r['selector'])!=(CONTROL_MODES[i//6],[16,31,0][i//2%3],[1,3][i%2]):raise ValueError('Missing control context')
    for section,count in [('tan',base*45),('dense',dense)]:
        values=data[section]
        if not isinstance(values,list) or len(values)!=count:raise ValueError('Incomplete SANE math scenarios')
        for i,r in enumerate(values):
            scalar(r,ROW);kind=i//45 if section=='tan' else i
            if (r['kind'],r['input_env'],r['input_ccr'])!=(kind,MODES[i//3%15] if section=='tan' else 0,[16,31,0][i%3] if section=='tan' else 16):raise ValueError('Missing SANE input context')
            for key in ROW-{'before_hex','after_hex','guards','single_guards'}:
                if type(r[key]) is not int:raise ValueError('Missing SANE scalar')
            for key in ['before_hex','after_hex']:
                if not isinstance(r[key],str) or re.fullmatch('[0-9a-f]{20}',r[key]) is None:raise ValueError('Missing packed extended value')
            offset=(kind if section=='tan' else base+kind)*10
            if r['before_hex']!=operands[offset:offset+10].hex():raise ValueError('Changed owned SANE operand')
            for key in ['guards','single_guards']:
                if not isinstance(r[key],list) or len(r[key])!=2 or any(type(x) is not int for x in r[key]):raise ValueError('Missing SANE guard')
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','math'} or v['schema']!=1 or v['probe']!='sane-tan-semantics-v1':raise ValueError('Wrong SANE profile')
    if not isinstance(v['math'],dict) or set(v['math'])!={'initial_env','controls','tan','dense'}:raise ValueError('Missing SANE sections')
    records(v['math'])
def project(raw):
    if set(raw)!={'schema','probe','initial_env','controls','tan','dense'} or raw['schema']!=1 or raw['probe'] not in ['sane-tan-raw-v1','sane-tan-runtime-raw-v1']:raise ValueError('Wrong SANE raw capture')
    value={'schema':1,'probe':'sane-tan-semantics-v1','math':{k:copy.deepcopy(raw[k])for k in ['initial_env','controls','tan','dense']}};validate(value);return value
def ordinary(row):
    source=int(row['before_hex'],16);value=int(row['after_hex'],16);exponent=value>>64&0x7fff;mantissa=value&((1<<64)-1)
    return source>>64&0x7fff!=0x7fff and exponent not in [0,0x7fff] and mantissa>=1<<63 and exponent-16383>=-32
def rank(text):
    value=int(text,16);e=value>>64&0x7fff;m=value&((1<<64)-1)
    if e==0x7fff or m<1<<63:return None
    return (e<<63)+(m-(1<<63))
def distance(a,b):
    if (int(a[:4],16)^int(b[:4],16))&0x8000:return None
    x=rank(a);y=rank(b)
    return None if x is None or y is None else abs(x-y)
def eligible(a,b):return a['before_hex']==b['before_hex'] and ordinary(a) and distance(a['after_hex'],b['after_hex']) is not None
def bounded_differences(a,b):
    from compare import _exact_differences
    validate(a);validate(b);candidate=copy.deepcopy(b)
    for section in ['tan','dense']:
        for reference,result in zip(a['math'][section],candidate['math'][section]):
            if eligible(reference,result) and distance(reference['after_hex'],result['after_hex'])<=1:result['after_hex']=reference['after_hex']
    return _exact_differences(a,candidate)
def statistics(a,b):
    distances=[];different=0;single_exact=0;total=0
    for section in ['tan','dense']:
        for n,r in zip(a['math'][section],b['math'][section]):
            total+=1;single_exact+=n['single_bits']==r['single_bits']
            if eligible(n,r):
                d=distance(n['after_hex'],r['after_hex']);distances.append(d);different+=d!=0
    return {'extended_tan_tolerance_ulp':1,'extended_max_observed_ulp':max(distances,default=0),'extended_nonidentical_rows':different,'single_precision_bit_exact_rows':single_exact,'math_rows':total,'scope':'Only ordinary finite tangent results may differ by one64-bit-significand ULP. Specials/tiny values, D0/CCR/stack, environments, single results and guards remain exact; raw extended results are retained.'}
