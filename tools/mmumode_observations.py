"""Native32-bit-only SwapMMUMode legal-request ABI, without normalization."""
import copy
FIELDS=set('requested input_d0 input_ccr mode_before mode_after mode_restored d0 ccr stack_delta a0_preserved guard mem_error res_error'.split())
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','addressing'} or v['schema']!=1 or v['probe']!='mmu-mode-v1':raise ValueError('Wrong MMU mode profile')
    c=v['addressing']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=18:raise ValueError('Incomplete MMU matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS or any(type(x)is not int for x in r.values()):raise ValueError('Missing MMU field')
        mode=[1,0][i//9]
        if tuple(r[k]for k in ['requested','input_d0','input_ccr'])!=(mode,[0,0x12345600,0xffff0000][i//3%3]|mode,[16,31,0][i%3]):raise ValueError('Changed MMU context')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']not in ['mmu-mode-raw-v1','mmu-mode-runtime-raw-v1']:raise ValueError('Wrong raw MMU capture')
    v={'schema':1,'probe':'mmu-mode-v1','addressing':{'records':copy.deepcopy(raw['records'])}};validate(v);return v
