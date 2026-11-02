"""Native physical GetKeys bitmap and ABI; ROM scratch D0 remains raw only."""
import copy
INTS=set('phase input_d0 input_ccr ccr stack_delta guard prefix suffix mem_error res_error'.split())
HEX={'before_hex','after_hex','keys_hex'}
FIELDS=INTS|HEX
MAPS=['0'*32,'0'*30+'40','0020'+'0'*28,'0'*12+'02'+'0'*18,'0'*14+'01'+'0'*14+'40','0'*32]
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','events'} or v['schema']!=1 or v['probe']!='get-keys-v1':raise ValueError('Wrong GetKeys profile')
    c=v['events']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=36:raise ValueError('Incomplete GetKeys matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS or any(type(r[k])is not int for k in INTS):raise ValueError('Missing GetKeys field')
        if tuple(r[k]for k in ['phase','input_d0','input_ccr'])!=(i//6,[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed GetKeys context')
        for k in HEX:
            if not isinstance(r[k],str) or len(r[k])!=32:raise ValueError('Missing keyboard bytes')
            try:bytes.fromhex(r[k])
            except ValueError:raise ValueError('Invalid keyboard bytes')
        if r['before_hex']!=MAPS[i//6]:raise ValueError('Changed physical input')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']not in ['get-keys-raw-v1','get-keys-runtime-raw-v1']:raise ValueError('Wrong raw GetKeys capture')
    rows=[]
    for source in raw['records']:
        if not isinstance(source,dict) or set(source)!=FIELDS|{'d0'} or type(source['d0'])is not int:raise ValueError('Missing raw GetKeys field')
        r=copy.deepcopy(source);del r['d0'];rows.append(r)
    v={'schema':1,'probe':'get-keys-v1','events':{'records':rows}};validate(v);return v
