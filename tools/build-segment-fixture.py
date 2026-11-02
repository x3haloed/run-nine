#!/usr/bin/env python3
"""Add our tiny MPW CODE2 fixture to our own single-segment probe build.
Never edits a third-party application. Does not copy a system implementation.
"""
from pathlib import Path
import struct
ROOT=Path(__file__).resolve().parents[1]
def u16(b,p):return struct.unpack_from('>H',b,p)[0]
def u32(b,p):return struct.unpack_from('>I',b,p)[0]
def main():
    source=ROOT/'work/probes/segments/SegmentProbe.APPL/..namedfork/rsrc'
    blob=source.read_bytes();data,map_at,_,_=struct.unpack_from('>4I',blob)
    types=map_at+u16(blob,map_at+24);names=map_at+u16(blob,map_at+26)
    resources=[]
    for i in range(u16(blob,types)+1):
        at=types+2+i*8;kind=blob[at:at+4];refs=types+u16(blob,at+6)
        for j in range(u16(blob,at+4)+1):
            ref=refs+j*12;rid=struct.unpack_from('>h',blob,ref)[0];n=u16(blob,ref+2)
            name=None if n==65535 else blob[names+n+1:names+n+1+blob[names+n]]
            off=data+int.from_bytes(blob[ref+5:ref+8],'big');size=u32(blob,off)
            resources.append([kind,rid,blob[ref+4],name,blob[off+4:off+4+size]])
    if sorted(r[1] for r in resources if r[0]==b'CODE')!=[0,1]:
        raise RuntimeError('Expected our single-segment build, without patched segment traps')
    code0=next(r for r in resources if r[:2]==[b'CODE',0])
    above,below,length,offset=struct.unpack_from('>4I',code0[4])
    if len(code0[4])!=16+length or length%8 or length+16>65535:
        raise RuntimeError('Unexpected CODE0 layout')
    entries=struct.pack('>8H',0,0x3f3c,2,0xa9f0,4,0x3f3c,2,0xa9f0)
    code0[4]=struct.pack('>4I',max(above,offset+length+16),below,length+16,offset)+code0[4][16:]+entries
    # MPW header: jump-table offset and count; moveq #7,d0;rts;moveq #11,d0;rts.
    resources.append([b'CODE',2,0,None,struct.pack('>6H',length,2,0x7007,0x4e75,0x700b,0x4e75)])
    groups={};data_bytes=bytearray();name_bytes=bytearray()
    for kind,rid,attrs,name,payload in resources:
        data_off=len(data_bytes);data_bytes+=struct.pack('>I',len(payload))+payload
        name_off=65535
        if name is not None:
            name_off=len(name_bytes);name_bytes+=bytes([len(name)])+name
        ref=struct.pack('>hH',rid,name_off)+bytes([attrs])+data_off.to_bytes(3,'big')+bytes(4)
        groups.setdefault(kind,[]).append(ref)
    type_bytes=bytearray(struct.pack('>H',len(groups)-1));ref_bytes=bytearray()
    for kind,refs in groups.items():
        type_bytes+=kind+struct.pack('>HH',len(refs)-1,2+len(groups)*8+len(ref_bytes))
        ref_bytes+=b''.join(refs)
    index=type_bytes+ref_bytes
    map_bytes=bytearray(24)+struct.pack('>HH',28,28+len(index))+index+name_bytes
    header=struct.pack('>4I',256,256+len(data_bytes),len(data_bytes),len(map_bytes))
    map_bytes[:16]=header
    destination=ROOT/'work/probes/segments/SegmentProbe.rsrc'
    destination.write_bytes(header+bytes(240)+data_bytes+map_bytes)
    print(destination)
if __name__=='__main__':main()
