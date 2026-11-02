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
        if (row['kind'],row['device'],row['seed'],row['input_ccr'],row['resolution'])!=(i//12,i//6%2,i//3%2,[16,31,0][i%3],4):raise ValueError('Missing inverse input context')
        n=[2,4,8,16,4,4,4,6][row['kind']]
        for key in ['before_hex','after_hex']:hex_string(row[key],8+8*n)
        if row['itable_size']!=4620:raise ValueError('Wrong inverse allocation extent')
        hex_string(row['itable_hex']if raw else row['public_hex'],4620 if raw else 4102);hex_string(row['guard_hex'],37);array(row['indices'],4096);array(row['exact_indices'],n)
        port=row['port']
        if not isinstance(port,dict) or set(port)!=PORT:raise ValueError('Missing offscreen port state')
        for key in ['row_bytes','pixel_size','base_same','table_same','map_distinct']:
            if type(port[key]) is not int:raise ValueError('Missing offscreen port scalar')
        for key,n in [('pen_indices',2),('rgb_fg',3),('rgb_bg',3),('port_rect',4),('bitmap_bounds',4)]:array(port[key],n)
        for key in ['visible_hex','clip_hex']:hex_string(port[key],10)
def palette(text):
    import struct
    hex_string(text,2056)
    b=bytes.fromhex(text);seed,flags,last=struct.unpack_from('>IHH',b)
    if (seed,flags,last)!=(8,32768,255):raise ValueError('Changed main display palette')
    return {'seed':seed,'flags':flags,'count':last+1,'rgb_hex':b''.join(b[10+i*8:16+i*8]for i in range(256)).hex()}
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','color'} or v['schema']!=1 or v['probe']!='inverse-four-semantics-v1':raise ValueError('Wrong four-bit inverse profile')
    c=v['color']
    if not isinstance(c,dict) or set(c)!={'records','main_palette','main_indices'}:raise ValueError('Missing four-bit inverse section')
    rows(c['records'],False);array(c['main_indices'],4096)
    p=c['main_palette']
    if not isinstance(p,dict)or set(p)!={'seed','flags','count','rgb_hex'}:raise ValueError('Incomplete main palette')
    if any(type(p[k]) is not int for k in ['seed','flags','count'])or(p['seed'],p['flags'],p['count'])!=(8,32768,256):raise ValueError('Changed main palette metadata')
    hex_string(p['rgb_hex'],1536)
def project(raw):
    if not isinstance(raw,dict) or raw.get('schema')!=1:raise ValueError('Wrong four-bit capture')
    native=raw.get('probe')=='inverse-four-raw-v1'
    expected={'schema','probe','records','main_table_hex','main_inverse_hex'if native else'main_indices'}
    if set(raw)!=expected or raw['probe']not in['inverse-four-raw-v1','inverse-four-runtime-raw-v1']:raise ValueError('Incomplete four-bit capture')
    rows(raw['records'],True);records=copy.deepcopy(raw['records'])
    for r in records:
        del r['d0'];del r['ccr'];r['public_hex']=r.pop('itable_hex')[:8204]
    if native:
        hex_string(raw['main_inverse_hex'],4620);b=bytes.fromhex(raw['main_inverse_hex'])
        if b[:6]!=bytes.fromhex('000000080004'):raise ValueError('Changed main inverse header')
        indices=list(b[6:4102])
    else:indices=copy.deepcopy(raw['main_indices'])
    v={'schema':1,'probe':'inverse-four-semantics-v1','color':{'records':records,'main_palette':palette(raw['main_table_hex']),'main_indices':indices}}
    validate(v);return v
