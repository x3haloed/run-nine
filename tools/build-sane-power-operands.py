#!/usr/bin/env python3
"""Owned extended80 power pairs, generated with exact rational packing."""
from pathlib import Path
from fractions import Fraction
import sys
ROOT=Path(__file__).resolve().parents[1];DEST=ROOT/'probes/sanepower'
def packed(n,d=1):
    x=Fraction(n,d);sign=0x8000 if x<0 else 0;x=abs(x)
    if not x:return sign.to_bytes(2,'big')+bytes(8)
    e=x.numerator.bit_length()-x.denominator.bit_length()
    if x<Fraction(2)**e:e-=1
    scaled=x*Fraction(2)**(63-e);q,r=divmod(scaled.numerator,scaled.denominator)
    q+=r*2>scaled.denominator or (r*2==scaled.denominator and q&1)
    if q==1<<64:q>>=1;e+=1
    return (sign|(e+16383)).to_bytes(2,'big')+q.to_bytes(8,'big')
def main():
    bases=[packed(n,d)for n,d in [(0,1),(1,1),(-1,1),(2,1),(-2,1),(1,2),(-1,2),(3,4),(1,256),(1,65536),(65535,65536),(1<<16,1),(1,1<<16),(1<<100,1),(1,1<<100),((1<<63)+1,1<<63),((1<<64)-1,1<<64)]]
    bases+=[bytes.fromhex(s)for s in ['80000000000000000000','7fff0000000000000000','ffff0000000000000000','7fff4015000000000000','7fff0000000000000001']]
    exponents=[packed(n,d)for n,d in [(0,1),(1,1),(-1,1),(2,1),(-2,1),(1,2),(-1,2),(3,2),(1,3),(32767,1)]]
    pairs=[y+x for x in bases for y in exponents]
    # Dense positive curve grid, with independently packed exponent 5/4.
    pairs += [packed(5,4)+packed(i,256)for i in range(257)]
    data=b''.join(pairs);header='/* Owned rationally generated extended80 power pairs: exponent then base. */\n#define POWER_BASE_COUNT '+str(len(bases)*len(exponents))+'\n#define POWER_COUNT '+str(len(pairs))+'\nstatic const unsigned char power_operands[][20] = {\n'+''.join('{'+','.join(str(v)for v in b)+'},\n'for b in pairs)+'};\n'
    for name,value in [('operands.bin',data),('operands.h',header.encode())]:
        p=DEST/name
        if '--check'in sys.argv:assert p.read_bytes()==value,'Changed power operand '+name
        elif not p.exists()or p.read_bytes()!=value:p.write_bytes(value)
    print('Owned power pairs:',len(pairs),'base',len(bases)*len(exponents),'dense',257)
if __name__=='__main__':main()
