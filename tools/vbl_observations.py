"""Strict retrace observations. Hardware clock/epoch domains stay explicit."""
import copy
REC={'system','slot','step','input_d0','input_ccr','d0','ccr','a0_same','stack_delta','raw_task','raw_link','type','count','phase','proc_same','guards'}
COMMON={'calls','self_same','callback_type','callback_count','callback_phase','after_type','after_count','after_phase','guards'}
AUTO=COMMON|{'system','slot','kind','install_d0','install_result','install_ccr','remove_d0','remove_result','remove_ccr','ticks'}
MANUAL=COMMON|{'kind','install_d0','install_ccr','remove_d0','remove_ccr','ticks','pulse_samples','pulse_d0','pulse_ccr'}
def array(v,n):
    if not isinstance(v,list) or len(v)!=n or any(type(x) is not int for x in v):raise ValueError('Incomplete VBL array')
def fields(row,keys,arrays):
    if not isinstance(row,dict) or set(row)!=keys:raise ValueError('Missing VBL observation')
    for k,n in arrays.items():array(row[k],n)
    if any(type(row[k]) is not int for k in keys-set(arrays)):raise ValueError('Noninteger VBL field')
def context(row,i,section,slot):
    if section=='records':
        values=list(range(17))+[-1,-2];value=values[i//12];system=int(value==-2);upper=0x12340000 if i//6%2 else 0
        if (row['system'],row['slot'],row['step'],row['input_d0'],row['input_ccr'])!=(system,value,i%2,upper|(0x3456 if system else value&65535),[16,31,0][i//2%3]):raise ValueError('Missing VBL register context')
    elif section=='callbacks':
        if (row['system'],row['slot'],row['kind'])!=(int(i//6==0),slot if i//6==2 else 0,i%6):raise ValueError('Missing VBL automatic context')
    elif row['kind']!=i:raise ValueError('Missing VBL explicit pulse context')
def tick_bounds(ticks,calls,manual):
    n=min(max(calls,0),3);used=ticks[1:1+n]
    return {'wait_valid':ticks[4]>=ticks[0] if manual else ticks[4]-ticks[0]>=30,'within_wait':all((ticks[0]<=t if manual else ticks[0]<t) and t<=ticks[4] for t in used),'ordered':all(a<=b if manual else a<b for a,b in zip(used,used[1:])),'unused_zero':all(t==0 for t in ticks[1+n:4])}
def project(raw):
    if raw.get('schema')!=1:raise ValueError('Wrong VBL schema')
    if raw.get('probe')=='vbl-raw-v3':
        if set(raw)!={'schema','probe','main_refnum','main_slot','records','callbacks','manual'} or type(raw['main_refnum']) is not int:raise ValueError('Incomplete native VBL hardware context')
        slot=raw['main_slot']
    elif raw.get('probe')=='vbl-runtime-raw-v1':
        if set(raw)!={'schema','probe','configured_slot','records','callbacks','manual'}:raise ValueError('Incomplete owned VBL configuration')
        slot=raw['configured_slot']
    else:raise ValueError('Wrong VBL raw identifier')
    if type(slot) is not int:raise ValueError('Missing configured VBL driver slot')
    result={'schema':1,'probe':'vbl-semantics-v1','retrace':{'slot_under_test':slot}}
    for section,n,keys,arrays in [('records',228,REC,{'guards':2}),('callbacks',18,AUTO,{'guards':2,'ticks':5}),('manual',6,MANUAL,{'guards':2,'ticks':5,'pulse_samples':3,'pulse_d0':12,'pulse_ccr':12})]:
        rows=raw[section]
        if not isinstance(rows,list) or len(rows)!=n:raise ValueError('Incomplete VBL scenarios')
        output=[]
        for i,row in enumerate(rows):
            fields(row,keys,arrays);context(row,i,section,slot);r=copy.deepcopy(row)
            if section=='records':
                p=r.pop('raw_task');link=r.pop('raw_link');r['task_aligned']=int(p>=0x100 and p%2==0);r['owned_successor']=int(link==p)
                if r['system']:r['count']={'within_adjacent_call_bound':31998<=r['count']<=32000}
            else:
                ticks=r.pop('ticks');r['tick_bounds']=tick_bounds(ticks,r['calls'],section=='manual')
                if section=='callbacks' and r['system'] and r['kind'] in [3,5]:
                    initial=0 if r['kind']==3 else -2
                    r['after_count']={'negative_counter_progress':1<=initial-r['after_count']<=ticks[4]-ticks[0]}
            output.append(r)
        result['retrace'][section]=output
    validate(result);return result

def validate(value):
    if set(value)!={'schema','probe','retrace'} or value['schema']!=1 or value['probe']!='vbl-semantics-v1':raise ValueError('Wrong retrace profile')
    data=value['retrace']
    if not isinstance(data,dict) or set(data)!={'slot_under_test','records','callbacks','manual'} or type(data['slot_under_test']) is not int:raise ValueError('Incomplete retrace sections')
    for section,n,base,arrays in [('records',228,REC-{'raw_task','raw_link'}|{'task_aligned','owned_successor'},{'guards':2}),('callbacks',18,AUTO-{'ticks'}|{'tick_bounds'},{'guards':2}),('manual',6,MANUAL-{'ticks'}|{'tick_bounds'},{'guards':2,'pulse_samples':3,'pulse_d0':12,'pulse_ccr':12})]:
        rows=data[section]
        if not isinstance(rows,list) or len(rows)!=n:raise ValueError('Incomplete retrace cases')
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=base:raise ValueError('Missing retrace field')
            context(row,i,section,data['slot_under_test'])
            dynamic=set()
            if section=='records' and row['system']:dynamic.add('count');key='within_adjacent_call_bound';obj=row['count']
            elif section=='callbacks' and row['system'] and row['kind'] in [3,5]:dynamic.add('after_count');key='negative_counter_progress';obj=row['after_count']
            else:key=None
            if key and (not isinstance(obj,dict) or set(obj)!={key} or type(obj[key]) is not bool):raise ValueError('Missing VBL elapsed-counter bound')
            if section!='records':
                dynamic.add('tick_bounds');obj=row['tick_bounds']
                if not isinstance(obj,dict) or set(obj)!={'wait_valid','within_wait','ordered','unused_zero'} or any(type(v) is not bool for v in obj.values()):raise ValueError('Missing VBL tick observations')
            for k,size in arrays.items():array(row[k],size)
            if any(type(row[k]) is not int for k in base-set(arrays)-dynamic):raise ValueError('Missing VBL scalar')
