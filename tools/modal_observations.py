"""ModalDialog callback/result contract; native ResError side effects remain unmodeled."""
import copy
FIELDS=set('mode input_d0 input_ccr d0 ccr stack_delta guard mem_error port_preserved item prefix suffix calls'.split())
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','dialogs'}or v['schema']!=1 or v['probe']!='modal-dialog-v1':raise ValueError('Wrong modal profile')
    c=v['dialogs']
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=48:raise ValueError('Incomplete modal matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing modal observation')
        if tuple(r[k]for k in ['mode','input_d0','input_ccr'])!=(i//6,[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed modal context')
        for k,x in r.items():
            if k=='calls':
                n=2 if i//6==2 else 4 if i//6==6 else 1
                if not isinstance(x,list)or len(x)!=n or any(not isinstance(row,list)or len(row)!=8 or any(type(y)is not int for y in row)for row in x):raise ValueError('Missing callback observation')
            elif type(x)is not int:raise ValueError('Invalid modal integer')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in ['modal-dialog-raw-v1','modal-dialog-runtime-raw-v1']:raise ValueError('Wrong raw modal capture')
    rows=copy.deepcopy(raw['records'])
    if not isinstance(rows,list):raise ValueError('Missing raw modal records')
    for r in rows:
        if not isinstance(r,dict)or set(r)!=FIELDS|{'res_error'}or type(r['res_error'])is not int or not -32768<=r['res_error']<=32767:raise ValueError('Missing raw modal observation')
        r.pop('res_error')
    v={'schema':1,'probe':'modal-dialog-v1','dialogs':{'records':rows}};validate(v);return v
