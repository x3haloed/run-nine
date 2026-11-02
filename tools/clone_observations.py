"""Exact HandToHand register/state/payload snapshots, without normalization."""
import re
FIELDS={'kind','state','length','input_d0','input_ccr','d0','ccr','stack_delta','mem_error','output_nonnull','distinct_source','source_size','copy_size','source_state','copy_state','source_hex','copy_hex','guard_hex'}
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','memory'} or v['schema']!=1 or v['probe']!='hand-clone-v1':raise ValueError('Wrong clone profile')
    data=v['memory']
    if not isinstance(data,dict) or set(data)!={'records'} or not isinstance(data['records'],list) or len(data['records'])!=51:raise ValueError('Incomplete clone scenarios')
    for i,row in enumerate(data['records']):
        if not isinstance(row,dict) or set(row)!=FIELDS:raise ValueError('Missing clone field')
        kind=0 if i<3 else 1+(i-3)//12;state=0 if i<3 else [0,64,128,192][(i-3)%12//3];n=[0,0,1,17,512][kind]
        if any(type(row[k]) is not int for k in FIELDS-{'source_hex','copy_hex','guard_hex'}):raise ValueError('Missing clone scalar')
        if (row['kind'],row['state'],row['length'],row['input_d0'],row['input_ccr'])!=(kind,state,n,305419896,[16,31,0][i%3]):raise ValueError('Missing clone context')
        for key,size in [('source_hex',n if kind else 0),('copy_hex',n if row['output_nonnull'] else 0),('guard_hex',37)]:
            s=row[key]
            if not isinstance(s,str) or len(s)!=size*2 or re.fullmatch('[0-9a-f]*',s) is None:raise ValueError('Missing clone bytes')
def project(raw):
    if set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']!='hand-clone-raw-v1':raise ValueError('Wrong native clone input')
    v={'schema':1,'probe':'hand-clone-v1','memory':{'records':raw['records']}};validate(v);return v
