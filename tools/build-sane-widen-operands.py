#!/usr/bin/env python3
"""Owned IEEE64 bit patterns for direct FP68K FD2X measurements."""
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1];DEST=ROOT/'probes/sanewiden'
def main():
    values=[0,1,0x000fffffffffffff,0x0010000000000000,0x3fe0000000000000,0x3ff0000000000000,0x3ff0000000000001,0x3ff8000000000000,0x4000000000000000,0x7fefffffffffffff,0x7ff0000000000000,0x7ff8000000000000,0x7ff0000000000001,0x7ff8044000000000,0x7ffab542468acf13,0x7fffffffffffffff]
    rows=[v|sign for v in values for sign in [0,1<<63]]
    for exponent in [-150,-149,-127,-126,127,128]:
        for fraction in [0,1,1<<28,(1<<52)-1]:
            for sign in [0,1<<63]:rows.append(sign|((exponent+1023)<<52)|fraction)
    state=0x54f3678c921ba05d;exponents=[0,1,2,0x3fe,0x3ff,0x400,0x7fd,0x7fe]
    for i in range(128):
        state^=(state<<13)&((1<<64)-1);state^=state>>7;state^=(state<<17)&((1<<64)-1)
        rows.append((state&((1<<52)-1))|(exponents[i%8]<<52)|((i&1)<<63))
    data=[v.to_bytes(8,'big') for v in rows];header='/* Owned generated IEEE64 operands. */\n#define WIDEN_COUNT '+str(len(data))+'\nstatic const unsigned char widen_operands[][8] = {\n'+''.join('{'+','.join(str(v) for v in b)+'},\n' for b in data)+'};\n'
    for name,value in [('operands.bin',b''.join(data)),('operands.h',header.encode())]:
        p=DEST/name
        if '--check' in sys.argv:assert p.read_bytes()==value,'Changed widen operand '+name
        elif not p.exists() or p.read_bytes()!=value:p.write_bytes(value)
    print('Owned IEEE64 operands:',len(data))
if __name__=='__main__':main()
