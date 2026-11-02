#!/usr/bin/env python3
"""Inspect both forks and identity from an already completed reference snapshot.
The source is mounted read-only. Never pass QEMU's live working disk here.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[1]
def sha(b):return hashlib.sha256(b).hexdigest()
def relative(value):
    p=Path(value)
    if p.is_absolute() or '..' in p.parts:raise argparse.ArgumentTypeError('guest path must be relative')
    return p
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('snapshot',type=Path);p.add_argument('guest_path',type=relative);p.add_argument('output',type=Path)
    p.add_argument('--data-file',required=True,type=Path);p.add_argument('--resource-output',required=True,type=Path)
    p.add_argument('--application',required=True,type=relative);p.add_argument('--application-provenance',required=True,type=Path)
    args=p.parse_args()
    if args.snapshot.name!='backup.raw' or not args.snapshot.is_file():p.error('expected completed collector backup.raw')
    for output in [args.output,args.resource_output]:
        if output.exists():raise FileExistsError(output)
    expected=json.loads(args.application_provenance.read_text())
    parent=ROOT/'work/reference-inspection';parent.mkdir(exist_ok=True,parents=True)
    with tempfile.TemporaryDirectory(dir=parent) as temp:
        mount=Path(temp);attached=False
        try:
            subprocess.run(['hdiutil','attach','-readonly','-imagekey','diskimage-class=CRawDiskImage','-mountpoint',str(mount),str(args.snapshot.resolve())],check=True)
            attached=True;document=mount/args.guest_path;application=mount/args.application
            data=document.read_bytes();resource=Path(str(document)+'/..namedfork/rsrc').read_bytes()
            app_resource=Path(str(application)+'/..namedfork/rsrc').read_bytes();app_data=application.read_bytes()
            if sha(app_resource)!=expected['resource_sha256'] or sha(app_data)!=expected['data_sha256']:raise RuntimeError('Original application identity mismatch in snapshot')
            if args.data_file.read_bytes()!=data:raise RuntimeError('Collected document differs from snapshot')
            finder=bytes.fromhex(subprocess.check_output(['xattr','-px','com.apple.FinderInfo',str(document)],text=True))
            if len(finder)!=32:raise RuntimeError('Unexpected Finder info length')
            result={'guest_path':str(args.guest_path),'data_bytes':len(data),'data_sha256':sha(data),'resource_bytes':len(resource),'resource_sha256':sha(resource),'finder_info':list(finder),'application_guest_path':str(args.application),'application_resource_sha256':sha(app_resource),'application_data_sha256':sha(app_data),'snapshot':str(args.snapshot),'collection':'Completed full drive backup, read-only offline mount; no live disk mount'}
        finally:
            if attached:subprocess.run(['hdiutil','detach',str(mount)],check=True)
    args.resource_output.parent.mkdir(exist_ok=True,parents=True);args.resource_output.write_bytes(resource)
    args.output.parent.mkdir(exist_ok=True,parents=True);args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(args.output)
if __name__=='__main__':main()
