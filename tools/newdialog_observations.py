"""Resource dialogs: normalize only allocated pointer identity and retain D0/result relation."""
import copy
COMMON=set('which storage input_d0 input_ccr d0_is_result result_nonnull ccr stack_delta guard mem_error res_error current_port_preserved front_changed dst_reused prefix suffix'.split())
PUBLIC=set('window_kind visible hilited go_away refcon port_version port_rect edit_field edit_open default_item items_nonnull textedit_nonnull title_hex items'.split())
DETAILS=set('global_origin bitmap_bounds te_dest te_view te_font te_size te_text_length te_port_matches'.split())
RAW_COMMON=(COMMON-{'d0_is_result','result_nonnull'})|{'d0','result_address'}
def ishex(x):return isinstance(x,str)and len(x)%2==0 and all(c in '0123456789abcdef'for c in x)
def array(x,n):return isinstance(x,list)and len(x)==n and all(type(t)is int for t in x)
def validate_rows(rows,details=True):
    if not isinstance(rows,list)or len(rows)!=(96 if details else 48):raise ValueError('Incomplete dialog matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict):raise ValueError('Invalid dialog record')
        success=i//12 not in [2,3]
        fields=COMMON|(PUBLIC|(DETAILS if details else set())if success else set())
        if set(r)!=fields:raise ValueError('Missing dialog observation')
        if tuple(r[k]for k in ['which','storage','input_d0','input_ccr'])!=(i//12,i//6%2,[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed dialog context')
        for k,x in r.items():
            if k=='items':
                if not isinstance(x,list)or len(x)!=5:raise ValueError('Missing dialog item')
                for item in x:
                    if not isinstance(item,dict)or set(item)!={'kind','nonnull','rect','text_hex'}or type(item['kind'])is not int or item['nonnull']not in [0,1]or not array(item['rect'],4)or not ishex(item['text_hex']):raise ValueError('Invalid dialog item')
            elif k.endswith('_hex'):
                if not ishex(x):raise ValueError('Invalid dialog text')
            elif k in ['port_rect','bitmap_bounds','te_dest','te_view','global_origin']:
                if not array(x,2 if k=='global_origin'else 4):raise ValueError('Invalid dialog rectangle')
            elif type(x)is not int:raise ValueError('Invalid dialog integer')
        if r['result_nonnull']not in [0,1]or r['d0_is_result']not in [0,1]:raise ValueError('Invalid pointer relation')
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','dialogs'}or v['schema']!=1 or v['probe']!='new-dialog-v1':raise ValueError('Wrong dialog profile')
    c=v['dialogs']
    if not isinstance(c,dict)or set(c)!={'records'}:raise ValueError('Missing dialogs')
    validate_rows(c['records'])
def project(raw,details=True):
    probes=['new-dialog-details-raw-v1','new-dialog-details-runtime-raw-v1']if details else ['new-dialog-raw-v1']
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','records'}or raw['schema']!=1 or raw['probe']not in probes:raise ValueError('Wrong raw dialog capture')
    rows=copy.deepcopy(raw['records'])
    if not isinstance(rows,list):raise ValueError('Missing dialog records')
    for i,r in enumerate(rows):
        fields=RAW_COMMON|(PUBLIC|(DETAILS if details else set())if i//12 not in [2,3]else set())
        if not isinstance(r,dict)or set(r)!=fields or any(type(r[k])is not int or not 0<=r[k]<=0xffffffff for k in ['result_address','d0']):raise ValueError('Missing raw dialog observation')
        pointer=r.pop('result_address');r['d0_is_result']=int(r.pop('d0')==pointer);r['result_nonnull']=int(pointer!=0)
    validate_rows(rows,details)
    if not details:return rows
    v={'schema':1,'probe':'new-dialog-v1','dialogs':{'records':rows}};validate(v);return v
