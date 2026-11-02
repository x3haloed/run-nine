"""Measured ColorTable observations; only the initial dynamic seed is normalized."""
import copy,re
IDS=[-1,0,1,2,3,4,8,16,32,33,34,36,40,64,128,129,256,1000,5000]
FIELDS={'id','input_ccr','present','d0','d0_is_handle','ccr','stack_delta','guard','resource_error','memory_error','state','size','home','home_error','repeat_same','repeat_bytes_equal','repeat_seed_delta','bytes','repeat_bytes'}
OWN={'input_ccr','entry_before','entry_zero_after','same_as_source','source_state','result_state','reload_same','result_bytes','reload_bytes'}
STRINGS={'bytes','repeat_bytes','result_bytes','reload_bytes'}
def records(v,raw):
    if not isinstance(v,dict)or set(v)!=({'schema','probe','records','ownership'}if raw else {'schema','probe','color'})or v.get('schema')!=1 or v.get('probe')!=('get-color-table-raw-v1'if raw else 'get-color-table-v1'):raise ValueError('Wrong ColorTable profile')
    c=v if raw else v['color']
    if not raw and(not isinstance(c,dict)or set(c)!={'records','ownership'}):raise ValueError('Missing color table observations')
    if not isinstance(c['records'],list)or len(c['records'])!=57 or not isinstance(c['ownership'],list)or len(c['ownership'])!=3:raise ValueError('Incomplete color table matrix')
    for name,fields in [('records',FIELDS),('ownership',OWN)]:
        for i,r in enumerate(c[name]):
            if not isinstance(r,dict)or set(r)!=fields:raise ValueError('Missing ColorTable field')
            if any(type(r[k])is not int for k in fields-STRINGS):raise ValueError('Invalid ColorTable scalar')
            if r['input_ccr']!=[16,31,0][i%3]or(name=='records'and r['id']!=IDS[i//3]):raise ValueError('Changed ColorTable context')
            for k in fields&STRINGS:
                if not isinstance(r[k],str)or re.fullmatch('(?:[0-9a-f]{2})*',r[k])is None:raise ValueError('Invalid ColorTable bytes')
                n=r.get('size',24)
                if len(r[k])!=2*n:raise ValueError('Truncated ColorTable payload')
    return {k:c[k] for k in ['records','ownership']}

def normalize(v):
    c=copy.deepcopy(records(v,True));base=None;ordinal=0
    for row,key in [(r,k)for r in c['records']if r['id']==128 for k in ['bytes','repeat_bytes']]+[(r,'result_bytes')for r in c['ownership']]:
        if len(row[key])<16:raise ValueError('Missing application table seed')
        seed=int(row[key][:8],16)
        if base is None:
            base=seed
            if not 0x400<=base<0xffffffff-8:raise ValueError('Invalid initial dynamic color-table seed')
        if seed!=base+ordinal:raise ValueError('Nonconsecutive dynamic color-table seeds')
        ordinal+=1
        row[key]=f'{ordinal:08x}'+row[key][8:]
    assert ordinal==9
    result={'schema':1,'probe':'get-color-table-v1','color':c};validate(result);return result

def validate(v):
    c=records(v,False)
    seeds=[int(r[k][:8],16)for r in c['records']if r['id']==128 for k in ['bytes','repeat_bytes']]+[int(r['result_bytes'][:8],16)for r in c['ownership']]
    if seeds!=list(range(1,10)):raise ValueError('Invalid relative ColorTable seed sequence')
