"""Exact CTabChanged bytes/ABI; one initial dynamic seed offset normalized."""
import copy,re
FIELDS={'which','input_ccr','size','before_state','base','before_hex','d0','ccr','stack_delta','after','after_state','memory_error','resource_error','after_hex'}
BASES=[1,5,9,12,15,18]
def records(v,raw):
    if not isinstance(v,dict)or set(v)!=({'schema','probe','records'}if raw else {'schema','probe','color'})or v.get('schema')!=1 or v.get('probe')!=('ctab-changed-raw-v1'if raw else 'ctab-changed-v1'):raise ValueError('Wrong CTabChanged profile')
    c={'records':v['records']}if raw else v['color']
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=6:raise ValueError('Incomplete notification matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS or any(type(r[k])is not int for k in FIELDS-{'before_hex','after_hex'}):raise ValueError('Missing notification field')
        if(r['which'],r['input_ccr'],r['size'])!=(i//3,[16,31,0][i%3],[24,2056][i//3]):raise ValueError('Changed notification context')
        for k in ['before_hex','after_hex']:
            if not isinstance(r[k],str)or re.fullmatch('[0-9a-f]{'+str(2*r['size'])+'}',r[k])is None:raise ValueError('Truncated color-table payload')
    return c

def normalize(v):
    c=copy.deepcopy(records(v,True));base=c['records'][0]['base']
    if not 0x400<=base<0xffffffff-24:raise ValueError('Invalid initial notification seed')
    for i,r in enumerate(c['records']):
        actual=base+BASES[i]-1
        if r['base']!=actual or int(r['after_hex'][:8],16)!=actual+1 or r['after']!=actual+2:raise ValueError('Broken notification/global counter relation')
        r['base']-=base-1;r['after']-=base-1;r['after_hex']=f'{BASES[i]+1:08x}'+r['after_hex'][8:]
    result={'schema':1,'probe':'ctab-changed-v1','color':c};validate(result);return result

def validate(v):
    c=records(v,False)
    for i,r in enumerate(c['records']):
        if r['base']!=BASES[i]or r['after']!=BASES[i]+2 or int(r['after_hex'][:8],16)!=BASES[i]+1:raise ValueError('Invalid relative notification sequence')
