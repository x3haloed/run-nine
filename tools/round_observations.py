"""Rounded rectangle raster/pen contract with explicit unsupported frame inputs."""
import copy
FIELDS=set('which op bounds oval_width oval_height input_d0 input_ccr hidden_d0_preserved ccr stack_delta guard res_error pen_visibility before_pen_hex after_pen_hex before_pixels_hex pixels_hex'.split())
RAW=(FIELDS-{'hidden_d0_preserved'})|{'d0','mem_error'}
CONTEXTS=[(i//30,i//6%5,[0x12345678,0][i//3%2],[16,31,0][i%3])for i in range(630)]
def unsupported(which,op):return op==0 and which in[12,14]
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','drawing'}or v['schema']!=1 or v['probe']!='round-rect-v1':raise ValueError('Wrong rounded rectangle profile')
    c=v['drawing'];expected=[x for x in CONTEXTS if not unsupported(x[0],x[1])]
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=618:raise ValueError('Incomplete rounded rectangle matrix')
    for r,context in zip(c['records'],expected):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing rounded rectangle observation')
        if tuple(r[k]for k in ['which','op','input_d0','input_ccr'])!=context:raise ValueError('Changed rounded rectangle input')
        for k,x in r.items():
            if k=='bounds':
                if not isinstance(x,list)or len(x)!=4 or any(type(y)is not int for y in x):raise ValueError('Invalid rectangle bounds')
            elif k.endswith('_hex'):
                n=18 if 'pen' in k else 128
                if not isinstance(x,str)or len(x)!=n*2 or any(c not in'0123456789abcdef'for c in x):raise ValueError('Invalid rounded rectangle bytes')
            elif k=='hidden_d0_preserved':
                if context[0]==10:
                    if type(x)is not int or x not in[0,1]:raise ValueError('Missing hidden register relation')
                elif x is not None:raise ValueError('Unexpected ROM scratch claim')
            elif type(x)is not int:raise ValueError('Invalid rounded rectangle integer')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in['round-rect-raw-v1','round-rect-runtime-raw-v1']:raise ValueError('Wrong rounded rectangle capture')
    runtime=raw['probe']=='round-rect-runtime-raw-v1';rows=copy.deepcopy(raw['records']);selected=[]
    if not isinstance(rows,list)or len(rows)!=630:raise ValueError('Missing raw rounded matrix')
    for r,context in zip(rows,CONTEXTS):
        if not isinstance(r,dict)or set(r)!=(RAW|{'error'}if runtime else RAW):raise ValueError('Missing raw rounded field')
        if tuple(r[k]for k in ['which','op','input_d0','input_ccr'])!=context:raise ValueError('Changed raw rounded input')
        if type(r['d0'])is not int or not 0<=r['d0']<=0xffffffff or type(r['mem_error'])is not int:raise ValueError('Missing raw register/cache error')
        for k,x in r.items():
            if k=='error':continue
            if k=='bounds':
                if not isinstance(x,list)or len(x)!=4 or any(type(y)is not int for y in x):raise ValueError('Invalid raw bounds')
            elif k.endswith('_hex'):
                n=18 if 'pen' in k else 128
                if not isinstance(x,str)or len(x)!=2*n or any(c not in'0123456789abcdef'for c in x):raise ValueError('Invalid raw raster/pen bytes')
            elif type(x)is not int:raise ValueError('Invalid raw rounded integer')
        if unsupported(r['which'],r['op']):
            if runtime:
                message='unsupported rounded frame with '+('zero pen dimension'if r['which']==12 else'inverted bounds')
                if r['error']!=message or r['d0']!=r['input_d0']or r['ccr']!=r['input_ccr']or r['stack_delta']!=-8 or r['guard']!=0x55667788 or r['before_pen_hex']!=r['after_pen_hex']or r['before_pixels_hex']!=r['pixels_hex']:raise ValueError('Unsupported frame was not an unchanged explicit rejection')
            continue
        if runtime:
            if r.pop('error')is not None:raise ValueError('Supported rounded primitive rejected')
        r['hidden_d0_preserved']=int(r['d0']==r['input_d0'])if r['which']==10 else None
        r.pop('d0');r.pop('mem_error');selected.append(r)
    v={'schema':1,'probe':'round-rect-v1','drawing':{'records':selected}};validate(v);return v
