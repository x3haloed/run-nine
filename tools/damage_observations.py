"""Exact causally seeded window damage/visibility/event observations."""
import re
SCALARS={'trap','origin_case','shape','input_ccr','d0','d0_is_arg','d0_is_window','d0_is_update_handle','d0_is_update_data','ccr','stack_delta','current_same'}
ARRAYS={'input_rect','port_rect','bitmap_bounds','after_input_rect','after_port_rect','after_bitmap_bounds','event1','event2'}
REGIONS={'input_region','before_update','before_vis','before_clip','after_update','after_vis','after_clip','after_input_region','after_events_update','during_update','during_vis','during_clip','final_update','final_vis','final_clip'}
FIELDS=SCALARS|ARRAYS|REGIONS
def records(v,raw):
    if not isinstance(v,dict)or set(v)!=({'schema','probe','records'}if raw else {'schema','probe','windows'})or v.get('schema')!=1 or v.get('probe')!=('window-damage-raw-v2'if raw else 'window-damage-v2'):raise ValueError('Wrong window damage profile')
    c={'records':v['records']}if raw else v['windows']
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=96:raise ValueError('Incomplete window damage matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS or any(type(r[k])is not int for k in SCALARS):raise ValueError('Missing window damage field')
        if(r['trap'],r['origin_case'],r['shape'],r['input_ccr'])!=([0xa92a,0xa929,0xa928,0xa927][i//24],i%24//12,i%12//3,[16,31,0][i%3]):raise ValueError('Changed window damage context')
        for k in ARRAYS:
            if not isinstance(r[k],list)or len(r[k])!=4 or any(type(x)is not int for x in r[k]):raise ValueError('Invalid rectangle/event observation')
        for k in REGIONS:
            s=r[k]
            if not isinstance(s,str)or re.fullmatch('(?:[0-9a-f]{4}){5,2048}',s)is None or int(s[:4],16)*2!=len(s):raise ValueError('Invalid/truncated native region')
    return c

def project(v):return {'schema':1,'probe':'window-damage-v2','windows':records(v,True)}
def validate(v):records(v,False)
