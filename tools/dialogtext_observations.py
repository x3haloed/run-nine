"""GetDialogItemText copies a bounded Pascal string without changing handle state."""
import copy
FIELDS=set('which state length input_d0 input_ccr d0 ccr stack_delta guard mem_error res_error prefix suffix before_state after_state after_hex'.split())
def validate(value):
    if not isinstance(value,dict) or set(value)!={'schema','probe','dialogs'} or value['schema']!=1 or value['probe']!='dialog-item-text-v1':raise ValueError('Wrong dialog text profile')
    c=value['dialogs']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=96:raise ValueError('Incomplete dialog text matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS:raise ValueError('Missing dialog text observation')
        if tuple(r[k]for k in ['which','state','length','input_d0','input_ccr'])!=(i//12,i//6%2,[0,1,8,64,127,255,256,300][i//12],[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed dialog text input')
        for k,x in r.items():
            if k=='after_hex':
                if not isinstance(x,str) or len(x)!=1200 or any(c not in '0123456789abcdef'for c in x):raise ValueError('Invalid dialog text bytes')
            elif type(x)is not int:raise ValueError('Invalid dialog text integer')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']not in ['dialog-item-text-raw-v1','dialog-item-text-runtime-raw-v1']:raise ValueError('Wrong raw dialog text capture')
    value={'schema':1,'probe':'dialog-item-text-v1','dialogs':{'records':copy.deepcopy(raw['records'])}};validate(value);return value
