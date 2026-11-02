"""Exact synchronous Device Manager missing-unit observations."""
import re
FIELDS={'trap','reference','code','input_d0','input_ccr','unit_index','in_range','entry_empty','d0','ccr','stack_delta','a0_preserved','before_guard','after_guard','pb_hex'}
def records(v,raw):
    if not isinstance(v,dict)or set(v)!=({'schema','probe','unit_count','records'}if raw else {'schema','probe','drivers'})or v.get('schema')!=1 or v.get('probe')!=('empty-driver-raw-v1'if raw else 'empty-driver-v1'):raise ValueError('Wrong Device Manager profile')
    c={k:v[k]for k in ['unit_count','records']}if raw else v['drivers']
    if not isinstance(c,dict)or set(c)!={'unit_count','records'}or type(c['unit_count'])is not int or c['unit_count']!=96 or not isinstance(c['records'],list)or len(c['records'])!=192:raise ValueError('Incomplete unit table/profile')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS or any(type(r[k])is not int for k in FIELDS-{'pb_hex'}):raise ValueError('Missing Device Manager field')
        if(r['trap'],r['reference'],r['code'],r['input_d0'],r['input_ccr'])!=([0xa004,0xa204,0xa005,0xa205][i//48],[-4,-32768,0,1][i%48//12],[2,-1][i%12//6],[0x12345678,0][i%6//3],[16,31,0][i%3]):raise ValueError('Changed Device Manager context')
        if not isinstance(r['pb_hex'],str)or re.fullmatch('[0-9a-f]{100}',r['pb_hex'])is None:raise ValueError('Invalid/truncated parameter block')
    return c

def project(v):
    c=records(v,True);return {'schema':1,'probe':'empty-driver-v1','drivers':c}
def validate(v):records(v,False)
