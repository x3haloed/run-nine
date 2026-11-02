#!/usr/bin/env python3
"""Verify original SimpleText cancels Save without losing text or its visible ink."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def text_pixels(path):
    magic,dimensions,maximum,pixels=path.read_bytes().split(b'\n',3)
    if magic!=b'P6' or dimensions!=b'640 480' or maximum!=b'255' or len(pixels)!=640*480*3:raise RuntimeError('Unexpected framebuffer layout')
    # Text ink only; the blinking insertion caret is outside this rectangle.
    return b''.join(pixels[(y*640+8)*3:(y*640+97)*3] for y in range(44,60))
def main():
    app=ROOT/'work/apps/SimpleText/SimpleText.rsrc';script=ROOT/'probes/simpletext/cancel-save.json'
    native=json.loads((OUT/'SimpleText-reference-result.json').read_text())
    if sha(app)!=native['application_resource_sha256']:raise RuntimeError('Original SimpleText identity changed')
    base=ROOT/'work/app-validation';base.mkdir(parents=True,exist_ok=True)
    session=Path(tempfile.mkdtemp(prefix='simpletext-cancel-',dir=base));volume=session/'volume'
    documents=OUT/'SimpleText-cancel-documents.json';documents.unlink(missing_ok=True);ppm=session/'cancel.ppm'
    result=subprocess.run(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--application-name','SimpleText','--trace','--instruction-limit','250000','--event-script',str(script),'--volume',str(volume),'--dump-documents',str(documents),'--capture-framebuffer',str(ppm)],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (OUT/'SimpleText-cancel-trace.txt').write_text(result.stdout)
    if result.returncode!=1 or not result.stdout.strip().splitlines()[-1].startswith('instruction limit reached'):raise RuntimeError('Cancellation stopped before the bounded event loop: '+result.stdout[-2000:])
    expected=(OUT/'SimpleText-reference-document.txt').read_bytes();state=json.loads(documents.read_text());baseline=json.loads((OUT/'SimpleText-typing-documents.json').read_text())
    if len(state.get('documents',[]))!=1 or state['documents'][0]!=baseline['documents'][0] or bytes(state['documents'][0]['text_bytes'])!=expected:raise RuntimeError('Cancel changed original document state')
    catalog=json.loads((volume/'catalog.json').read_text())
    if catalog['files']:raise RuntimeError('Cancel created a file')
    before=text_pixels(ROOT/'work/validation/SimpleText-typing.ppm');after=text_pixels(ppm)
    if before!=after or b'\x00\x00\x00' not in after:raise RuntimeError('Text ink was not restored after Cancel')
    if 'Standard File: dialog closed, accepted=false' not in result.stdout or 'callback: enter ' not in result.stdout or 'token=3' not in result.stdout:raise RuntimeError('Original hook/filter/cancellation path not observed')
    draw_enters=sum('callback: enter ' in line and 'token=4' in line for line in result.stdout.splitlines())
    draw_returns=result.stdout.count('callback: return token=4 result=0')
    if not draw_enters or draw_enters!=draw_returns:raise RuntimeError('Original user-item procedures did not all return')
    delivered=result.stdout.count('script event:')+result.stdout.count('Standard File input:')
    if delivered!=len(json.loads(script.read_text())['events']):raise RuntimeError('Incomplete cancellation input delivery')
    subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/'SimpleText-cancel-runtime.png')],check=True,capture_output=True)
    report={'application_resource_sha256':sha(app),'script_sha256':sha(script),'events_delivered':delivered,'original_modal_filter_executed':True,'original_save_hook_executed':True,'original_user_item_procedures_executed':draw_enters,'all_user_item_procedures_returned':True,'cancel_returned_to_document':True,'document_state_matches_independent_typing_baseline':True,'no_file_created':True,'text_pixels_match_typing_baseline':True,'text_rectangle':[44,8,60,97],'text_pixel_sha256':hashlib.sha256(after).hexdigest(),'volume':str(volume),'scope':'Scripted Escape cancellation through original SimpleText filter/hook; text ink compared with independent original-app typing baseline using our artwork. Actual hosted mouse cancellation is separate evidence. Save/reopen remains incomplete.','goal_complete':False}
    (OUT/'SimpleText-cancel-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
