#!/usr/bin/env python3
"""Extract only our owned Rez resources into a portable Rust-test fixture."""
from pathlib import Path
import struct
ROOT=Path(__file__).resolve().parents[1]
def resources(blob):
    data,map_at,data_len,map_len=struct.unpack_from('>4I',blob)
    assert data+data_len<=len(blob) and map_at+map_len<=len(blob)
    word=lambda p:struct.unpack_from('>H',blob,p)[0]
    types=map_at+word(map_at+24);result={}
    for i in range(word(types)+1):
        at=types+2+8*i;kind=blob[at:at+4];refs=types+word(at+6)
        for j in range(word(at+4)+1):
            ref=refs+12*j;rid=word(ref);off=data+int.from_bytes(blob[ref+5:ref+8],'big');n=struct.unpack_from('>I',blob,off)[0]
            assert off+4+n<=data+data_len
            result[(kind,rid)]=blob[off+4:off+4+n]
    return result

def main():
    binary=(ROOT/'work/probes/alerts/alerts.r.rsrc.bin').read_bytes()
    data_size,resource_size=struct.unpack_from('>II',binary,83)
    assert data_size==0 and 0<resource_size<=len(binary)-128
    fixture=binary[128:128+resource_size];owned=resources(fixture)
    assert set(owned)=={(b'ALRT',n) for n in range(128,132)}|{(b'DITL',128)}
    installed=resources((ROOT/'work/probes/alerts/AlertProbe.APPL/..namedfork/rsrc').read_bytes())
    assert all(installed[key]==payload for key,payload in owned.items())
    (ROOT/'probes/alerts/resources.rsrc').write_bytes(fixture)
    print('Portable owned alert resources: '+str(len(fixture))+' bytes; native probe payloads match')
if __name__=='__main__':main()
