#!/usr/bin/env python3
"""Prepare a fresh OS 9 boot disk with an application in Startup Items.
Operates on a NEW staging directory. Never mounts QEMU's live working disk.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORIGINAL = ROOT / 'work/os9/base/Mac OS 9.2.1.utm/Images/disk-0.qcow2'
def run(*args):
    subprocess.run([str(a) for a in args], check=True)

def support_resource(path):
    try:return Path(str(path)+'/..namedfork/rsrc').read_bytes()
    except FileNotFoundError:return b''

def finder_info(path):
    return bytes.fromhex(subprocess.check_output(['xattr','-px','com.apple.FinderInfo',str(path)],text=True))

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('resource_fork',type=Path)
    p.add_argument('destination',type=Path)
    p.add_argument('--base-image',type=Path,default=ORIGINAL,help='read-only source OS 9 disk; may be a verified local cache')
    p.add_argument('--name',default='TeachText')
    p.add_argument('--creator',default='ttxt')
    p.add_argument('--data-fork',type=Path,help='original application data fork; omitted means empty')
    p.add_argument('--support-folder',type=Path,help='copy this folder with both forks to the fresh volume as Reference Files')
    p.add_argument('--control-panel',type=Path,action='append',default=[],help='install an original cdev file with both forks into the fresh System Folder')
    args=p.parse_args()
    original=args.base_image.resolve()
    if not original.is_file():p.error('base image must be an existing file')
    if not args.name or any(c in args.name for c in '/:') or len(args.name)>31:
        p.error('name must be a Macintosh filename of at most 31 characters')
    creator=args.creator.encode('ascii')
    if len(creator)!=4:p.error('creator must be four ASCII characters')
    if args.support_folder and not args.support_folder.is_dir():p.error('support folder must be a directory')
    panels=[]
    for panel in args.control_panel:
        if not panel.is_file() or not panel.name or len(panel.name)>31 or any(c in panel.name for c in '/:'):
            p.error('control panel must be a file with a valid Macintosh name')
        info=finder_info(panel)
        if len(info)!=32 or info[:4]!=b'cdev' or not support_resource(panel):
            p.error('control panel requires original cdev Finder information and a nonempty resource fork')
        panels.append({'path':str(panel),'name':panel.name,'finder_info':info.hex(),
            'data_sha256':hashlib.sha256(panel.read_bytes()).hexdigest(),
            'resource_sha256':hashlib.sha256(support_resource(panel)).hexdigest()})
    resource=args.resource_fork.read_bytes()
    data=args.data_fork.read_bytes() if args.data_fork else b''
    if not resource:
        p.error('resource fork is empty; for a native .APPL use PATH/..namedfork/rsrc')
    # Validate the resource-fork header before constructing an expensive disk.
    import struct
    if len(resource)<16:
        p.error('resource fork header is truncated')
    data_offset,map_offset,data_length,map_length=struct.unpack_from('>4I',resource)
    if data_offset<16 or map_offset<16 or data_offset+data_length>len(resource) or map_offset+map_length>len(resource) or map_length<28:
        p.error('invalid resource fork ranges')
    destination=args.destination.resolve()
    destination.mkdir(parents=True,exist_ok=False)
    raw=destination/'staging.raw'
    mount=destination/'mount';mount.mkdir()
    run('qemu-img','convert','-U','-O','raw',original,raw)
    attached=False
    try:
        run('hdiutil','attach','-imagekey','diskimage-class=CRawDiskImage','-mountpoint',mount,raw)
        attached=True
        startup=mount/'System Folder/Startup Items'
        disabled=mount/'Reference Setup Disabled';disabled.mkdir(exist_ok=True)
        for name in ['AirPort Setup Assistant','Mac OS Setup Assistant']:
            item=startup/name
            if item.exists():item.rename(disabled/name)
        for panel,metadata in zip(args.control_panel,panels):
            target=mount/'System Folder/Control Panels'/panel.name
            if target.exists():raise FileExistsError(target)
            run('ditto','--rsrc','--extattr',panel,target)
            if (panel.read_bytes()!=target.read_bytes() or support_resource(panel)!=support_resource(target)
                    or finder_info(target).hex()!=metadata['finder_info']):
                raise RuntimeError(f'control-panel verification failed: {panel.name}')
        if args.support_folder:
            if not args.support_folder.is_dir():raise NotADirectoryError(args.support_folder)
            support=mount/'Reference Files'
            if support.exists():raise FileExistsError(support)
            run('ditto','--rsrc','--extattr',args.support_folder,support)
            for source in args.support_folder.rglob('*'):
                if source.is_file():
                    target=support/source.relative_to(args.support_folder)
                    if source.read_bytes()!=target.read_bytes() or support_resource(source)!=support_resource(target):
                        raise RuntimeError(f'support-fork verification failed: {source.name}')
        app=startup/args.name
        app.write_bytes(data)
        Path(str(app)+'/..namedfork/rsrc').write_bytes(resource)
        finder=b'APPL'+creator+bytes(24)
        run('xattr','-wx','com.apple.FinderInfo',finder.hex(),app)
        if app.read_bytes()!=data:raise RuntimeError('data-fork verification failed')
        if Path(str(app)+'/..namedfork/rsrc').read_bytes()!=resource:
            raise RuntimeError('resource-fork verification failed')
    finally:
        if attached:run('hdiutil','detach',mount)
    base=destination/'base.qcow2';working=destination/'working.qcow2'
    run('qemu-img','convert','-O','qcow2',raw,base)
    run('qemu-img','create','-f','qcow2','-F','qcow2','-b',base,working)
    manifest={'original':str(original),'application':args.name,
        'resource_sha256':hashlib.sha256(resource).hexdigest(),
        'data_sha256':hashlib.sha256(data).hexdigest(),'data_bytes':len(data),'creator':args.creator,
        'support_folder':str(args.support_folder) if args.support_folder else None,'control_panels':panels,'base':str(base),'working':str(working),'startup_assistants':'retained in Reference Setup Disabled'}
    (destination/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(working)
if __name__=='__main__':main()
