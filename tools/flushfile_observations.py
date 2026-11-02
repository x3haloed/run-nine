"""PBFlushFile public effects; retain unprojected native PB and failed query values."""
import copy
INTS=set('kind volume_input input_d0 input_ccr d0 ccr stack_delta guard a0_preserved mem_error res_error mark_before_error mark_after_error eof_before_error eof_after_error'.split())
QUERIES=['mark_before','mark_after','eof_before','eof_after']
FIELDS=INTS|set(QUERIES)|{'before_hex','after_hex'}
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','files'} or v['schema']!=1 or v['probe']!='flush-file-v1':raise ValueError('Wrong flush profile')
    c=v['files']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=108:raise ValueError('Incomplete flush matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS or any(type(r[k])is not int for k in INTS):raise ValueError('Missing flush field')
        if tuple(r[k]for k in ['kind','volume_input','input_d0','input_ccr'])!=(i//12,i//6%2,[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed flush context')
        for k in QUERIES:
            if (r[k]is not None if r[k+'_error'] else type(r[k])is not int):raise ValueError('Wrong defined query scope')
        for k in ['before_hex','after_hex']:
            if not isinstance(r[k],str) or len(r[k])!=(192 if k=='before_hex' else 184):raise ValueError('Missing public PB bytes')
            try:bytes.fromhex(r[k])
            except ValueError:raise ValueError('Invalid PB bytes')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']not in ['flush-file-raw-v1','flush-file-runtime-raw-v1']:raise ValueError('Wrong raw flush capture')
    rows=[]
    for source in raw['records']:
        if not isinstance(source,dict) or set(source)!=FIELDS|{'reference'}:raise ValueError('Missing raw flush field')
        r=copy.deepcopy(source);ref=r.pop('reference');kind=r['kind']
        if type(ref)is not int or (kind>=6 and ref!=[0,-1,12345][kind-6]):raise ValueError('Changed reference input')
        for k in ['before_hex','after_hex']:
            if not isinstance(r[k],str) or len(r[k])!=192:raise ValueError('Missing raw PB bytes')
            data=bytearray.fromhex(r[k])
            if int.from_bytes(data[24:26],'big')!=ref&0xffff:raise ValueError('Reference not preserved in PB')
            if kind<6:data[24:26]=(0x7000+kind).to_bytes(2,'big')
            # ioCmdAddr is a private ROM routine pointer, not a portable result.
            if k=='after_hex':del data[8:12]
            r[k]=data.hex()
        for k in QUERIES:
            if type(r[k])is not int:raise ValueError('Missing raw query value')
            if r[k+'_error']:r[k]=None
        rows.append(r)
    v={'schema':1,'probe':'flush-file-v1','files':{'records':rows}};validate(v);return v
