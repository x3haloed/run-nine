#!/usr/bin/env python3
"""Prune generated probe disks, preserving source images and recorded evidence.

The retained directories are intentional working environments. Every other
candidate must be a recognized prepare-reference stage or collect-reference
backup. Only generated image filenames are removed; manifests and captures stay.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import subprocess
import time

ROOT=Path(__file__).resolve().parents[1]
DISKS=ROOT/'work/os9'
QEMU='/opt/homebrew/bin/qemu-img'
KEEP_DIRS={
    'base','media','run','reference-app',
    'marathon-full-reference-v1','simpletext-reference-v1',
    'teachtext-ordinary-reference-v1','teachtext-mousekeys-reference-v1',
    'soundpcm-screamer-v1','soundguest-screamer-v1',
    'soundcommands-screamer-v3','sounddiscovery-screamer-v1',
    'soundamplitude-screamer-v1',
}
KEEP_FILES={
    'soundguest-screamer-collection-v1/backup.qcow2',
    'soundcommands-screamer-collection-v3/backup.qcow2',
    'sounddiscovery-screamer-collection-v2/backup.qcow2',
    'soundamplitude-collection-v1/backup.qcow2',
}
GENERATED={'base.qcow2','working.qcow2','repeat-working.qcow2',
           'working-cocoa.qcow2','staging.raw','backup.qcow2','backup.raw'}

def free_bytes():
    s=os.statvfs(ROOT);return s.f_bavail*s.f_frsize

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply',action='store_true')
    parser.add_argument('--report',type=Path,required=True)
    args=parser.parse_args()
    # Lifecycle checks cover both ordinary QEMU and renamed reference bundles.
    commands=subprocess.check_output(['ps','-axo','pid=,command='],text=True)
    live=[line.strip()for line in commands.splitlines()
          if str(DISKS) in line and ('-drive ' in line or '-hda ' in line)]
    if live:raise RuntimeError('Reference image still used by an emulator: '+str(live))
    mounted=plistlib.loads(subprocess.check_output(['hdiutil','info','-plist']))
    active={Path(i['image-path']).resolve()for i in mounted.get('images',[])if i.get('image-path')}
    protected={DISKS/p for p in KEEP_FILES}
    candidates=[];skipped=[]
    for directory in sorted(DISKS.iterdir()):
        if not directory.is_dir() or directory.is_symlink():continue
        if directory.name in KEEP_DIRS:
            protected.update(p for p in directory.iterdir()if p.is_file() and p.name in GENERATED)
            continue
        # Content reads can hydrate old iCloud placeholders. The generator's
        # manifest + fixed image names identify its directories without that.
        stage=((directory/'manifest.json').is_file() and
               ((directory/'base.qcow2').is_file() or (directory/'working.qcow2').is_file()))
        collection=((directory/'mount').is_dir() and
                    ('collection' in directory.name or
                     ((directory/'backup.qcow2').is_file() and (directory/'backup.raw').is_file())))
        for path in sorted(directory.iterdir()):
            if path.name not in GENERATED or not path.is_file() or path.is_symlink():continue
            if path in protected:continue
            recognized=(stage and path.name in GENERATED-{'backup.qcow2','backup.raw'}) or (collection and path.name in {'backup.qcow2','backup.raw'})
            if recognized:candidates.append(path)
            else:skipped.append(str(path.relative_to(ROOT)))
    # Keep every backing dependency of retained working images. Original source
    # directories are protected wholesale and need no materialization to inspect.
    pending=[p for p in protected if p.suffix=='.qcow2' and p.exists()]
    seen=set()
    while pending:
        p=pending.pop().resolve()
        if p in seen or p.is_relative_to((DISKS/'base').resolve()):continue
        seen.add(p)
        info=json.loads(subprocess.check_output([QEMU,'info','-U','--output=json',str(p)],text=True))
        backing=info.get('full-backing-filename') or info.get('backing-filename')
        if backing:
            target=Path(backing)
            if not target.is_absolute():target=p.parent/target
            target=target.resolve()
            if target.is_relative_to(DISKS.resolve()):
                protected.add(target);pending.append(target)
    rows=[]
    for p in candidates:
        if p.resolve() in protected or p.resolve() in active:continue
        stat=p.stat()
        rows.append({'path':str(p.relative_to(ROOT)),'logical_bytes':stat.st_size,
                     'allocated_bytes':stat.st_blocks*512,'removed':False})
    report={'scope':'Generated offline reference disks only; original sources, useful working references, manifests, probe binaries and outputs preserved',
            'applied':args.apply,'free_bytes_before':free_bytes(),
            'kept_directories':sorted(KEEP_DIRS),'kept_files':sorted(KEEP_FILES),
            'backing_dependencies':sorted(str(p.relative_to(ROOT))for p in protected if p.is_relative_to(ROOT)),
            'skipped_unrecognized':skipped,'files':rows,
            'reproduction':'Use prepare-reference.py with the original base and owned probe in a fresh staging directory; collect-reference.py creates fresh backups. Existing manifests remain as provenance.'}
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2)+'\n')
    print(f"Selected {len(rows)} generated images; allocated size {sum(r['allocated_bytes']for r in rows)/2**30:.1f}GiB",flush=True)
    if args.apply:
        for row in rows:
            (ROOT/row['path']).unlink();row['removed']=True
        report['free_bytes_after']=free_bytes()
        report['removed_allocated_bytes']=sum(r['allocated_bytes']for r in rows)
        args.report.write_text(json.dumps(report,indent=2)+'\n')
        print(f"Removed {len(rows)} images; available space now {report['free_bytes_after']/2**30:.1f}GiB",flush=True)

if __name__=='__main__':main()
