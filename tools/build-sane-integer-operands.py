#!/usr/bin/env python3
"""Owned signed16 raw fixtures for native FI2X conversion."""
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1];DEST=ROOT/'probes/saneinteger'
def main():
    values=[0,1,-1,2,-2,3,-3,7,-7,15,-15,31,-31,127,-128,255,-256,1023,-1024,2047,-2048,4095,-4096,8191,-8192,16383,-16384,32766,32767,-32767,-32768,12345]
    state=0x6ba3
    for i in range(128):state=(state*25173+13849)&65535;values.append(state)
    rows=[(v&65535).to_bytes(2,'big') for v in values]
    header='/* Owned signed16 operands. */\n#define INTEGER_COUNT '+str(len(rows))+'\nstatic const unsigned char integer_operands[][2] = {\n'+''.join('{'+','.join(str(v) for v in b)+'},\n' for b in rows)+'};\n'
    for name,data in [('operands.bin',b''.join(rows)),('operands.h',header.encode())]:
        p=DEST/name
        if '--check' in sys.argv:assert p.read_bytes()==data,'Changed integer operand '+name
        elif not p.exists() or p.read_bytes()!=data:p.write_bytes(data)
    print('Owned signed16 inputs:',len(rows))
if __name__=='__main__':main()
