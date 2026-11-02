"""Roman TruncText/TruncString public buffer, result, errors and CCR contract."""
import copy
FIELDS=set('op fontctx which mode width_case width full_width ellipsis_width input_d0 input_ccr before_length before_hex result ccr stack_delta guard mem_error res_error prefix suffix length_prefix length_suffix length_word after_length after_hex'.split())
def validate(value):
    if not isinstance(value,dict) or set(value)!={'schema','probe','text'} or value['schema']!=1 or value['probe']!='text-truncation-v1':raise ValueError('Wrong text truncation profile')
    c=value['text']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=1728:raise ValueError('Incomplete truncation matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS:raise ValueError('Missing truncation observation')
        if tuple(r[k]for k in ['op','fontctx','which','mode','width_case','input_d0','input_ccr'])!=(i//864,i//432%2,i//108%4,i//54%2,i//6%9,[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed truncation context')
        for key,x in r.items():
            if key.endswith('_hex'):
                if not isinstance(x,str) or len(x)!=512 or any(c not in '0123456789abcdef'for c in x):raise ValueError('Invalid truncation bytes')
            elif type(x)is not int:raise ValueError('Invalid truncation integer')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','records'} or raw['schema']!=1 or raw['probe']not in ['text-truncation-raw-v1','text-truncation-runtime-raw-v1']:raise ValueError('Wrong raw truncation capture')
    rows=copy.deepcopy(raw['records'])
    if not isinstance(rows,list):raise ValueError('Missing raw truncation records')
    for r in rows:
        if not isinstance(r,dict) or set(r)!=FIELDS|{'d0'} or type(r['d0'])is not int or not 0<=r['d0']<=0xffffffff:raise ValueError('Missing raw truncation field')
        # D0 contains native ROM/dispatcher scratch, retained only in raw output.
        r.pop('d0')
    value={'schema':1,'probe':'text-truncation-v1','text':{'records':rows}};validate(value);return value
