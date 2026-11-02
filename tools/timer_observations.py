"""Strict timer observation projection; raw addresses/clock samples stay in raw files.

Only epoch/elapsed-time dependent fields are projected. No expected-result file
is imported by the runtime or by this projection.
"""
import copy
RECORD={'mode','step','input_ccr','d0','ccr','a0_same','raw_task','raw_link','type','count','wake','reserved','proc_same','guards'}
CALLBACK={'mode','kind','calls','self_same','entry_a0_proc','callback_type','callback_count','after_type','after_count','raw_wake','raw_reserved','ticks','guards'}
EDGE={'mode','request','prime_type','prime_count','remaining','after_type','guards'}
def rows(value,fields,length):
    if not isinstance(value,list) or len(value)!=length:raise ValueError('Incomplete timer cases')
    for row in value:
        if not isinstance(row,dict) or set(row)!=fields:raise ValueError('Missing timer observation')
        for k,v in row.items():
            if k in ('guards','ticks'):
                if not isinstance(v,list) or len(v)!=(2 if k=='guards' else 5) or any(type(n) is not int for n in v):raise ValueError('Incomplete timer array')
            elif type(v) is not int:raise ValueError('Noninteger timer observation')
def remaining(value,request):
    duration=-request if request<0 else request*1000
    units='us' if value<=0 else 'ms'
    # Allow only the elapsed time of the measured immediately adjacent traps,
    # with one coarse millisecond of quantization. This is not latency fidelity.
    left=-value if value<=0 else value*1000
    return {'units':units,'within_request':0<=left<=duration,'adjacent_call_bound':abs(duration-left)<=100000}
def project(raw):
    if set(raw)!={'schema','probe','records','callbacks','edges'} or raw['schema']!=1 or raw['probe']!='time-manager-raw-v2':raise ValueError('Wrong timer raw profile')
    rows(raw['records'],RECORD,18);rows(raw['callbacks'],CALLBACK,6);rows(raw['edges'],EDGE,8)
    result=copy.deepcopy(raw);result['probe']='time-manager-semantics-v1'
    for i,row in enumerate(result['records']):
        if (row['mode'],row['input_ccr'],row['step'])!=(i//9,[16,31,0][(i//3)%3],i%3):raise ValueError('Missing timer register context')
        p=row.pop('raw_task');link=row.pop('raw_link')
        # One owned task at a time. Native OS internal queue tails are outside
        # this domain. A self-link is never an acceptable external tail.
        row['owned_successor']=1 if link==p else 0
        row['task_aligned']=int(p>=0x100 and p%2==0)
        if row['step']==2:row['count']=remaining(row['count'],10000)
        if row['mode'] and row['step']:
            wake=row.pop('wake');prime=raw['records'][i-i%3+1]['wake']
            row['wake_role']={'nonzero':int(wake!=0),'same_as_prime':int(wake==prime)}
    for i,row in enumerate(result['callbacks']):
        if (row['mode'],row['kind'])!=(i//3,i%3):raise ValueError('Missing timer callback context')
        ticks=row.pop('ticks');count=3 if row['kind']==2 else 1
        samples=ticks[1:1+count]
        row['tick_bounds']={'wait_30_ticks':ticks[4]-ticks[0]>=30,'within_wait':all(ticks[0]<t<=ticks[4] for t in samples),'ordered':all(b>a for a,b in zip(samples,samples[1:])),'unused_zero':all(t==0 for t in ticks[1+count:4])}
        if row['mode']:row['raw_wake']=int(row['raw_wake']!=0)
    for i,row in enumerate(result['edges']):
        if (row['mode'],row['request'])!=(i//4,[2147483647,86400000,-2147483647,2147483][i%4]):raise ValueError('Missing timer limit context')
        row['remaining']=remaining(row['remaining'],row['request'])
    return {'schema':1,'probe':result['probe'],'timers':{k:result[k] for k in ['records','callbacks','edges']}}
def validate(value):
    """Reject missing projected fields without treating observed values as truth."""
    if set(value)!={'schema','probe','timers'} or set(value['timers'])!={'records','callbacks','edges'}:raise ValueError('Incomplete timer profile')
    # Project a synthetic raw shape to get exact per-context mandatory fields.
    raw={'schema':1,'probe':'time-manager-raw-v2','records':[],'callbacks':[],'edges':[]}
    for i in range(18):raw['records'].append(dict.fromkeys(RECORD,0));raw['records'][-1].update(mode=i//9,input_ccr=[16,31,0][i//3%3],step=i%3,guards=[0,0])
    for i in range(6):raw['callbacks'].append(dict.fromkeys(CALLBACK,0));raw['callbacks'][-1].update(mode=i//3,kind=i%3,guards=[0,0],ticks=[0]*5)
    for i in range(8):raw['edges'].append(dict.fromkeys(EDGE,0));raw['edges'][-1].update(mode=i//4,request=[2147483647,86400000,-2147483647,2147483][i%4],guards=[0,0])
    expected=project(raw)
    def shape(a,b):
        if type(a) is not type(b):raise ValueError('Timer field type mismatch')
        if isinstance(a,dict):
            if set(a)!=set(b):raise ValueError('Missing timer field')
            for k in a:shape(a[k],b[k])
        elif isinstance(a,list):
            if len(a)!=len(b):raise ValueError('Incomplete timer cases')
            for x,y in zip(a,b):shape(x,y)
    shape(expected,value)
    for section,keys in [('records',['mode','step','input_ccr']),('callbacks',['mode','kind']),('edges',['mode','request'])]:
        for a,b in zip(expected['timers'][section],value['timers'][section]):
            if any(a[k]!=b[k] for k in keys):raise ValueError('Missing timer context')
