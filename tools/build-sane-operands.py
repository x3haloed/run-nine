#!/usr/bin/env python3
"""Owned dyadic/range/mantissa fixtures, shared by native and independent replay."""
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1];SOURCE=ROOT/'probes/sane'
def main():
    base=[bytes.fromhex(s)for s in(SOURCE/'base-operands.txt').read_text().splitlines()];assert len(base)==38 and all(len(b)==10 for b in base)
    dense=[]
    def encoded(exp,mant,sign):return ((exp+16383)|(sign<<15)).to_bytes(2,'big')+mant.to_bytes(8,'big')
    for i in range(257):
        for sign in [0,1]:
            dense.append(encoded(i.bit_length()-1-6,i<<(64-i.bit_length()),sign)if i else (sign<<15).to_bytes(2,'big')+bytes(8))
    state=0x78d03ab45c918fe2
    exponents=[-32,-12,-3,-1,0,1,3,12,63,100]
    for i in range(128):
        state^=(state<<13)&((1<<64)-1);state^=state>>7;state^=(state<<17)&((1<<64)-1)
        dense.append(encoded(exponents[i%len(exponents)],state|(1<<63),i&1))
    values=base+dense;blob=b''.join(values)
    header='/* Generated owned operands; tools/build-sane-operands.py. */\n#define SANE_BASE_COUNT 38\n#define SANE_DENSE_COUNT '+str(len(dense))+'\nstatic const unsigned char sane_operands[][10] = {\n'+''.join('{'+','.join(str(v)for v in b)+'},\n'for b in values)+'};\n'
    for name,data in [('operands.bin',blob),('operands.h',header.encode()),('operand_counts.rs',f'const BASE_COUNT:usize={len(base)};\nconst DENSE_COUNT:usize={len(dense)};\n'.encode())]:
        path=SOURCE/name
        if '--check'in sys.argv:assert path.read_bytes()==data,'Changed SANE fixture '+name
        elif not path.exists() or path.read_bytes()!=data:path.write_bytes(data)
    print('Owned SANE operands:',len(base),'base and',len(dense),'dense')
if __name__=='__main__':main()
