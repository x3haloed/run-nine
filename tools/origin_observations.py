"""Exact public-state coordinate observations; no address or timing projection."""
FIELDS={'kind','step','h','v','input_ccr','d0','ccr','stack_delta','current_same','pen','local_global','global_local','port_rect','bitmap','visible','clip'}
def validate(value):
    if not isinstance(value,dict) or set(value)!={'schema','probe','ports'} or value['schema']!=1 or value['probe']!='origin-v1':raise ValueError('Wrong origin profile')
    ports=value['ports']
    if not isinstance(ports,dict) or set(ports)!={'cases'} or not isinstance(ports['cases'],list) or len(ports['cases'])!=16:raise ValueError('Incomplete origin contexts')
    for i,row in enumerate(ports['cases']):
        if not isinstance(row,dict) or set(row)!=FIELDS:raise ValueError('Missing origin field')
        h,v=[(-17,23),(31,-29),(31,-29),(0,0)][i%4]
        if (row['kind'],row['step'],row['h'],row['v'],row['input_ccr'])!=(i//4,i%4,h,v,[16,31,0][i%4%3]):raise ValueError('Missing origin input context')
        arrays={'pen':2,'local_global':2,'global_local':2,'port_rect':4,'bitmap':4,'visible':22,'clip':5}
        for key,n in arrays.items():
            if not isinstance(row[key],list) or len(row[key])!=n or any(type(x) is not int for x in row[key]):raise ValueError('Incomplete origin '+key)
        if any(type(row[k]) is not int for k in FIELDS-set(arrays)):raise ValueError('Noninteger origin state')
def project(raw):
    if set(raw)!={'schema','probe','cases'} or raw['schema']!=1 or raw['probe']!='origin-raw-v2':raise ValueError('Wrong native origin capture')
    v={'schema':1,'probe':'origin-v1','ports':{'cases':raw['cases']}};validate(v);return v
