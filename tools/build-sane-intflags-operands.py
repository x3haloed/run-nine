#!/usr/bin/env python3
"""Owned exact exponent/mantissa/sign grid for integer conversion diagnostics."""
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1]
def main():
    vals=[]
    for e in [-1,0,14,15,16,30,31,32,62,63,64,100,16383]:
        for m in [1<<63,(1<<63)+1,(1<<63)+2,(1<<63)+3,(1<<64)-2,(1<<64)-1]:
            for sign in [0,0x8000]:vals.append((sign|e+16383).to_bytes(2,'big')+m.to_bytes(8,'big'))
    header='/* Owned exact integer conversion edge cases; exponent/mantissa/sign grid. */\n#define ROUND_COUNT '+str(len(vals))+'\nstatic const unsigned char round_operands[][10]={\n'+''.join('{'+','.join(map(str,b))+'},\n'for b in vals)+'};\n'
    for name,data in [('operands.bin',b''.join(vals)),('operands.h',header.encode())]:
        p=ROOT/'probes/saneintflags'/name
        if '--check'in sys.argv:assert p.read_bytes()==data
        elif not p.exists()or p.read_bytes()!=data:p.write_bytes(data)
    print('Owned integer conversion flag fixtures:',len(vals))
if __name__=='__main__':main()
