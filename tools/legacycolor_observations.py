"""Documented eight-color assignment and RGB reader ABI; all retained fields exact."""
import copy
COLORS=[30,33,205,341,409,273,137,69]
EXTRA=[0,1,-1,0x12345678]
BASE={'port_kind','op','color','input_d0','input_ccr','d0','ccr','stack_delta','guard','mem_error','res_error','fg_color','bg_color','foreground','background','other_port_bytes_preserved'}
READ={'result','result_before_guard','result_after_guard'}
def rows(records,read,raw_native=False):
    colors=COLORS+EXTRA if raw_native and not read else COLORS
    if not isinstance(records,list)or len(records)!=2*2*len(colors)*2*3:raise ValueError('Incomplete legacy color matrix')
    fields=BASE|READ if read else BASE
    for i,r in enumerate(records):
        if not isinstance(r,dict)or set(r)!=fields:raise ValueError('Missing color observation field')
        arrays={'foreground','background','result'} if read else {'foreground','background'}
        if any(type(r[k])is not int for k in fields-arrays):raise ValueError('Invalid color scalar')
        for k in arrays:
            if not isinstance(r[k],list)or len(r[k])!=3 or any(type(x)is not int or not 0<=x<=65535 for x in r[k]):raise ValueError('Invalid RGB triplet')
        n=len(colors)
        if tuple(r[k]for k in ['port_kind','op','color','input_d0','input_ccr'])!=(i//(n*12),i//(n*6)%2,colors[i//6%n],[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed color context')
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','colors'}or v['schema']!=1 or v['probe']not in['legacy-color-v1','color-read-v1']:raise ValueError('Wrong color profile')
    c=v['colors']
    if not isinstance(c,dict)or set(c)!={'depth','records'}or type(c['depth'])is not int or c['depth']!=8:raise ValueError('Changed color display')
    rows(c['records'],v['probe']=='color-read-v1')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','depth','records'}or raw['schema']!=1 or raw['probe']not in['legacy-color-raw-v1','legacy-color-runtime-raw-v1','color-read-raw-v1','color-read-runtime-raw-v1']:raise ValueError('Wrong raw color capture')
    read=raw['probe'].startswith('color-read-');native='runtime'not in raw['probe'];rows(raw['records'],read,native)
    selected=[copy.deepcopy(r)for r in raw['records']if r['color']in COLORS]
    v={'schema':1,'probe':'color-read-v1'if read else'legacy-color-v1','colors':{'depth':raw['depth'],'records':selected}}
    validate(v);return v
