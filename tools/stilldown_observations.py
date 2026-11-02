"""Mouse tracking return/queue ABI; retain native ROM scratch and housekeeping raw."""
import copy
INPUTS=[[],[3],[2],[1],[3,2],[2,3],[1,3],[1,2,3]]
RAW={'phase','op','queue','input_d0','input_ccr','button_before','button_after','result_word','d0','ccr','stack_delta','guard','mem_error','res_error','post_errors','remaining_types'}
OMITTED={'d0','mem_error','res_error'}
def rows(records,raw):
    if not isinstance(records,list)or len(records)!=192:raise ValueError('Incomplete mouse tracking matrix')
    fields=RAW if raw else RAW-OMITTED
    for i,r in enumerate(records):
        if not isinstance(r,dict)or set(r)!=fields:raise ValueError('Missing mouse tracking field')
        if any(type(r[k])is not int for k in fields-{'post_errors','remaining_types'}):raise ValueError('Invalid tracking scalar')
        if tuple(r[k]for k in ['phase','op','queue','input_d0','input_ccr'])!=(i//96,i//48%2,i//6%8,[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed mouse tracking context')
        if(r['button_before'],r['button_after'])!=(i//96,i//96):raise ValueError('Physical button state changed during sample')
        errors=r['post_errors']
        if not isinstance(errors,list)or len(errors)!=len(INPUTS[r['queue']])or any(type(x)is not int or x!=0 for x in errors):raise ValueError('Invalid owned queue input')
        rest=r['remaining_types']
        if not isinstance(rest,list)or len(rest)>20 or any(type(x)is not int or not 0<=x<16 for x in rest):raise ValueError('Invalid remaining queue snapshot')
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','events'}or v['schema']!=1 or v['probe']!='still-down-v1':raise ValueError('Wrong mouse tracking profile')
    if not isinstance(v['events'],dict)or set(v['events'])!={'records'}:raise ValueError('Missing tracking records')
    rows(v['events']['records'],False)
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in['still-down-raw-v1','still-down-runtime-raw-v1']:raise ValueError('Wrong raw tracking capture')
    rows(raw['records'],True);records=copy.deepcopy(raw['records'])
    for r in records:
        # Pascal results reside on the stack. D0 contains ROM/environment
        # scratch; initial held calls may run housekeeping that clobbers
        # unrelated MemErr/ResErr. Retain those fields in both raw captures.
        for k in OMITTED:del r[k]
    v={'schema':1,'probe':'still-down-v1','events':{'records':records}};validate(v);return v
