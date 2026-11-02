"""Classic line raster/public pen state. Native visible-line D0 scratch and QD cache MemErr are excluded."""
import copy
FIELDS=set('which op input_d0 input_ccr h v hidden_d0_preserved ccr stack_delta guard res_error pen_visibility before_hex after_hex pixels_hex'.split())
START=[(3,4),(15,4),(5,3),(3,3),(20,3),(3,3),(20,10),(-4,-3),(8,8),(3,3),(3,3),(3,3)]
END=[(15,4),(3,4),(5,19),(20,20),(3,20),(20,10),(3,3),(35,38),(8,8),(18,9),(18,9),(18,9)]
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','drawing'}or v['schema']!=1 or v['probe']!='qd-line-v1':raise ValueError('Wrong line profile')
    c=v['drawing']
    if not isinstance(c,dict)or set(c)!={'records'}or not isinstance(c['records'],list)or len(c['records'])!=144:raise ValueError('Incomplete line matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing line observation')
        which=i//12;op=i//6%2;h,v=END[which]if not op else tuple(b-a for a,b in zip(START[which],END[which]))
        if tuple(r[k]for k in ['which','op','input_d0','input_ccr','h','v'])!=(which,op,[0x12345678,0][i//3%2],[16,31,0][i%3],h,v):raise ValueError('Changed line context')
        for k,x in r.items():
            if k.endswith('_hex'):
                n=128 if k=='pixels_hex'else 18
                if not isinstance(x,str)or len(x)!=n*2 or any(c not in '0123456789abcdef'for c in x):raise ValueError('Invalid line bytes')
            elif k=='hidden_d0_preserved':
                if which==10:
                    if type(x)is not int or x not in[0,1]:raise ValueError('Missing hidden-line register relation')
                elif x is not None:raise ValueError('Unexpected visible-line D0 claim')
            elif type(x)is not int:raise ValueError('Invalid line integer')
def project(raw):
    rawfields=(FIELDS-{'hidden_d0_preserved'})|{'d0','mem_error'}
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in['qd-line-raw-v1','qd-line-runtime-raw-v1']:raise ValueError('Wrong raw line capture')
    rows=copy.deepcopy(raw['records'])
    if not isinstance(rows,list):raise ValueError('Missing raw line records')
    for r in rows:
        if not isinstance(r,dict)or set(r)!=rawfields or type(r['d0'])is not int or type(r['mem_error'])is not int:raise ValueError('Missing raw line field')
        r['hidden_d0_preserved']=int(r.pop('d0')==r['input_d0'])if r['pen_visibility']<0 else None
        # The visible branch still has an unclaimed D0 scratch field to remove.
        r.pop('d0',None);r.pop('mem_error')
    v={'schema':1,'probe':'qd-line-v1','drawing':{'records':rows}};validate(v);return v
