#!/usr/bin/env python3
"""Owned IEEE32 inputs for native FS2X, without host floating-point math."""
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1];DEST=ROOT/'probes/sanef32widen'
def main():
    values=[0,1,0x007fffff,0x00800000,0x3f000000,0x3f800000,0x3f800001,0x3fc00000,0x40000000,0x7f7fffff,0x7f800000,0x7fc00000,0x7f800001,0x7fc44000,0x7fd5a123,0x7fffffff]
    rows=[v|sign for v in values for sign in [0,1<<31]]
    for exponent in[1,0x7e,0x7f,0x80,0xfd,0xfe]:
        for fraction in[0,1,1<<22,(1<<23)-1]:
            for sign in[0,1<<31]:rows.append(sign|(exponent<<23)|fraction)
    state=0x54f3678c;exponents=[0,1,2,0x7e,0x7f,0x80,0xfd,0xfe]
    for i in range(128):
        state^=(state<<13)&0xffffffff;state^=state>>17;state^=(state<<5)&0xffffffff
        rows.append((state&((1<<23)-1))|(exponents[i%8]<<23)|((i&1)<<31))
    data=[v.to_bytes(4,'big')for v in rows];header='/* Owned generated IEEE32 operands. */\n#define SINGLE_WIDEN_COUNT '+str(len(data))+'\nstatic const unsigned char single_widen_operands[][4] = {\n'+''.join('{'+','.join(str(v)for v in b)+'},\n'for b in data)+'};\n'
    for name,value in [('operands.bin',b''.join(data)),('operands.h',header.encode())]:
        p=DEST/name
        if '--check'in sys.argv:assert p.read_bytes()==value,'Changed single operand '+name
        elif not p.exists()or p.read_bytes()!=value:p.write_bytes(value)
    print('Owned IEEE32 FS2X operands:',len(rows))
if __name__=='__main__':main()
