"""Explicit, strict native 8-bit software-device comparison profile."""
STATE={'flags','type','depth','pixel_type','components','component_size','row_bytes','bounds','device_bounds','table_size','table_flags'}
ABI={'selector','depth','input_ccr','result','ccr','stack_delta','guard'}
INDEXED={'available','default_palette','custom_entries','exact_indices','paints','set_pixel','copy_pixels','copy_source','neutral_copy_pixels','rop_modes','recolored'}
def array(v,n):
    if not isinstance(v,list) or len(v)!=n:raise ValueError('Incomplete color array')
def ints(v):
    if any(type(n) is not int for n in v):raise ValueError('Noninteger color observation')
def state(v,projected=False):
    keys=STATE-{'flags'}|{'flag_roles'} if projected else STATE
    if not isinstance(v,dict) or set(v)!=keys:raise ValueError('Incomplete device state')
    ints([v[k] for k in keys-{'bounds','device_bounds'}])
    for k in ['bounds','device_bounds']:array(v[k],4);ints(v[k])
def validate(value):
    if set(value)!={'schema','probe','color'} or value['schema']!=1 or value['probe']!='indexed-color-v1':raise ValueError('Wrong color profile')
    c=value['color']
    if not isinstance(c,dict) or set(c)!={'abi','device8','indexed'}:raise ValueError('Incomplete color sections')
    state(c['device8'],True);array(c['abi'],12)
    for i,row in enumerate(c['abi']):
        if not isinstance(row,dict) or set(row)!=ABI:raise ValueError('Missing depth ABI field')
        ints(row.values())
        if (row['selector'],row['depth'],row['input_ccr'])!=([0x0a14,0x0a13][i//6],[8,3][i//3%2],[16,31,0][i%3]):raise ValueError('Missing depth ABI context')
    x=c['indexed']
    if not isinstance(x,dict) or set(x)!=INDEXED:raise ValueError('Missing indexed observation')
    if type(x['available']) is not int:raise ValueError('Missing color capability')
    for key,length in [('default_palette',256),('custom_entries',4),('paints',3)]:
        array(x[key],length)
        for row in x[key]:array(row,8 if key=='paints' else 3);ints(row)
    for key,length in [('exact_indices',4),('copy_pixels',4),('copy_source',6),('neutral_copy_pixels',4),('rop_modes',16)]:array(x[key],length);ints(x[key])
    for key in ['set_pixel','recolored']:
        if not isinstance(x[key],dict) or set(x[key])!={'index','rgb'} or type(x[key]['index']) is not int:raise ValueError('Incomplete pixel readback')
        array(x[key]['rgb'],3);ints(x[key]['rgb'])
def project(raw):
    if set(raw)!={'schema','probe','main_is_first','next_null','current_is_main','initial','queries','abi','changes','indexed','restore_error','restored'} or raw['schema']!=1 or raw['probe']!='device-depth-raw-v4':raise ValueError('Wrong native depth capture')
    state(raw['initial']);state(raw['restored']);array(raw['changes'],6);array(raw['queries'],21)
    for i,row in enumerate(raw['changes']):
        if set(row)!={'request','by_mode','error','state'} or row['request']!=[8,16,32][i//2] or row['by_mode']!=i%2:raise ValueError('Missing native depth change')
        state(row['state'])
    for i,row in enumerate(raw['queries']):
        if set(row)!={'depth','which','flags','mode'} or (row['depth'],row['which'],row['flags'])!=([1,2,4,8,16,32,3][i//3],[0,1,1][i%3],[0,0,1][i%3]):raise ValueError('Missing native depth query')
    # Select exactly the first successful indexed-depth state. Native direct
    # devices and driver-only flag bits stay raw, outside this capability profile.
    if raw['changes'][0]['error']!=0:raise ValueError('Native 8-bit switch failed')
    s=raw['changes'][0]['state'].copy();s['flag_roles']=s.pop('flags')&0xa821
    value={'schema':1,'probe':'indexed-color-v1','color':{'abi':raw['abi'],'device8':s,'indexed':raw['indexed']}}
    validate(value);return value
