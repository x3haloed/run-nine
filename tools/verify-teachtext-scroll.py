#!/usr/bin/env python3
"""Run the original TeachText Open/arrow/page/up workflow on an owned long text file."""
import hashlib,json,re,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    app=ROOT/'work/apps/TeachText/TeachText.rsrc';assert sha(app)=='b32d7a0ffe50f6a7b9fd62a7702b916cbb82443e57278d0f8fe745529bd2f31a'
    script=ROOT/'probes/teachtext/scroll-document.json';session=Path(tempfile.mkdtemp(prefix='teachtext-scroll-',dir=ROOT/'work/app-validation'));volume=session/'volume';name=b'scroll-test.txt';folder=volume/'files'/name.hex();folder.mkdir(parents=True)
    text=b''.join(f'Line {i:02}: the classic scrolling reference document.\r'.encode() for i in range(1,81))
    (folder/'data').write_bytes(text);(folder/'resource').write_bytes(b'');(volume/'catalog.json').write_text(json.dumps({'format':'ClassicRuntimeVolume','version':1,'files':[{'name':list(name),'finder':list(b'TEXTttxt'+bytes(8))}]}))
    docs=OUT/'TeachText-scrolled-documents.json';docs.unlink(missing_ok=True);ppm=session/'scrolled.ppm'
    result=subprocess.run(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--trace','--dump-code',str(session/'decoded'),'--event-script',str(script),'--instruction-limit','250000','--volume',str(volume),'--dump-documents',str(docs),'--capture-framebuffer',str(ppm)],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (OUT/'TeachText-scrolled-trace.txt').write_text(result.stdout)
    assert result.returncode==1 and result.stdout.strip().splitlines()[-1].startswith('instruction limit reached'),result.stdout[-1500:]
    state=json.loads(docs.read_text())['documents'];assert len(state)==1 and bytes(state[0]['text_bytes'])==text and state[0]['selection']==[0,0]
    assert state[0]['dest_rect']==[-416,4,4,552] and state[0]['view_rect']==[0,4,420,552],'Original scroll geometry differs'
    calls=re.findall(r'control tracking: handle=0x[0-9a-f]+ result=(\d+) value=(\d+) hilite=(\d+)',result.stdout)
    assert calls==[('21','2','0'),('23','28','0'),('20','27','0')],calls
    code=(session/'decoded/code1.bin').read_bytes()
    code_sha=hashlib.sha256(code).hexdigest()
    assert code_sha=='28b9452735bb028638615092dff7198a4a1da836964487788a38a0d83b1537b5','Original decoded CODE1 changed'
    loads=re.findall(r'^loaded CODE 1: (\d+) bytes at (0x[0-9a-f]+)$',result.stdout,re.M)
    assert len(loads)==1 and int(loads[0][0])==len(code)-4,'Missing/ambiguous original CODE1 load'
    # MPW offsets are relative to executable bytes after the4byte header.
    action_offset=0x250e;action_address=int(loads[0][1],16)+action_offset
    assert code[4+action_offset:4+action_offset+8].hex()=='4e56fffe48e70300'
    entries=re.findall(r'^callback: enter (0x[0-9a-f]+), token=5$',result.stdout,re.M)
    assert entries==[f'{action_address:#x}']*3 and result.stdout.count('callback: return token=5 result=0')==3,'Original action procedure did not run exactly once per gesture'
    subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/'TeachText-scrolled-runtime.png')],check=True,capture_output=True)
    report={'unmodified_application':True,'application_resource_sha256':sha(app),'script_sha256':sha(script),'text_sha256':hashlib.sha256(text).hexdigest(),'text_bytes':len(text),'original_control_action_callbacks':3,'action_code_resource':1,'action_code_offset':action_offset,'action_code_sha256':code_sha,'action_loaded_address':action_address,'tracked_parts':[21,23,20],'control_values':[2,28,27],'dest_rect':state[0]['dest_rect'],'view_rect':state[0]['view_rect'],'document_and_selection_preserved':True,'returned_to_bounded_event_loop':True,'scope':'Original Open, lower-arrow, page-down and upper-arrow paths on80 lines. Exact scripted callbacks and edit geometry; native manager comparisons and actual hosted interaction are separate evidence. Native original TeachText UI and indicator dragging remain open.','goal_complete':False}
    (OUT/'TeachText-scroll-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
