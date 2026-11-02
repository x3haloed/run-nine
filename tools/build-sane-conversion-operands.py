#!/usr/bin/env python3
"""Own packed dyadic inputs spanning double range and precision boundaries."""
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1];DEST=ROOT/'probes/saneconvert'
def main():
    source=(ROOT/'probes/sane/operands.bin').read_bytes();rows=[source[i:i+10] for i in range(0,len(source),10)]
    rows=rows[:38]+rows[-128:]
    for exponent in [-1075,-1074,-1023,-1022,-1021,1022,1023,1024]:
        for mantissa in [1<<63,(1<<63)+1,(1<<63)+(1<<10),(1<<64)-1]:
            for sign in [0,0x8000]:rows.append((sign|(exponent+16383)).to_bytes(2,'big')+mantissa.to_bytes(8,'big'))
    header='/* Owned generated double-conversion operands. */\n#define CONVERSION_COUNT '+str(len(rows))+'\nstatic const unsigned char conversion_operands[][10] = {\n'+''.join('{'+','.join(str(v)for v in b)+'},\n'for b in rows)+'};\n'
    for name,data in [('operands.h',header.encode()),('operands.bin',b''.join(rows))]:
        path=DEST/name
        if '--check' in sys.argv:assert path.read_bytes()==data,'Changed conversion operand '+name
        elif not path.exists() or path.read_bytes()!=data:path.write_bytes(data)
    print('Owned conversion operands:',len(rows))
if __name__=='__main__':main()
