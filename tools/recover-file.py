#!/usr/bin/env python3
"""Export one completed recovery checkpoint into a NEW logical volume."""
import argparse
import json
from pathlib import Path

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('checkpoint',type=Path)
    p.add_argument('destination',type=Path,help='new directory; existing paths are never overwritten')
    a=p.parse_args()
    manifest=json.loads((a.checkpoint/'manifest.json').read_text())
    if manifest.get('format')!='ClassicRuntimeRecovery' or manifest.get('version')!=1:raise ValueError('Unrecognized recovery checkpoint')
    name=bytes(manifest['name']);finder=bytes(manifest['finder'])
    identity=manifest.get('id')
    if identity is not None and (type(identity) is not int or not 3<=identity<=0xffffffff):raise ValueError('Invalid recovered file identity')
    if not name or len(name)>31 or b'\0' in name or b':' in name or len(finder)!=16:raise ValueError('Invalid Macintosh name or Finder metadata')
    meta=None
    if 'metadata' in manifest:
        meta=manifest['metadata']
        if not isinstance(meta,dict) or set(meta)!={'extended','dates'} or len(meta['extended'])!=16 or len(meta['dates'])!=3 or any(type(v) is not int or not 0<=v<=255 for v in meta['extended']) or any(type(v) is not int or not 0<=v<=0xffffffff for v in meta['dates']):raise ValueError('Invalid recovered catalog metadata')
    forks={k:(a.checkpoint/k).read_bytes() for k in ('data','resource')}
    for k,v in forks.items():
        if len(v)!=manifest[k+'_bytes'] or len(v)>32*1024*1024:raise ValueError('Incomplete or oversized checkpoint fork')
    # Validate every input before creating a destination. Never mutate the
    # original volume or the checkpoint, even when exporting an older version.
    a.destination.mkdir(parents=True,exist_ok=False)
    folder=a.destination/'files'/name.decode('mac_roman').lower().encode('utf-8').hex()
    folder.mkdir(parents=True)
    for k,v in forks.items():(folder/k).write_bytes(v)
    entry={'name':list(name),'finder':list(finder)}
    if identity is not None:entry['id']=identity
    if meta is not None:entry['metadata']=meta
    (a.destination/'catalog.json').write_text(json.dumps({'format':'ClassicRuntimeVolume','version':1,'files':[entry]},indent=2)+'\n')
    print(a.destination)
if __name__=='__main__':main()
