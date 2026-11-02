#!/usr/bin/env python3
"""Independent integer-only audit of native signed32 widening and status."""
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def audit():
    rows=json.loads((ROOT/'outputs/SANEFL2XProbe-v1-reference.json').read_text())['records']
    for r in rows:
        bits=int(r['source_before_hex'],16)
        n=bits if bits<0x80000000 else bits-0x100000000
        a=abs(n);env=r['input_env'];precision={0:64,32:53,64:24}[env&96];flags=0
        if a:
            top=a.bit_length()-1;shift=max(0,top+1-precision)
            q=a>>shift;rem=a-(q<<shift);mode=(env>>13)&3
            inc=bool(rem) and ((mode==0 and (rem*2>1<<shift or (rem*2==1<<shift and q&1))) or (mode==1 and n>=0) or (mode==2 and n<0))
            if rem:flags=0x1000|(0x80 if inc else 0)
            rounded=(q+inc)<<shift;top=rounded.bit_length()-1
            expected=((0x8000 if n<0 else 0)|(16383+top)).to_bytes(2,'big')+(rounded<<(63-top)).to_bytes(8,'big')
        else:expected=bytes(10)
        assert r['dest_hex']==expected.hex(),r
        assert r['output_env']==(env&~0x80)|flags,r
        assert r['ccr']==(r['input_ccr']&16)|(4 if n==0 else 0),r
        assert r['source_after_hex']==r['source_before_hex']
        assert r['d0']==0x12345678 and r['stack_delta']==0
        assert r['guards']==[0x11223344,0x55667788,0x99aabbcc,0xddeeff00]
    return len(rows)
if __name__=='__main__':print(json.dumps({'independent_integer_audit_records':audit(),'passed':True}))
