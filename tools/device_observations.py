"""Exact single-display selection, with opaque handles expressed as roles."""
FIELDS={'kind','input_ccr','rect','d0','ccr','stack_delta','selected_role','main_same','null','current_same'}
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','devices'} or v['schema']!=1 or v['probe']!='max-device-v1':raise ValueError('Wrong device profile')
    d=v['devices']
    if not isinstance(d,dict) or set(d)!={'screen','main_nonnull','cases'} or type(d['main_nonnull']) is not bool:raise ValueError('Missing device context')
    if not isinstance(d['screen'],list) or len(d['screen'])!=4 or any(type(x) is not int for x in d['screen']):raise ValueError('Missing device bounds')
    if not isinstance(d['cases'],list) or len(d['cases'])!=36:raise ValueError('Incomplete device scenarios')
    for i,row in enumerate(d['cases']):
        if not isinstance(row,dict) or set(row)!=FIELDS or (row['kind'],row['input_ccr'])!=(i//3,[16,31,0][i%3]):raise ValueError('Missing selection input context')
        if not isinstance(row['rect'],list) or len(row['rect'])!=4 or any(type(x) is not int for x in row['rect']):raise ValueError('Missing selection rectangle')
        if any(type(row[k]) is not int for k in FIELDS-{'rect'}):raise ValueError('Missing device state')
def project(raw):
    if set(raw)!={'schema','probe','main_handle','screen','cases'} or raw['schema']!=1 or raw['probe']!='max-device-raw-v1' or type(raw['main_handle']) is not int:raise ValueError('Wrong native display capture')
    rows=[]
    for row in raw['cases']:
        if not isinstance(row,dict) or set(row)!=(FIELDS-{'selected_role'})|{'selected'} or type(row['selected']) is not int:raise ValueError('Missing native handle identity')
        r=row.copy();selected=r.pop('selected');r['selected_role']=0 if selected==0 else 1 if selected==raw['main_handle'] else 2;rows.append(r)
    v={'schema':1,'probe':'max-device-v1','devices':{'screen':raw['screen'],'main_nonnull':raw['main_handle']!=0,'cases':rows}};validate(v);return v
