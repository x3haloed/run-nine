"""Indexed resource-chain ABI. Only allocation-specific handle addresses project to nonnull."""
import copy
FIELDS=set('phase op type index input_d0 input_ccr nonnull d0 ccr stack_delta guard mem_error res_error current_preserved home id info_type same_handle size name_hex data_hex'.split())
RAW=(FIELDS-{'nonnull'})|{'result_handle'}
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','resources'}or v['schema']!=1 or v['probe']!='ind-resource-v1':raise ValueError('Wrong indexed resource profile')
    c=v['resources']
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=1188:raise ValueError('Incomplete indexed resource matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing indexed resource field')
        which=i//6%11
        expected=(i//132,i//66%2,int.from_bytes(b'pNil' if which==10 else b'pCnt','big'),1 if which==10 else which-1,[0x12345678,0][i//3%2],[16,31,0][i%3])
        if tuple(r[k]for k in ['phase','op','type','index','input_d0','input_ccr'])!=expected:raise ValueError('Changed index context')
        for k,x in r.items():
            if k.endswith('_hex'):
                if not isinstance(x,str)or len(x)%2 or any(c not in '0123456789abcdef'for c in x):raise ValueError('Invalid indexed resource bytes')
                if k=='data_hex' and len(x)!=r['size']*2:raise ValueError('Wrong indexed resource size')
            elif type(x)is not int:raise ValueError('Invalid indexed resource integer')
        if r['nonnull']not in (0,1):raise ValueError('Invalid handle identity')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in ['ind-resource-raw-v1','ind-resource-runtime-raw-v1']:raise ValueError('Wrong raw indexed capture')
    rows=copy.deepcopy(raw['records'])
    if not isinstance(rows,list):raise ValueError('Missing raw indexed records')
    for r in rows:
        if not isinstance(r,dict)or set(r)!=RAW or type(r['result_handle'])is not int or not 0<=r['result_handle']<=0xffffffff:raise ValueError('Missing raw indexed field')
        r['nonnull']=int(r.pop('result_handle')!=0)
    v={'schema':1,'probe':'ind-resource-v1','resources':{'records':rows}};validate(v);return v
