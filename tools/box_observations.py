"""TETextBox layout, port state and empty-box raster; glyph pixels/cache errors remain raw."""
import copy
FIELDS=set('which fontctx height_case box justification input_d0 input_ccr calls d0 ccr stack_delta guard after_clip before_pen_hex after_pen_hex before_text_hex after_text_hex empty_pixels_hex'.split())
CALLFIELDS=set('count point font size face mode clip numer denom text_hex'.split())
def hexbytes(x,n):
    return isinstance(x,str)and len(x)==n*2 and all(c in '0123456789abcdef'for c in x)
def vector(x,n):return isinstance(x,list)and len(x)==n and all(type(v)is int for v in x)
def validate(value):
    if not isinstance(value,dict)or set(value)!={'schema','probe','text'}or value['schema']!=1 or value['probe']!='text-box-v1':raise ValueError('Wrong text box profile')
    c=value['text']
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=1152:raise ValueError('Incomplete text box matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing text box field')
        if tuple(r[k]for k in ['which','fontctx','height_case','justification','input_d0','input_ccr'])!=(i//144,i//72%2,i//18%4,[0,1,-1][i//6%3],[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed text box input')
        if r['box']!=[2,3,2+[0,1,24,56][i//18%4],63]:raise ValueError('Changed box bounds')
        for k,x in r.items():
            if k=='calls':
                if not isinstance(x,list)or len(x)>64:raise ValueError('Missing text draw observations')
                for call in x:
                    if not isinstance(call,dict)or set(call)!=CALLFIELDS:raise ValueError('Missing draw argument')
                    for name,a in call.items():
                        if name in ['point','numer','denom']:
                            if not vector(a,2):raise ValueError('Invalid draw point/ratio')
                        elif name=='clip':
                            if not vector(a,4):raise ValueError('Invalid draw clip')
                        elif name=='text_hex':
                            if type(call['count'])is not int or not 0<=call['count']<=32767 or not hexbytes(a,call['count']):raise ValueError('Invalid text run')
                        elif type(a)is not int:raise ValueError('Invalid draw integer')
            elif k in ['box','after_clip']:
                if not vector(x,4):raise ValueError('Invalid box/clip')
            elif k=='empty_pixels_hex':
                if r['which']==0:
                    if not hexbytes(x,768):raise ValueError('Missing empty-box raster')
                elif x is not None:raise ValueError('Unexpected glyph pixel claim')
            elif k.endswith('_hex'):
                if not hexbytes(x,18 if 'pen' in k else 20):raise ValueError('Invalid port state bytes')
            elif type(x)is not int:raise ValueError('Invalid text box integer')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in ['text-box-raw-v3','text-box-runtime-raw-v3']:raise ValueError('Wrong raw text box capture')
    rows=copy.deepcopy(raw['records'])
    if not isinstance(rows,list):raise ValueError('Missing raw text box records')
    for r in rows:
        if not isinstance(r,dict)or set(r)!=(FIELDS-{'empty_pixels_hex'})|{'pixels_hex','mem_error','res_error'}:raise ValueError('Missing raw box observation')
        for k in ['mem_error','res_error']:
            if type(r[k])is not int or not -32768<=r[k]<=32767:raise ValueError('Missing raw cache error observation')
            r.pop(k)
        pixels=r.pop('pixels_hex')
        if not hexbytes(pixels,768):raise ValueError('Missing raw text box raster')
        r['empty_pixels_hex']=pixels if r['which']==0 else None
    value={'schema':1,'probe':'text-box-v1','text':{'records':rows}};validate(value);return value
