"""Native QuickDraw Random result, state and ABI, without normalization."""
import copy
FIELDS=set('initial_seed input_d0 input_ccr step seed_before seed_after result d0 ccr stack_delta guard mem_error res_error'.split())
SEEDS=[0,1,2,0x12345678,0x7ffffffe,0x7fffffff,0x80000000,0xffffffff,0xffff0000,0xffff,0x00ff00ff,0x55555555,0xaaaaaaaa]
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','random'} or v['schema']!=1 or v['probe']!='qd-random-v1':raise ValueError('Wrong Random profile')
    c=v['random']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=1248:raise ValueError('Incomplete Random matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS or any(type(x)is not int for x in r.values()):raise ValueError('Missing Random field')
        if tuple(r[k]for k in ['initial_seed','input_d0','input_ccr','step'])!=(SEEDS[i//96],[0x12345678,0][i//48%2],[16,31,0][i//16%3],i%16):raise ValueError('Changed Random context')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']not in ['qd-random-raw-v1','qd-random-runtime-raw-v1']:raise ValueError('Wrong raw Random capture')
    v={'schema':1,'probe':'qd-random-v1','random':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
