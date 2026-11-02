"""Relocation semantics independent of allocator-specific payload addresses."""
INTS={'mode','input_ccr','result','ccr','a0_same','stack_delta','mem_error','before_ptr','after_ptr','other_before','other_after','state_before','state_after','sum','other_sum','guard_ok','barrier_ptr','barrier_ok'}
HEX={'payload_hex','other_hex','guard_hex','barrier_hex'}
def validate_rows(rows,projected):
    if not isinstance(rows,list) or len(rows)!=27:raise ValueError('Incomplete relocation cases')
    fields=(INTS-{'before_ptr','after_ptr','other_before','other_after','barrier_ptr'}|{'movement','barrier_present','other_live'}) if projected else INTS
    for i,row in enumerate(rows):
        if not isinstance(row,dict) or set(row)!=fields|HEX:raise ValueError('Missing relocation observation')
        if any(type(row[k]) is not int for k in fields-{'other_live'}):raise ValueError('Noninteger relocation field')
        if row['mode']!=i//3 or row['input_ccr']!=[16,31,0][i%3]:raise ValueError('Missing relocation context')
        if projected:
            if not isinstance(row['other_live'],list) or len(row['other_live'])!=2 or any(type(v) is not int for v in row['other_live']):raise ValueError('Missing other payload presence')
        sizes={'payload_hex':0 if row['mode'] in [2,3] else 32,'other_hex':0 if row['mode']==8 else 48,'guard_hex':16,'barrier_hex':32 if row['mode'] in [6,8] else 0}
        for k,n in sizes.items():
            if not isinstance(row[k],str) or len(row[k])!=n*2 or len(bytes.fromhex(row[k]))!=n:raise ValueError('Missing relocation bytes')
def project(raw):
    if set(raw)!={'schema','probe','cases'} or raw['schema']!=1 or raw['probe']!='move-high-raw-v4':raise ValueError('Wrong relocation raw profile')
    validate_rows(raw['cases'],False);rows=[]
    for row in raw['cases']:
        r={k:v for k,v in row.items() if k not in ['before_ptr','after_ptr','other_before','other_after','barrier_ptr']}
        r['movement']=(row['after_ptr']>row['before_ptr'])-(row['after_ptr']<row['before_ptr'])
        r['other_live']=[int(row[k]!=0) for k in ['other_before','other_after']]
        r['barrier_present']=int(row['barrier_ptr']!=0);rows.append(r)
    return {'schema':1,'probe':'move-high-semantics-v1','memory':{'cases':rows}}
def validate(value):
    if set(value)!={'schema','probe','memory'} or value['schema']!=1 or value['probe']!='move-high-semantics-v1' or set(value['memory'])!={'cases'}:raise ValueError('Wrong relocation profile')
    validate_rows(value['memory']['cases'],True)
