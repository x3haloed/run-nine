#!/usr/bin/env python3
"""Verify original TeachText Edit commands, including replacement and Cut/Paste."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    app=ROOT/'work/apps/TeachText/TeachText.rsrc'
    reference=json.loads((OUT/'TeachText-reference-result.json').read_text())
    if sha(app)!=reference['resource_sha256']:raise RuntimeError('Original application identity mismatch')
    baseline=(OUT/'TeachText-reference-document.txt').read_bytes()
    cases=[]
    for script,expected in [('copy-paste',baseline*2),('cut-document',b''),('cut-paste',baseline*2)]:
        source=ROOT/'probes/teachtext'/f'{script}.json'
        snapshot=OUT/f'TeachText-{script}-documents.json';snapshot.unlink(missing_ok=True)
        ppm=ROOT/'work'/f'TeachText-{script}.ppm'
        run=subprocess.run(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--trace','--trace-scrap','--event-script',str(source),'--dump-documents',str(snapshot),'--capture-framebuffer',str(ppm)],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (OUT/f'TeachText-{script}-trace.txt').write_text(run.stdout)
        if run.returncode!=1 or not run.stdout.strip().splitlines()[-1].startswith('instruction limit reached'):raise RuntimeError(f'{script}: unexpected execution stop: {run.stdout[-1000:]}')
        if run.stdout.count('script event:')!=len(json.loads(source.read_text())['events']):raise RuntimeError(f'{script}: not all events delivered')
        docs=json.loads(snapshot.read_text())['documents']
        if len(docs)!=1 or bytes(docs[0]['text_bytes'])!=expected:raise RuntimeError(f'{script}: actual document differs')
        traps=re.findall(r'clipboard trap=(0x[0-9a-f]+)',run.stdout)
        required=['0xa9d5','0xa9db','0xa9c2'] + (['0xa9d6'] if script!='copy-paste' else [])
        if any(t not in traps for t in required):raise RuntimeError(f'{script}: original edit code did not call required services')
        subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/f'TeachText-{script}-runtime.png')],check=True,capture_output=True)
        cases.append({'script':str(source.relative_to(ROOT)),'script_sha256':sha(source),'events_delivered':run.stdout.count('script event:'),'text_bytes':list(expected),'text_sha256':hashlib.sha256(expected).hexdigest(),'clipboard_trap_counts':{t:traps.count(t) for t in sorted(set(traps))}})
    result={'application':'TeachText 7.0','resource_sha256':sha(app),'unmodified_reference_binary':True,'cases':cases,'interface':'Scripted classic key events processed by original app Edit menu commands','scope':'Plain text, in-process classic clipboard. Actual hosted input, cross-document transfer and host clipboard synchronization are not verified by this check.','goal_complete':False}
    (OUT/'TeachText-clipboard-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':
    try:main()
    except Exception as e:
        (OUT/'TeachText-clipboard-result.json').write_text(json.dumps({'verification_error':str(e),'goal_complete':False},indent=2)+'\n');raise
