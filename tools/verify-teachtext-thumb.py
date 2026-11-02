#!/usr/bin/env python3
"""Run the original TeachText Open/down-thumb/up-thumb/unchanged workflow on an owned long text file."""
import hashlib,json,re,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    app=ROOT/'work/apps/TeachText/TeachText.rsrc';assert sha(app)=='b32d7a0ffe50f6a7b9fd62a7702b916cbb82443e57278d0f8fe745529bd2f31a'
    script=ROOT/'probes/teachtext/thumb-document.json';session=Path(tempfile.mkdtemp(prefix='teachtext-thumb-',dir=ROOT/'work/app-validation'));volume=session/'volume';name=b'scroll-test.txt';folder=volume/'files'/name.hex();folder.mkdir(parents=True)
    text=b''.join(f'Line {i:02}: the classic scrolling reference document.\r'.encode() for i in range(1,81))
    (folder/'data').write_bytes(text);(folder/'resource').write_bytes(b'');(volume/'catalog.json').write_text(json.dumps({'format':'ClassicRuntimeVolume','version':1,'files':[{'name':list(name),'finder':list(b'TEXTttxt'+bytes(8))}]}))
    docs=OUT/'TeachText-thumb-documents.json';docs.unlink(missing_ok=True);ppm=session/'thumb.ppm'
    result=subprocess.run(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--trace','--event-script',str(script),'--instruction-limit','250000','--volume',str(volume),'--dump-documents',str(docs),'--capture-framebuffer',str(ppm)],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (OUT/'TeachText-thumb-trace.txt').write_text(result.stdout)
    assert result.returncode==1 and result.stdout.strip().splitlines()[-1].startswith('instruction limit reached'),result.stdout[-1500:]
    state=json.loads(docs.read_text())['documents'];assert len(state)==1 and bytes(state[0]['text_bytes'])==text and state[0]['selection']==[0,0]
    assert state[0]['dest_rect']==[0,4,420,552] and state[0]['view_rect']==[0,4,420,552],'Original scroll geometry differs'
    calls=re.findall(r'control tracking: handle=0x[0-9a-f]+ result=(\d+) value=(\d+) hilite=(\d+)',result.stdout)
    assert calls==[('129','4','0'),('129','1','0'),('0','1','0')],calls
    assert re.search(r'^callback: enter 0x[0-9a-f]+, token=5(?: |$)',result.stdout,re.M) is None,'Nil indicator action unexpectedly invoked a control action'
    subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/'TeachText-thumb-runtime.png')],check=True,capture_output=True)
    report={'unmodified_application':True,'application_resource_sha256':sha(app),'script_sha256':sha(script),'text_sha256':hashlib.sha256(text).hexdigest(),'text_bytes':len(text),'original_control_action_callbacks':0,'tracked_parts':[129,129,0],'control_values':[4,1,1],'dest_rect':state[0]['dest_rect'],'view_rect':state[0]['view_rect'],'document_and_selection_preserved':True,'returned_to_bounded_event_loop':True,'scope':'Original Open, down20px/up20px/unchanged indicator gestures on80 lines. Public value changes on release, exact document geometry and suspended-trap completion; native semantics and actual hosted interaction are separate evidence.','goal_complete':False}
    (OUT/'TeachText-thumb-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
