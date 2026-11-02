#!/usr/bin/env python3
"""Owned PICT2 indexed/monochrome bitmap streams from Apple's published layout."""
from pathlib import Path
import struct,sys
ROOT=Path(__file__).resolve().parents[1];DEST=ROOT/'probes/pictbitmap'
def u16(x):return struct.pack('>H',x)
def rect(t,l,b,r):return struct.pack('>4h',t,l,b,r)
def pack(row):
    out=bytearray();i=0
    while i<len(row):
        run=1
        while i+run<len(row)and row[i+run]==row[i]and run<128:run+=1
        if run>=3:out.extend([257-run,row[i]]);i+=run;continue
        start=i;i+=run
        while i<len(row)and i-start<128:
            run=1
            while i+run<len(row)and row[i+run]==row[i]and run<128:run+=1
            if run>=3:break
            i+=min(run,128-(i-start))
        out.append(i-start-1);out.extend(row[start:i])
    return bytes(out)
def main():
    results=[]
    for kind in range(4):
        width=260 if kind==1 else (4 if kind==2 else 16);height=8;mono=kind==3;rowbytes=(width+7)//8 if mono else width
        frame=rect(0,0,height,width)
        b=bytearray(b'\0\0'+frame+bytes.fromhex('001102ff0c00')+struct.pack('>hHII',-2,0,72<<16,72<<16)+frame+bytes(4))
        b+=u16(1)+u16(10)+frame
        b+=u16(0x90 if kind==2 else 0x98)+u16(rowbytes|(0 if mono else 0x8000))+frame
        if not mono:
            b+=struct.pack('>HHIIIHHHHIII',0,0,0,72<<16,72<<16,0,8,1,8,0,0,0)
            b+=struct.pack('>IHH',0x10203040,0,7)
            colors=[(65535,65535,65535),(0,0,0),(65535,0,0),(0,65535,0),(0,0,65535),(0,65535,65535),(65535,0,65535),(65535,65535,0)]
            for i,rgb in enumerate(colors):b+=struct.pack('>4H',i,*rgb)
        b+=frame+frame+u16(0)
        pixels=[]
        for y in range(height):
            row=bytes([0xaa if y%2==0 else 0x33]*rowbytes)if mono else bytes((x//3+y)%8 for x in range(width));pixels.append(row)
            if kind==2 or rowbytes<8:b+=row
            else:
                encoded=pack(row);b+=u16(len(encoded))if rowbytes>250 else bytes([len(encoded)]);b+=encoded
        if len(b)%2:b.append(0)
        b+=u16(255);b[:2]=u16(len(b)&65535);results.append(bytes(b))
    header='/* Owned PICT2 fixtures, generated from the public picture opcode format. */\n'
    for i,b in enumerate(results):header+=f'static const unsigned char pict_{i}[]={{'+','.join(str(v)for v in b)+'};\n'
    header+='static const unsigned char *pictures[]={pict_0,pict_1,pict_2,pict_3};\nstatic const unsigned long picture_sizes[]={'+','.join(str(len(b))for b in results)+'};\n'
    for name,value in [(f'pict-{i}.bin',b)for i,b in enumerate(results)]+[('fixtures.h',header.encode())]:
        p=DEST/name
        if '--check'in sys.argv:assert p.read_bytes()==value,'Changed picture fixture '+name
        elif not p.exists()or p.read_bytes()!=value:p.write_bytes(value)
    print('Owned PICT2 fixture sizes:',[len(b)for b in results])
if __name__=='__main__':main()
