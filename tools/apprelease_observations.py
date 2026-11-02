"""Exact primary application resource release/detach observations."""
import re
STRINGS={'retained_payload','reloaded_payload'}
FIELDS={'case','input_ccr','initial_error','before_state','d0','ccr','stack_delta','error','after_state','home','home_error','entry_before','entry_zero_after_call','entry_matches_reload','reload_error','reload_home_same','current_same','reload_resource_bit'}|STRINGS
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','resources'}or v['schema']!=1 or v['probe']!='app-resource-release-v1':raise ValueError('Wrong application resource release profile')
    if not isinstance(v['resources'],dict)or set(v['resources'])!={'records'}:raise ValueError('Missing release records')
    rows=v['resources']['records']
    if not isinstance(rows,list)or len(rows)!=12:raise ValueError('Incomplete release matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing release field')
        if any(type(r[k])is not int for k in FIELDS-STRINGS):raise ValueError('Invalid release scalar')
        if(r['case'],r['input_ccr'])!=(i//3,[16,31,0][i%3]):raise ValueError('Changed release input context')
        for key in STRINGS:
            length=0 if key=='retained_payload'and i//3 in [0,1]else 12
            if not isinstance(r[key],str)or re.fullmatch('[0-9a-f]{'+str(length)+'}',r[key])is None:raise ValueError('Invalid release bytes')
