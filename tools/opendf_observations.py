"""Exact datafork open comparison. Only successful allocated IDs are normalized."""
import copy
SCALARS={'case','hierarchical','input_ccr','d0','ccr','stack_delta','a0_preserved','io_result','ref_result','ref_valid_or_preserved','volume_preserved','dir_preserved','name_preserved','close_error'}
FIELDS=SCALARS|{'read','guards'}
def validate(value):
    if not isinstance(value,dict)or set(value)!={'schema','probe','files'}or value['schema']!=1 or value['probe']!='open-df-semantics-v2':raise ValueError('Wrong datafork open profile')
    if not isinstance(value['files'],dict)or set(value['files'])!={'records'}:raise ValueError('Missing OpenDF records')
    rows=value['files']['records']
    if not isinstance(rows,list)or len(rows)!=72:raise ValueError('Incomplete OpenDF matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing OpenDF field')
        if any(type(r[k])is not int for k in SCALARS):raise ValueError('Invalid OpenDF scalar')
        if (r['case'],r['hierarchical'],r['input_ccr'])!=(i//6,i//3%2,[16,31,0][i%3]):raise ValueError('Changed OpenDF input context')
        for key,length in [('read',6),('guards',2)]:
            if not isinstance(r[key],list)or len(r[key])!=length or any(type(x)is not int for x in r[key]):raise ValueError('Missing OpenDF list')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','raw_environment','files'}or raw['schema']!=1 or raw['probe']not in ['open-df-raw-v2','open-df-runtime-raw-v2']:raise ValueError('Wrong raw OpenDF capture')
    if not isinstance(raw['raw_environment'],dict)or set(raw['raw_environment'])!={'volume','base','child','other'}or any(type(x)is not int for x in raw['raw_environment'].values()):raise ValueError('Missing OpenDF raw environment')
    value={'schema':1,'probe':'open-df-semantics-v2','files':copy.deepcopy(raw['files'])}
    for row in value['files']['records']:
        reference=row.pop('raw_ref');assert type(reference)is int
        row['ref_result']=int(reference>0)if row['io_result']==0 else reference
    validate(value);return value
