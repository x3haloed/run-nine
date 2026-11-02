"""Strict bitmap picture pixel/ABI comparisons with retained palette RGB values.

Raw captures retain complete destination color tables. This picture profile
compares seed/flags/count/RGB, not auxiliary ColorSpec.value bookkeeping.
"""
import copy,re,struct
FIELDS={'kind','destination','clip','ink','input_d0','input_ccr','d0','ccr','stack_delta','guard','mem_error','res_error','picture_state','picture_preserved','port_preserved','clip_preserved','dest_rect','pixels_hex'}
DESTS=[[2,3,10,19],[1,2,17,34],[3,4,7,12],[-2,-3,6,13],[4,4,4,20]]
def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','pictures'}or v['schema']!=1 or v['probe']!='pict-bitmap-v1':raise ValueError('Wrong picture profile')
    p=v['pictures']
    if not isinstance(p,dict)or set(p)!={'depth','row_bytes','destination_palette','records'}or(p['depth'],p['row_bytes'])!=(8,64):raise ValueError('Changed picture destination')
    table=p['destination_palette']
    if not isinstance(table,dict)or set(table)!={'seed','flags','count','rgb_hex'}or any(type(table[k])is not int for k in ['seed','flags','count']):raise ValueError('Missing destination palette field')
    if(table['seed'],table['flags'],table['count'])!=(8,32768,256)or not isinstance(table['rgb_hex'],str)or re.fullmatch('[0-9a-f]{3072}',table['rgb_hex'])is None:raise ValueError('Changed picture palette')
    rows=p['records']
    if not isinstance(rows,list)or len(rows)!=480:raise ValueError('Incomplete picture bitmap matrix')
    for i,r in enumerate(rows):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing picture field')
        if any(type(r[k])is not int for k in FIELDS-{'dest_rect','pixels_hex'}):raise ValueError('Invalid picture scalar')
        context=(i//120,i//24%5,i//12%2,i//6%2,[0x12345678,0][i//3%2],[16,31,0][i%3])
        if tuple(r[k]for k in ['kind','destination','clip','ink','input_d0','input_ccr'])!=context:raise ValueError('Changed picture context')
        if not isinstance(r['dest_rect'],list)or any(type(x)is not int for x in r['dest_rect'])or r['dest_rect']!=DESTS[i//24%5]:raise ValueError('Changed destination rectangle')
        if not isinstance(r['pixels_hex'],str)or re.fullmatch('[0-9a-f]{3072}',r['pixels_hex'])is None:raise ValueError('Incomplete destination pixels')
def project(raw):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','depth','row_bytes','destination_table_hex','records'}or raw['schema']!=1 or raw['probe']not in['pict-bitmap-raw-v1','pict-bitmap-runtime-raw-v1']:raise ValueError('Wrong raw picture capture')
    text=raw['destination_table_hex']
    if not isinstance(text,str)or re.fullmatch('[0-9a-f]{4112}',text)is None:raise ValueError('Incomplete raw destination color table')
    table=bytes.fromhex(text);seed,flags,last=struct.unpack_from('>IHH',table)
    rgb=b''.join(table[10+i*8:16+i*8]for i in range(last+1))
    v={'schema':1,'probe':'pict-bitmap-v1','pictures':{'depth':raw['depth'],'row_bytes':raw['row_bytes'],'destination_palette':{'seed':seed,'flags':flags,'count':last+1,'rgb_hex':rgb.hex()},'records':copy.deepcopy(raw['records'])}}
    validate(v);return v
