#!/usr/bin/env python3
"""Request a read-only SIGUSR1 frame from an owned --gui development process."""
import argparse,hashlib,json,os,signal,shutil,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('pid',type=int);p.add_argument('capture',type=Path);p.add_argument('destination',type=Path);p.add_argument('--delay',type=float,default=0)
a=p.parse_args();meta=Path(str(a.capture)+'.json')
if a.delay:time.sleep(a.delay)
old=meta.read_text()if meta.exists()else None
os.kill(a.pid,0);os.kill(a.pid,signal.SIGUSR1);deadline=time.monotonic()+20
while True:
    try:
        text=meta.read_text();v=json.loads(text)
        if text!=old and v.get('read_only_observation')is True:break
    except (FileNotFoundError,json.JSONDecodeError):pass
    os.kill(a.pid,0)
    if time.monotonic()>deadline:raise TimeoutError('Live process did not acknowledge snapshot; it was not restarted')
    time.sleep(.05)
a.destination.parent.mkdir(parents=True,exist_ok=True)
ppm=Path(str(a.destination)+'.ppm');shutil.copyfile(a.capture,ppm)
v['ppm_sha256']=hashlib.sha256(ppm.read_bytes()).hexdigest();Path(str(a.destination)+'.json').write_text(json.dumps(v,indent=2)+'\n')
subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(a.destination)+'.png'],check=True,stdout=subprocess.DEVNULL)
print(json.dumps(v))
