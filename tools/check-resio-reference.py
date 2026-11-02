#!/usr/bin/env python3
"""Audit native resource API observations against retained native raw file bytes.

This is a native-capture consistency audit, not a runtime comparison. It does
not compare volatile in-memory links/handles serialized into native map headers.
"""
import json
from pathlib import Path
import struct
ROOT=Path(__file__).resolve().parents[1]
def parse_fork(data):
    def span(start,length):
        if start<0 or length<0 or start+length>len(data):raise ValueError('Native fork range outside captured bytes')
        return data[start:start+length]
    def word(start):return struct.unpack('>H',span(start,2))[0]
    def long(start):return struct.unpack('>I',span(start,4))[0]
    data_offset,map_offset,data_length,map_length=struct.unpack('>4I',span(0,16))
    span(data_offset,data_length);span(map_offset,map_length)
    types=map_offset+word(map_offset+24);names=map_offset+word(map_offset+26);count=word(types)
    rows=[]
    for i in range(0 if count==65535 else count+1):
        entry=types+2+8*i;kind=long(entry);n=word(entry+4);references=types+word(entry+6)
        for j in range(n+1):
            ref=references+12*j;identifier=struct.unpack('>h',span(ref,2))[0];name_offset=word(ref+2);packed=long(ref+4);offset=data_offset+(packed&0xffffff);length=long(offset)
            name=b'' if name_offset==65535 else span(names+name_offset+1,span(names+name_offset,1)[0])
            rows.append({'id':identifier,'kind':kind,'attributes':packed>>24,'name_hex':name.hex(),'data_hex':span(offset+4,length).hex()})
    return rows

def main():
    api=json.loads((ROOT/'outputs/ResIOProbe-reference.json').read_text())['resources']
    raw=json.loads((ROOT/'outputs/ResIOProbe-raw-reference.json').read_text())
    refs=raw['refs']
    if len(refs)!=8 or any(type(n) is not int for n in refs):raise ValueError('Missing raw native references')
    app,first,second,search_current,repeated,repeat_before,repeat_current,reopened=refs
    if [int(repeated==first),int(repeat_current==second),int(repeat_current==first)] != [api['repeated_open'][0],api['repeated_open'][2],api['repeated_current_is_original']]:raise ValueError('Native repeated-open predicates do not derive from raw references')
    if [int(search_current==first),int(search_current==second)]!=api['second_search_current'] or int(repeat_before==second)!=api['repeated_before_is_second']:raise ValueError('Native search/current-file predicates do not derive from raw references')
    rows=parse_fork(bytes.fromhex(raw['fork_hex']))
    expected={'id':api['read_info'][1],'kind':api['read_info'][2],'attributes':api['read_info'][3],'name_hex':api['name_hex'],'data_hex':api['data_hex']}
    if rows!=[expected]:raise ValueError('Native reopened API data/metadata do not match retained raw resource file')
    print(json.dumps({'native_api_matches_raw_resource':True,'repeated_open_identity_matches_raw_references':True,'resources':len(rows),'runtime_comparison':False,'scope':'Native capture consistency only; volatile map links and handle pointers are retained in raw bytes but not treated as portable resource values.'}))
if __name__=='__main__':main()
