"""PenPat public state, ABI and monochrome raster, without normalization."""
import copy
FIELDS=set('kind pattern input_d0 input_ccr d0 ccr stack_delta guard prefix suffix mem_error res_error source_hex before_hex after_hex pixels_hex'.split())
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','pens'} or v['schema']!=1 or v['probe']!='pen-pattern-v1':raise ValueError('Wrong pen profile')
    c=v['pens']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=48:raise ValueError('Incomplete pen matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS:raise ValueError('Missing pen field')
        if tuple(r[k] for k in ['kind','pattern','input_d0','input_ccr'])!=(i//24,i//6%4,[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed pen context')
        for k,value in r.items():
            if k.endswith('_hex'):
                n={'source_hex':8,'before_hex':18,'after_hex':18,'pixels_hex':128 if i<24 else 0}[k]
                if not isinstance(value,str) or len(value)!=n*2 or any(c not in '0123456789abcdef' for c in value):raise ValueError('Bad pen bytes')
            elif type(value)is not int:raise ValueError('Bad pen integer')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']not in ['pen-pattern-raw-v1','pen-pattern-runtime-raw-v1']:raise ValueError('Wrong raw pen capture')
    v={'schema':1,'probe':'pen-pattern-v1','pens':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
