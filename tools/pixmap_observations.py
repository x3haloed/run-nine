"""Public PixMap lifecycle observations, normalizing only actual pointer fields."""
import copy,re
ABI={'d0','ccr','stack_delta','guard','memory_error'}
SNAP={'size','state','base_zero','base_is_device','table_present','table_is_device','table_size','table_state','map_hex','table_hex'}
FIELDS={'depth','input_ccr','new_abi','initial','copy_abi','copied','distinct_maps','distinct_tables','destination_table_reused','copied_base_same','copy_table_after_source_mutation','dispose_abi','base_size_after_dispose','free_balanced','base_bytes_after_dispose'}
STRINGS={'copy_table_after_source_mutation','base_bytes_after_dispose'}
POINTER_BYTES=set(range(4))|set(range(42,46))
def hex_value(s,n):
    if not isinstance(s,str)or re.fullmatch('[0-9a-f]{'+str(n*2)+'}',s)is None:raise ValueError('Missing/invalid PixMap bytes')
def abi(v):
    if not isinstance(v,dict)or set(v)!=ABI or any(type(x)is not int for x in v.values()):raise ValueError('Missing PixMap ABI field')
def records(v,raw):
    if not isinstance(v,dict)or set(v)!=({'schema','probe','records','nil_dispose'}if raw else {'schema','probe','pixmaps'})or v.get('schema')!=1 or v.get('probe')!=('pixmap-raw-v1'if raw else 'pixmap-v1'):raise ValueError('Wrong PixMap profile')
    c={k:v[k]for k in ['records','nil_dispose']}if raw else v['pixmaps']
    if not isinstance(c,dict)or set(c)!={'records','nil_dispose'}or not isinstance(c['records'],list)or len(c['records'])!=6 or not isinstance(c['nil_dispose'],list)or len(c['nil_dispose'])!=3:raise ValueError('Incomplete PixMap matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing PixMap observation')
        scalar=FIELDS-STRINGS-{'new_abi','copy_abi','dispose_abi','initial','copied'}
        if any(type(r[k])is not int for k in scalar)or(r['depth'],r['input_ccr'])!=([8,32][i//3],[16,31,0][i%3]):raise ValueError('Changed PixMap context')
        for key in ['new_abi','copy_abi','dispose_abi']:abi(r[key])
        for key in ['initial','copied']:
            s=r[key]
            if not isinstance(s,dict)or set(s)!=SNAP or any(type(s[k])is not int for k in SNAP-{'map_hex','table_hex'}):raise ValueError('Missing public PixMap field')
            if not 0<=s['table_size']<=4096:raise ValueError('Invalid table size')
            hex_value(s['map_hex'],50);hex_value(s['table_hex'],s['table_size'])
        hex_value(r['copy_table_after_source_mutation'],r['copied']['table_size']);hex_value(r['base_bytes_after_dispose'],64)
    for i,r in enumerate(c['nil_dispose']):
        if not isinstance(r,dict)or set(r)!={'input_ccr','abi'}or type(r['input_ccr'])is not int or r['input_ccr']!=[16,31,0][i]:raise ValueError('Missing nil disposal context')
        abi(r['abi'])
    return c

def normalize(v):
    c=copy.deepcopy(records(v,True));device_base=None
    for r in c['records']:
        pointers=[]
        for j,key in enumerate(['initial','copied']):
            s=r[key];b=bytearray.fromhex(s['map_hex']);base=int.from_bytes(b[:4],'big');table=int.from_bytes(b[42:46],'big')
            if not base or not table or base==table or s['base_zero']!=0 or s['base_is_device']!=1-j or s['table_present']!=1 or s['table_is_device']!=0:raise ValueError('Invalid PixMap pointer role/presence')
            if j==0:
                if device_base is None:device_base=base
                if base!=device_base:raise ValueError('Changed screen-base alias within fixture')
            elif base==device_base:raise ValueError('Caller pixel storage aliases screen')
            pointers.append((base,table));b[:4]=(j+1).to_bytes(4,'big');b[42:46]=(j+1).to_bytes(4,'big');s['map_hex']=b.hex()
        if pointers[0][1]==pointers[1][1]or any(base==table for base,_ in pointers for _,table in pointers):raise ValueError('Aliased PixMap storage')
    result={'schema':1,'probe':'pixmap-v1','pixmaps':c};validate(result);return result

def validate(v):
    c=records(v,False)
    for r in c['records']:
        for j,k in enumerate(['initial','copied']):
            b=bytes.fromhex(r[k]['map_hex'])
            if int.from_bytes(b[:4],'big')!=j+1 or int.from_bytes(b[42:46],'big')!=j+1:raise ValueError('Invalid canonical PixMap pointer role')
