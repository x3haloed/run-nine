#!/usr/bin/env python3
"""Verify original TeachText resizing and text preservation; hosted input is separate evidence."""
import hashlib,json,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    app=ROOT/'work/apps/TeachText/TeachText.rsrc';assert sha(app)=='b32d7a0ffe50f6a7b9fd62a7702b916cbb82443e57278d0f8fe745529bd2f31a'
    script=ROOT/'probes/teachtext/resize-document.json';session=Path(tempfile.mkdtemp(prefix='teachtext-resize-',dir=ROOT/'work/app-validation'));docs=OUT/'TeachText-resized-documents.json';docs.unlink(missing_ok=True);ppm=session/'resized.ppm'
    result=subprocess.run(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--trace','--event-script',str(script),'--instruction-limit','250000','--volume',str(session/'volume'),'--dump-documents',str(docs),'--capture-framebuffer',str(ppm)],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (OUT/'TeachText-resized-trace.txt').write_text(result.stdout)
    assert result.returncode==1 and result.stdout.strip().splitlines()[-1].startswith('instruction limit reached'),result.stdout[-1500:]
    state=json.loads(docs.read_text())['documents'];assert len(state)==1 and bytes(state[0]['text_bytes'])==b'reference text' and state[0]['selection']==[14,14]
    assert 'window tracking: trap=0xa92b' in result.stdout and 'grow_result=Some(21299642)' in result.stdout,'Original grow did not return expected dimensions'
    assert state[0]['view_rect']==[0,4,310,427] and state[0]['dest_rect']==[0,4,310,427],'Edit geometry did not follow the original application resize'
    assert result.stdout.count('script event:')==16,'Unexpected input delivery: pointer move/up must be consumed by tracking'
    subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/'TeachText-resized-runtime.png')],check=True,capture_output=True)
    report={'unmodified_application':True,'application_resource_sha256':sha(app),'script_sha256':sha(script),'keyboard_events':15,'tracking_pointer_events':3,'original_resize_completed':True,'size':[442,325],'document_bytes_preserved':True,'selection_preserved':True,'returned_to_bounded_event_loop':True,'scope':'Scripted original New, typing and size-box drag. Actual native host input is separate evidence. Original scrolling and double-click workflows remain separate gates.','goal_complete':False}
    (OUT/'TeachText-resize-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
