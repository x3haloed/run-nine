"""Complete native all-map and current-map resource-count ABI observations."""
import copy
FIELDS={'phase','op','type','input_d0','input_ccr','result','d0','ccr','stack_delta','guard','mem_error','res_error','current_preserved'}
TYPES=[int.from_bytes(t,'big')for t in[b'pCnt',b'pNil']]
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','resources'}or v['schema']!=1 or v['probe']!='count-resources-v1':raise ValueError('Wrong count profile')
    c=v['resources']
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=216:raise ValueError('Incomplete count matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS or any(type(x)is not int for x in r.values()):raise ValueError('Missing count field')
        if tuple(r[k]for k in ['phase','op','type','input_d0','input_ccr'])!=(i//24,i//12%2,TYPES[i//6%2],[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed count context')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in['count-resources-raw-v1','count-resources-runtime-raw-v1']:raise ValueError('Wrong raw count capture')
    v={'schema':1,'probe':'count-resources-v1','resources':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
