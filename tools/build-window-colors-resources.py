#!/usr/bin/env python3
"""Extract only owned WIND/wctb resources for portable independent replay."""
from pathlib import Path
import struct,runpy
ROOT=Path(__file__).resolve().parents[1]
def main():
    parse=runpy.run_path(str(ROOT/'tools/build-alert-resources.py'),run_name='owned_resource_parser')['resources']
    binary=(ROOT/'work/probes/windowcolors/fixtures.r.rsrc.bin').read_bytes();data_size,resource_size=struct.unpack_from('>II',binary,83)
    assert data_size==0 and 0<resource_size<=len(binary)-128
    fixture=binary[128:128+resource_size];owned=parse(fixture)
    assert set(owned)=={(b'WIND',n) for n in range(22000,22003)}|{(b'wctb',n) for n in range(22001,22004)}
    native=parse((ROOT/'work/probes/windowcolors/WindowColorsProbe.APPL/..namedfork/rsrc').read_bytes())
    assert all(native[key]==payload for key,payload in owned.items())
    (ROOT/'probes/windowcolors/resources.rsrc').write_bytes(fixture)
    print('Portable owned window color resources: '+str(len(fixture))+' bytes, matching native application payloads')
if __name__=='__main__':main()
