#!/usr/bin/env python3
"""Verify pinned application/asset forks in a completed, read-only VM snapshot."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def sha(b):return hashlib.sha256(b).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('snapshot',type=Path);p.add_argument('guest_directory',type=Path);p.add_argument('provenance',type=Path);p.add_argument('output',type=Path);args=p.parse_args()
    if args.snapshot.name!='backup.raw' or not args.snapshot.is_file():p.error('expected completed collector backup.raw')
    if args.guest_directory.is_absolute() or '..' in args.guest_directory.parts:p.error('guest directory must be relative')
    if args.output.exists():raise FileExistsError(args.output)
    assets=json.loads(args.provenance.read_text())['assets'];records=[]
    parent=ROOT/'work/reference-inspection';parent.mkdir(exist_ok=True,parents=True)
    with tempfile.TemporaryDirectory(dir=parent) as temp:
        mount=Path(temp);attached=False
        try:
            subprocess.run(['hdiutil','attach','-readonly','-imagekey','diskimage-class=CRawDiskImage','-mountpoint',str(mount),str(args.snapshot.resolve())],check=True);attached=True
            for asset in assets:
                name=asset['name']
                if Path(name).name!=name or name in ['.','..']:raise ValueError('asset must be a single filename')
                file=mount/args.guest_directory/name;data=file.read_bytes();resource=Path(str(file)+'/..namedfork/rsrc').read_bytes()
                for fork,value in [('data',data),('resource',resource)]:
                    if len(value)!=asset[f'{fork}_bytes'] or sha(value)!=asset[f'{fork}_sha256']:raise RuntimeError(f'{name}: {fork} differs from pin')
                finder=bytes.fromhex(subprocess.check_output(['xattr','-px','com.apple.FinderInfo',str(file)],text=True))
                records.append({**asset,'finder_info':list(finder),'both_forks_equal_pin':True})
        finally:
            if attached:subprocess.run(['hdiutil','detach',str(mount)],check=True)
    args.output.parent.mkdir(exist_ok=True,parents=True);args.output.write_text(json.dumps({'snapshot':str(args.snapshot),'guest_directory':str(args.guest_directory),'assets':records,'all_forks_match_pins':True,'scope':'Completed snapshot mounted read-only, then detached. App execution is evidenced separately by the launch result and UI captures.'},indent=2)+'\n');print(args.output)
if __name__=='__main__':main()
