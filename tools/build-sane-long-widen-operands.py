#!/usr/bin/env python3
"""Owned signed32 inputs, including boundaries around single-precision rounding."""
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1];DEST=ROOT/'probes/sanei32widen'
def main():
    rows=[0,1,0xffffffff,0x7fffffff,0x80000000]
    for power in [1,15,23,24,25,30]:
        for delta in [-3,-2,-1,0,1,2,3]:
            for sign in [1,-1]:rows.append((sign*((1<<power)+delta))&0xffffffff)
    state=0x74e3a16b
    for i in range(64):
        state^=(state<<13)&0xffffffff;state^=state>>17;state^=(state<<5)&0xffffffff
        rows.append(state)
    data=[v.to_bytes(4,'big') for v in rows]
    header='/* Owned generated signed32 operands. */\n#define LONG_WIDEN_COUNT '+str(len(data))+'\nstatic const unsigned char long_widen_operands[][4] = {\n'+''.join('{'+','.join(str(v)for v in b)+'},\n'for b in data)+'};\n'
    for name,value in [('operands.bin',b''.join(data)),('operands.h',header.encode())]:
        p=DEST/name
        if '--check'in sys.argv:assert p.read_bytes()==value,'Changed signed32 operand '+name
        elif not p.exists()or p.read_bytes()!=value:p.write_bytes(value)
    print('Owned signed32 FL2X operands:',len(rows))
if __name__=='__main__':main()
