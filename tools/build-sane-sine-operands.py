#!/usr/bin/env python3
"""Owned dyadic angles and adjacent packed80 pi boundaries for SANE sine."""
from pathlib import Path
from decimal import Decimal,localcontext,ROUND_HALF_EVEN
import sys
ROOT=Path(__file__).resolve().parents[1];DEST=ROOT/'probes/sanesine'
def main():
    base=[bytes.fromhex(s) for s in (ROOT/'probes/sane/base-operands.txt').read_text().splitlines()]
    with localcontext() as context:
        context.prec=140
        def atan(x):
            total=x;power=x;square=x*x
            for k in range(1,1000):
                power=-power*square;after=total+power/Decimal(2*k+1)
                if after==total:return total
                total=after
            raise RuntimeError('Machin series did not converge')
        pi=16*atan(Decimal(1)/5)-4*atan(Decimal(1)/239)
        for multiple in [1,2,3,4]:
            x=pi*multiple/2;e=0
            while Decimal(2)**e>x:e-=1
            while Decimal(2)**(e+1)<=x:e+=1
            mant=int((x*Decimal(2)**(63-e)).to_integral_value(rounding=ROUND_HALF_EVEN))
            for delta in [-1,0,1]:
                for sign in [0,0x8000]:base.append((sign|(e+16383)).to_bytes(2,'big')+(mant+delta).to_bytes(8,'big'))
    dense=[]
    for i in range(1025):
        for sign in [0,0x8000]:
            dense.append(((i.bit_length()-1-7+16383)|sign).to_bytes(2,'big')+(i<<(64-i.bit_length())).to_bytes(8,'big') if i else sign.to_bytes(2,'big')+bytes(8))
    source=(ROOT/'probes/sane/operands.bin').read_bytes();dense.extend(source[i:i+10] for i in range(len(source)-1280,len(source),10));values=base+dense
    header='/* Owned sine angles, independently generated. */\n#define SANE_BASE_COUNT '+str(len(base))+'\n#define SANE_DENSE_COUNT '+str(len(dense))+'\nstatic const unsigned char sane_operands[][10] = {\n'+''.join('{'+','.join(str(v)for v in b)+'},\n'for b in values)+'};\n'
    for name,data in [('operands.bin',b''.join(values)),('operands.h',header.encode()),('operand_counts.rs',f'const BASE_COUNT:usize={len(base)};\nconst DENSE_COUNT:usize={len(dense)};\n'.encode())]:
        p=DEST/name
        if '--check' in sys.argv:assert p.read_bytes()==data,'Changed sine fixture '+name
        elif not p.exists() or p.read_bytes()!=data:p.write_bytes(data)
    print('Owned sine fixtures:',len(base),'base,',len(dense),'dense')
if __name__=='__main__':main()
