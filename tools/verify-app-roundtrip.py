#!/usr/bin/env python3
"""Drive original TeachText Save/Close/Open, then reopen in a fresh CPU process."""
import hashlib
import json
import re
from pathlib import Path
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def execute(script,volume,label):
    documents=OUT/f'TeachText-{label}-documents.json'
    documents.unlink(missing_ok=True)
    ppm=volume.parent/f'{label}.ppm'
    command=['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(ROOT/'work/apps/TeachText/TeachText.rsrc'),'--trace','--trace-files','--event-script',str(ROOT/'probes/teachtext'/script),'--volume',str(volume),'--dump-documents',str(documents),'--capture-framebuffer',str(ppm)]
    run=subprocess.run(command,cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (OUT/f'TeachText-{label}-trace.txt').write_text(run.stdout)
    if run.returncode!=1 or not run.stdout.strip().splitlines()[-1].startswith('instruction limit reached'):
        raise RuntimeError(f'{label} did not reach the bounded event loop:\n{run.stdout[-2000:]}')
    count=run.stdout.count('script event:')+run.stdout.count('Standard File input:')
    expected_count=len(json.loads((ROOT/'probes/teachtext'/script).read_text())['events'])
    if count!=expected_count:raise RuntimeError(f'{label}: delivered {count}/{expected_count} events')
    state=json.loads(documents.read_text())
    subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/f'TeachText-{label}-runtime.png')],check=True,capture_output=True)
    return run.stdout,state,count
def text_matches(state,expected):
    return len(state['documents'])==1 and bytes(state['documents'][0]['text_bytes'])==expected
def main():
    parent=ROOT/'work/app-validation';parent.mkdir(parents=True,exist_ok=True)
    session=Path(tempfile.mkdtemp(prefix='roundtrip-',dir=parent))
    volume=session/'volume';cancel_volume=session/'cancel-volume'
    expected=(OUT/'TeachText-reference-document.txt').read_bytes()
    application=ROOT/'work/apps/TeachText/TeachText.rsrc'
    reference=json.loads((OUT/'TeachText-reference-result.json').read_text())
    if sha(application)!=reference['resource_sha256']:raise RuntimeError('Application differs from OS 9 reference binary')
    trace,roundtrip,count=execute('save-close-open.json',volume,'roundtrip')
    catalog=json.loads((volume/'catalog.json').read_text())
    if len(catalog['files'])!=1:raise RuntimeError('Save did not create exactly one file')
    entry=catalog['files'][0];name=bytes(entry['name']);directory=volume/'files'/name.decode('mac_roman').lower().encode('utf-8').hex()
    data=(directory/'data').read_bytes();resource=(directory/'resource').read_bytes()
    info=bytes(entry['finder'])
    if name!=b'reference.txt' or info[:8]!=b'TEXTttxt' or data!=expected or resource:
        raise RuntimeError('Saved name, metadata, data or resource fork differs from expected plain text file')
    if not text_matches(roundtrip,expected):raise RuntimeError('Same-process Open did not load the saved text')
    lines=trace.splitlines();close_key=next(i for i,line in enumerate(lines) if 'script event:' in line and 'message=0xd77 ' in line)
    open_key=next(i for i,line in enumerate(lines) if i>close_key and 'script event:' in line and 'message=0x1f6f ' in line)
    if not any('file PB trap=0xa001 error=0' in line for line in lines[close_key:open_key]):raise RuntimeError('Original Close command did not close a file reference')
    # This pinned original's save hook is CODE1+0x488. Segment resources now
    # live in movable heap handles; derive its address from actual load records.
    bases={int(base,16) for base in re.findall(r'loaded CODE 1: \d+ bytes at (0x[0-9a-f]+)',trace)}
    if not bases:raise RuntimeError('No original CODE1 execution record')
    callbacks=sum(trace.count(f'callback: enter {base+0x488:#x}, token=1') for base in bases)
    if callbacks==0:raise RuntimeError('Original application save hook did not execute')
    _,fresh,fresh_count=execute('reopen-document.json',volume,'reopened')
    if not text_matches(fresh,expected):raise RuntimeError('Fresh process did not load the persisted text')
    cancel_trace,cancel,cancel_count=execute('cancel-save.json',cancel_volume,'cancelled')
    if json.loads((cancel_volume/'catalog.json').read_text())['files'] or not text_matches(cancel,expected) or 'accepted=false' not in cancel_trace:
        raise RuntimeError('Cancel changed the document or created a file')
    (OUT/'TeachText-persisted-document.txt').write_bytes(data)
    report={'application':'TeachText 7.0','resource_sha256':sha(application),'matches_reference_binary':True,
            'saved_filename':name.decode('mac_roman'),'saved_data_bytes':len(data),'saved_data_sha256':hashlib.sha256(data).hexdigest(),
            'saved_bytes_equal_os9_reference':True,'same_process_save_close_open':True,'fresh_process_reopen':True,'cancel_preserves_text_and_creates_no_file':True,
            'original_save_hook_executions':callbacks,'delivered_events':{'roundtrip':count,'fresh_open':fresh_count,'cancel':cancel_count},
            'volume':str(volume),'interface':'scripted classic keyboard events; native mouse/keyboard frontend verified separately',
            'scope':'New plain-text file in one root directory. Replacement and native input workflows are verified separately. Directory navigation, custom filters and full dialog semantics remain incomplete.',
            'full_goal_complete':False}
    (OUT/'TeachText-roundtrip-result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':
    try:main()
    except Exception as error:
        (OUT/'TeachText-roundtrip-result.json').write_text(json.dumps({'verification_error':str(error),'full_goal_complete':False},indent=2)+'\n')
        raise
