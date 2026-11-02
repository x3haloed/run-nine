"""OS queue ABI with independently sampled clock origin removed."""
import copy
INTS=set('queue mask op input_d0 input_ccr d0 ccr stack_delta a0_preserved guard prefix suffix mem_error res_error tick_span'.split())
FIELDS=INTS|{'post_errors','queue_before','queue_after','physical_hex','event_hex','null_tick_delta'}
RAW=(FIELDS-{'tick_span','null_tick_delta'})|{'ticks_before','ticks_after'}
MASKS=[0,65535,8,6,256];LENGTHS=[0,1,1,1,2,2,2,3]
def hex_value(v,n):
    if not isinstance(v,str) or len(v)!=2*n:raise ValueError('Missing raw bytes')
    try:bytes.fromhex(v)
    except ValueError:raise ValueError('Invalid hex bytes')
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','events'} or v['schema']!=1 or v['probe']!='os-events-v1':raise ValueError('Wrong OS event profile')
    c=v['events']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=480:raise ValueError('Incomplete OS event matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS or any(type(r[k])is not int for k in INTS):raise ValueError('Missing OS event field')
        mask=MASKS[i//12%5]
        if tuple(r[k]for k in ['queue','mask','op','input_d0','input_ccr'])!=(i//60,mask,i//6%2,[0,0x12340000][i//3%2]|mask,[16,31,0][i%3]):raise ValueError('Changed OS event context')
        if not isinstance(r['post_errors'],list) or len(r['post_errors'])!=LENGTHS[i//60] or any(type(x)is not int for x in r['post_errors']):raise ValueError('Missing post results')
        for k in ['queue_before','queue_after']:
            if not isinstance(r[k],list):raise ValueError('Missing queue')
            for x in r[k]:hex_value(x,16)
        if len(r['queue_before'])!=LENGTHS[i//60]:raise ValueError('Changed queue input')
        hex_value(r['physical_hex'],30);hex_value(r['event_hex'],16)
        if r['event_hex'][:4]=='0000':
            if type(r['null_tick_delta'])is not int:raise ValueError('Missing null-event clock delta')
        elif r['null_tick_delta']is not None:raise ValueError('Queued timestamp must remain absolute')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']not in ['os-events-raw-v2','os-events-runtime-raw-v2']:raise ValueError('Wrong raw OS event capture')
    rows=[]
    for source in raw['records']:
        if not isinstance(source,dict) or set(source)!=RAW:raise ValueError('Missing raw OS event field')
        r=copy.deepcopy(source);before=r.pop('ticks_before');after=r.pop('ticks_after')
        if type(before)is not int or type(after)is not int:raise ValueError('Missing sampled ticks')
        r['tick_span']=(after-before)&0xffffffff
        hex_value(r['event_hex'],16);event=bytearray.fromhex(r['event_hex']);r['null_tick_delta']=None
        if event[:2]==b'\0\0':
            r['null_tick_delta']=(int.from_bytes(event[6:10],'big')-before)&0xffffffff
            event[6:10]=b'\0'*4;r['event_hex']=event.hex()
        rows.append(r)
    v={'schema':1,'probe':'os-events-v1','events':{'records':rows}};validate(v);return v
