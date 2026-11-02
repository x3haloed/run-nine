"""Strict window-color API observations, with no selected-field normalization."""
SCALARS={'id','step','found','aux_nonnull','owner_same','aux_flags','aux_refcon','aux_reserved_nonnull','table_source_same','table_size','table_flags','current_unchanged','visible','variant'}
ARRAYS={'bg':3,'fg':3,'port_rect':4,'bitmap':4,'structure':4,'content':4,'visible_region':4}
def ints(v,n):
    if not isinstance(v,list) or len(v)!=n or any(type(x) is not int for x in v):raise ValueError('Incomplete window color array')
def validate(value):
    if set(value)!={'schema','probe','windows'} or value['schema']!=1 or value['probe']!='window-colors-v1' or set(value['windows'])!={'cases'}:raise ValueError('Wrong window color profile')
    rows=value['windows']['cases']
    if not isinstance(rows,list) or len(rows)!=12:raise ValueError('Incomplete window color contexts')
    for i,row in enumerate(rows):
        if not isinstance(row,dict) or row.get('id')!=22000+i//4 or row.get('step')!=i%4:raise ValueError('Missing window color context')
        if i%4==3:
            if set(row)!={'id','step','erased_rgb'}:raise ValueError('Incomplete erased content')
            ints(row['erased_rgb'],3);continue
        if set(row)!=SCALARS|set(ARRAYS)|{'table'} or any(type(row[k]) is not int for k in SCALARS):raise ValueError('Missing auxiliary window state')
        for k,n in ARRAYS.items():ints(row[k],n)
        expected=13 if i==0 else 5
        if not isinstance(row['table'],list) or len(row['table'])!=expected:raise ValueError('Missing window table entries')
        for colors in row['table']:ints(colors,4)
def project(raw):
    if set(raw)!={'schema','probe','cases','restore_error'} or raw['schema']!=1 or raw['probe']!='window-colors-raw-v3' or raw['restore_error']!=0:raise ValueError('Wrong native window color capture/failed restoration')
    result={'schema':1,'probe':'window-colors-v1','windows':{'cases':raw['cases']}};validate(result);return result
