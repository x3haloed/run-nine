"""Volume ABI and write footprint; physical owner metadata remains raw only."""
import copy
INTS=set('which input_d0 input_ccr d0 ccr stack_delta a0_preserved guard mem_error res_error name_pointer_preserved name_value_correct name_untouched_correct volume_identity_consistent owner_metadata_consistent owner_geometry_valid'.split())
FIELDS=INTS|{'before_hex','after_hex','pb_guards','name_guards'}
RAW=set('which input_d0 input_ccr name_address before_hex name_before_hex d0 ccr stack_delta a0_preserved guard pb_guards name_guards mem_error res_error after_hex name_after_hex'.split())
OWNER=[(30,38),(40,52),(56,64),(66,70),(72,76),(78,122)]
def byte_string(v,n):
    if not isinstance(v,str) or len(v)!=2*n:raise ValueError('Missing volume bytes')
    try:return bytearray.fromhex(v)
    except ValueError:raise ValueError('Invalid volume bytes')
def validate(v):
    if not isinstance(v,dict) or set(v)!={'schema','probe','volumes'} or v['schema']!=1 or v['probe']!='hgetvinfo-v1':raise ValueError('Wrong volume profile')
    c=v['volumes']
    if not isinstance(c,dict) or set(c)!={'records'} or not isinstance(c['records'],list) or len(c['records'])!=48:raise ValueError('Incomplete volume matrix')
    for i,r in enumerate(c['records']):
        if not isinstance(r,dict) or set(r)!=FIELDS or any(type(r[k])is not int for k in INTS):raise ValueError('Missing volume field')
        if tuple(r[k]for k in ['which','input_d0','input_ccr'])!=(i//6,[0x12345678,0][i//3%2],[16,31,0][i%3]):raise ValueError('Changed volume context')
        for k in ['before_hex','after_hex']:byte_string(r[k],128)
        for k in ['pb_guards','name_guards']:
            if not isinstance(r[k],list) or len(r[k])!=2 or any(type(x)is not int for x in r[k]):raise ValueError('Missing volume guards')
def project(raw):
    if not isinstance(raw,dict) or set(raw)!={'schema','probe','current_ref','current_name_hex','records'} or raw['schema']!=1 or raw['probe']not in ['hgetvinfo-raw-v1','hgetvinfo-runtime-raw-v1']:raise ValueError('Wrong raw volume capture')
    current=raw['current_ref'];name=bytearray.fromhex(raw['current_name_hex']);rows=[]
    if type(current)is not int or not name or name[0]+1!=len(name):raise ValueError('Missing volume context')
    anchor=owner=None
    for source in raw['records']:
        if not isinstance(source,dict) or set(source)!=RAW:raise ValueError('Missing raw volume field')
        r={k:copy.deepcopy(source[k])for k in INTS&source.keys()};which=r['which'];np=source['name_address']
        if type(np)is not int:raise ValueError('Missing name address')
        before=byte_string(source['before_hex'],128);after=byte_string(source['after_hex'],128)
        nb=byte_string(source['name_before_hex'],256);na=byte_string(source['name_after_hex'],256)
        expected=bytearray([0xa5]*256)
        if which==5:expected[:len(name)]=name
        if which==6:expected[:17]=b'\x10NotAnOwnedVolume'
        if nb!=expected:raise ValueError('Changed owned name input')
        if int.from_bytes(before[18:22],'big')!=(0 if which==7 else np):raise ValueError('Wrong input pointer')
        if int.from_bytes(before[22:24],'big',signed=True)!=(current if which==1 else 32767 if which==4 else 0):raise ValueError('Changed reference input')
        r['name_pointer_preserved']=int(after[18:22]==before[18:22])
        r['name_value_correct']=r['name_untouched_correct']=1
        r['volume_identity_consistent']=r['owner_metadata_consistent']=r['owner_geometry_valid']=1
        if r['d0']==0:
            ref=bytes(after[22:24]);payload=bytes(after[30:40]+after[42:122])
            if anchor is None:anchor=ref;owner=payload
            r['volume_identity_consistent']=int(ref==anchor)
            r['owner_metadata_consistent']=int(payload==owner)
            u=lambda off,n:int.from_bytes(after[off:off+n],'big')
            r['owner_geometry_valid']=int(u(46,2)>0 and u(48,4)>0 and u(48,4)%512==0 and u(62,2)<=u(46,2) and u(40,2)<=u(82,4)+u(86,4) and u(58,4)>u(82,4)+u(86,4)+2 and int.from_bytes(after[66:68],'big',signed=True)>0 and int.from_bytes(after[68:70],'big',signed=True)<0)
            if which!=7:
                r['name_value_correct']=int(na[:len(name)]==name)
                r['name_untouched_correct']=int(na[len(name):]==nb[len(name):])
            else:r['name_untouched_correct']=int(na==nb)
            after[22:24]=b'\x70\x01'
            for start,end in OWNER:after[start:end]=b'\0'*(end-start)
            # Preserve public hardware/software lock bits; other native volume
            # status flags describe its physical owner and remain in the raw file.
            after[38:40]=(u(38,2)&0x8080).to_bytes(2,'big')
        else:r['name_untouched_correct']=int(na==nb)
        if which==1:before[22:24]=b'\x70\x00'
        before[18:22]=(0 if which==7 else 0x5000).to_bytes(4,'big')
        after[18:22]=before[18:22]
        after[8:12]=b'\0'*4 # Private native ROM command pointer.
        r['before_hex']=before.hex();r['after_hex']=after.hex()
        for k in ['pb_guards','name_guards']:r[k]=copy.deepcopy(source[k])
        rows.append(r)
    v={'schema':1,'probe':'hgetvinfo-v1','volumes':{'records':rows}};validate(v);return v
