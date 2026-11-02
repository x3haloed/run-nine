"""Public driverless device state. Only allocated pointer identities are roles."""
import copy,re
RECORD={'refnum','mode','input_ccr','d0','ccr','stack_delta','new_nonnull','distinct_main','distinct_previous','current_same','list_same','gd_size','gd_hex','pm_size','pm_hex','itable_size','itable_seed','itable_res','table_size','ccdata_size','ccmask_size','table_hex','ccdata_hex','ccmask_hex','children_distinct'}
TABLE={'index','input_ccr','d0','ccr','stack_delta','replacement_distinct','current_same','list_same'}
SWITCH={'target','input_ccr','d0','ccr','stack_delta','selected_same','list_same'}
DISPOSE={'index','input_ccr','d0','ccr','stack_delta','current_same','list_same'}
def rows(data):
    for section,n,fields in [('records',12,RECORD),('tables',3,TABLE),('switches',9,SWITCH),('disposals',12,DISPOSE)]:
        values=data[section]
        if not isinstance(values,list) or len(values)!=n:raise ValueError('Incomplete device '+section)
        for i,row in enumerate(values):
            if not isinstance(row,dict) or set(row)!=fields:raise ValueError('Missing offscreen '+section+' field')
            strings={k for k in fields if k.endswith('_hex')}
            if any(type(row[k]) is not int for k in fields-strings):raise ValueError('Missing device scalar')
            if row['input_ccr']!=[16,31,0][i%3]:raise ValueError('Missing device CCR context')
            if section=='records':
                if(row['refnum'],row['mode'])!=([0,-1,42,-51][i//3],-1):raise ValueError('Missing driverless input')
                sizes={'gd_hex':62,'pm_hex':50,'table_hex':row['table_size'],'ccdata_hex':row['ccdata_size'],'ccmask_hex':row['ccmask_size']}
                for key,nbytes in sizes.items():
                    value=row[key]
                    if nbytes<0 or nbytes>4096 or not isinstance(value,str) or len(value)!=2*nbytes or re.fullmatch('[0-9a-f]*',value) is None:raise ValueError('Missing public record bytes')
            elif section=='switches':
                if row['target']!=i//3:raise ValueError('Missing switch context')
            elif row['index']!=i:raise ValueError('Missing lifecycle context')
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','devices'} or v['schema']!=1 or v['probe']!='off-device-v1':raise ValueError('Wrong driverless profile')
    data=v['devices']
    if not isinstance(data,dict) or set(data)!={'records','tables','switches','disposals'}:raise ValueError('Missing lifecycle sections')
    rows(data)
def project(raw):
    if set(raw)!={'schema','probe','records','tables','switches','disposals'} or raw['schema']!=1 or raw['probe'] not in ['off-device-raw-v3','off-device-runtime-raw-v1']:raise ValueError('Wrong driverless capture')
    data={k:copy.deepcopy(raw[k]) for k in ['records','tables','switches','disposals']};rows(data)
    for r in data['records']:
        for key,offsets in [('gd_hex',[6,22,50,54]),('pm_hex',[42])]:
            b=bytearray.fromhex(r[key])
            for offset in offsets:b[offset:offset+4]=int(any(b[offset:offset+4])).to_bytes(4,'big')
            r[key]=b.hex()
    v={'schema':1,'probe':'off-device-v1','devices':data};validate(v);return v
