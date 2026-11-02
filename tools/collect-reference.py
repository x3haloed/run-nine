#!/usr/bin/env python3
"""Collect a saved guest file through a full live-disk backup and read-only mount."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import time
from reference import QMP

def run(*args):subprocess.run([str(a) for a in args],check=True)
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('guest_path')
    p.add_argument('destination',type=Path)
    p.add_argument('output',type=Path)
    args=p.parse_args()
    relative=Path(args.guest_path)
    if relative.is_absolute() or '..' in relative.parts:p.error('guest_path must stay within the guest volume')
    dest=args.destination.resolve();dest.mkdir(parents=True,exist_ok=False)
    backup=dest/'backup.qcow2';raw=dest/'backup.raw';mount=dest/'mount';mount.mkdir()
    job='collect-'+str(time.time_ns())
    with QMP() as q:
        q.command('drive-backup',{'device':'ide0-hd0','target':str(backup),'format':'qcow2','sync':'full','job-id':job})
        completed=None
        while completed is None:
            for event in q.events:
                if event.get('data',{}).get('device')==job:
                    if event['event']=='BLOCK_JOB_COMPLETED':completed=event
                    if event['event'] in ('BLOCK_JOB_ERROR','BLOCK_JOB_CANCELLED'):raise RuntimeError(event)
            if completed:break
            event=json.loads(q.stream.readline())
            if 'event' in event:q.events.append(event)
        if completed['data'].get('error'):raise RuntimeError(completed)
    run('qemu-img','check',backup)
    run('qemu-img','convert','-O','raw',backup,raw)
    attached=False
    try:
        run('hdiutil','attach','-readonly','-imagekey','diskimage-class=CRawDiskImage','-mountpoint',mount,raw)
        attached=True
        source=mount/relative
        if not source.is_file():raise FileNotFoundError(f'Guest output not present: {relative}')
        args.output.parent.mkdir(parents=True,exist_ok=True)
        if args.output.exists():raise FileExistsError(args.output)
        shutil.copyfile(source,args.output)
        print(args.output)
    finally:
        if attached:run('hdiutil','detach',mount)
if __name__=='__main__':main()
