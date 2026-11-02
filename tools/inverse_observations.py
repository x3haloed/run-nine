"""Public inverse-grid/query/port semantics; retain opaque caches in raw captures."""
import copy,re
RAW={'kind','device','seed','input_ccr','resolution','d0','ccr','stack_delta','qd_error','itable_size','before_hex','after_hex','itable_hex','indices','exact_indices','port','current_same','list_same','guard_hex'}
PORT={'row_bytes','pixel_size','base_same','table_same','map_distinct','pen_indices','rgb_fg','rgb_bg','port_rect','bitmap_bounds','visible_hex','clip_hex'}
def hex_string(s,n):
    if not isinstance(s,str) or len(s)!=2*n or re.fullmatch('[0-9a-f]*',s) is None:raise ValueError('Incomplete inverse bytes')
def array(a,n):
    if not isinstance(a,list) or len(a)!=n or any(type(v) is not int for v in a):raise ValueError('Incomplete inverse array')
def rows(values,raw):
    if not isinstance(values,list) or len(values)!=96:raise ValueError('Incomplete inverse contexts')
    fields=RAW if raw else RAW-{'d0','ccr','itable_hex'}|{'public_hex'}
    for i,row in enumerate(values):
        if not isinstance(row,dict) or set(row)!=fields:raise ValueError('Missing inverse field')
        strings={k for k in fields if k.endswith('_hex')};dynamic=strings|{'indices','exact_indices','port'}
        if any(type(row[k]) is not int for k in fields-dynamic):raise ValueError('Missing inverse scalar')
        if (row['kind'],row['device'],row['seed'],row['input_ccr'],row['resolution'])!=(i//12,i//6%2,i//3%2,[16,31,0][i%3],3):raise ValueError('Missing inverse input context')
        n=[2,4,8,16,4,4,4,6][row['kind']]
        for key in ['before_hex','after_hex']:hex_string(row[key],8+8*n)
        if row['itable_size']!=1036:raise ValueError('Wrong inverse allocation extent')
        hex_string(row['itable_hex']if raw else row['public_hex'],1036 if raw else 518);hex_string(row['guard_hex'],37);array(row['indices'],512);array(row['exact_indices'],n)
        port=row['port']
        if not isinstance(port,dict) or set(port)!=PORT:raise ValueError('Missing offscreen port state')
        for key in ['row_bytes','pixel_size','base_same','table_same','map_distinct']:
            if type(port[key]) is not int:raise ValueError('Missing offscreen port scalar')
        for key,n in [('pen_indices',2),('rgb_fg',3),('rgb_bg',3),('port_rect',4),('bitmap_bounds',4)]:array(port[key],n)
        for key in ['visible_hex','clip_hex']:hex_string(port[key],10)
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','color'} or v['schema']!=1 or v['probe']!='inverse-semantics-v1':raise ValueError('Wrong inverse profile')
    if not isinstance(v['color'],dict) or set(v['color'])!={'records'}:raise ValueError('Missing inverse section')
    rows(v['color']['records'],False)
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe'] not in ['inverse-raw-v3','inverse-runtime-raw-v1']:raise ValueError('Wrong inverse capture')
    rows(raw['records'],True);result=copy.deepcopy(raw['records'])
    for r in result:
        # Void MakeITable's D0/CCR are caller-scratch, not return values.
        # First518 bytes are the published seed/resolution/512-byte RGB grid.
        # Remaining allocation is private cache/padding (including unstable
        # native bytes). Both domains remain visible in the retained raw JSON.
        del r['d0'];del r['ccr'];r['public_hex']=r.pop('itable_hex')[:1036]
    v={'schema':1,'probe':'inverse-semantics-v1','color':{'records':result}};validate(v);return v
