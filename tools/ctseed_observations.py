"""Shared color IDs: only the initial dynamic counter offset is normalized."""
import copy
DYNAMIC=['base','result','application_seed','after_builtin','device_after','after_entries']
FIELDS={'input_d0','input_ccr','base','result','d0','ccr','stack_delta','guard','application_seed','builtin_seed','after_builtin','device_before','device_after','after_entries','device_rgb'}
def records(v,raw):
    if not isinstance(v,dict)or set(v)!=({'schema','probe','records'}if raw else {'schema','probe','color'})or v.get('schema')!=1 or v.get('probe')!=('color-seed-raw-v1'if raw else 'color-seed-v1'):raise ValueError('Wrong color-seed profile')
    c={'records':v['records']}if raw else v['color']
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=6:raise ValueError('Incomplete shared color-seed matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS or any(type(r[k])is not int for k in FIELDS-{'device_rgb'}):raise ValueError('Missing color-seed scalar')
        if(r['input_d0'],r['input_ccr'])!=([0x12345678,0][i//3],[16,31,0][i%3]):raise ValueError('Changed color-seed context')
        if not isinstance(r['device_rgb'],list)or len(r['device_rgb'])!=3 or any(type(x)is not int for x in r['device_rgb']):raise ValueError('Incomplete palette edit')
    return c

def normalize(v):
    c=copy.deepcopy(records(v,True));rows=c['records'];base=rows[0]['base']
    if not 0x400<=base<0xffffffff-36:raise ValueError('Invalid dynamic color counter start')
    previous=None
    for i,r in enumerate(rows):
        if [r[k]for k in DYNAMIC]!=list(range(base+6*i,base+6*i+6)):raise ValueError('Broken shared seed sequence')
        if r['device_before']!=(8 if i==0 else previous):raise ValueError('Broken device-seed link')
        previous=r['device_after']
        for k in DYNAMIC:r[k]-=base-1
        if i:r['device_before']-=base-1
    result={'schema':1,'probe':'color-seed-v1','color':c};validate(result);return result

def validate(v):
    c=records(v,False)
    for i,r in enumerate(c['records']):
        if[r[k]for k in DYNAMIC]!=list(range(1+6*i,7+6*i))or r['device_before']!=(8 if i==0 else 6*i-1):raise ValueError('Invalid relative shared seed sequence')
