#!/usr/bin/env python3
"""Import explicitly pinned original sibling forks into a NEW logical source volume."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('source',type=Path);p.add_argument('provenance',type=Path);p.add_argument('destination',type=Path)
    a=p.parse_args();root=a.source.resolve();pins=json.loads(a.provenance.read_text())['assets'];prepared=[]
    for pin in pins:
        name=pin['name'];encoded=name.encode('mac_roman')
        if not encoded or len(encoded)>31 or ':' in name or '/' in name or '\0' in name:raise ValueError('Invalid flat source leaf')
        path=root/name
        if path.resolve().parent!=root:raise ValueError('Source leaf escapes its directory')
        forks={'data':path.read_bytes()}
        try:forks['resource']=(path/'..namedfork/rsrc').read_bytes()
        except FileNotFoundError:forks['resource']=b''
        for kind,data in forks.items():
            if len(data)!=pin[kind+'_bytes'] or hashlib.sha256(data).hexdigest()!=pin[kind+'_sha256']:raise ValueError('Original fork pin differs: '+name+' '+kind)
            if len(data)>32*1024*1024:raise ValueError('Oversized fork')
        finder=bytes.fromhex(subprocess.check_output(['xattr','-px','com.apple.FinderInfo',str(path)],text=True))[:16]
        if len(finder)!=16:raise ValueError('Missing Finder metadata')
        key=encoded.decode('mac_roman').lower().encode('utf-8').hex()
        prepared.append((encoded,finder,key,forks))
    if len({key for _,_,key,_ in prepared})!=len(prepared):raise ValueError('Duplicate case-folded source leaves')
    a.destination.mkdir(parents=True,exist_ok=False)
    entries=[]
    for name,finder,key,forks in prepared:
        folder=a.destination/'files'/key;folder.mkdir(parents=True)
        for kind,data in forks.items():(folder/kind).write_bytes(data)
        entries.append({'name':list(name),'finder':list(finder)})
    (a.destination/'catalog.json').write_text(json.dumps({'format':'ClassicRuntimeVolume','version':1,'files':entries},indent=2)+'\n')
    print(json.dumps({'files':len(entries),'original_forks_match_pins':True,'destination':str(a.destination)}))
if __name__=='__main__':main()
