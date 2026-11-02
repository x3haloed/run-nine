#!/usr/bin/env python3
"""Verify a bounded original SimpleText New/type/insert/delete workflow.

Saving/reopening and UI input are separate evidence, not implied by this check.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import struct
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    app=ROOT/'work/apps/SimpleText/SimpleText.rsrc'
    native=json.loads((OUT/'SimpleText-reference-result.json').read_text())
    if sha(app)!=native['application_resource_sha256']:raise RuntimeError('SimpleText binary differs from native reference')
    script=ROOT/'probes/simpletext/type-edit-document.json'
    documents=OUT/'SimpleText-typing-documents.json';documents.unlink(missing_ok=True)
    ppm=ROOT/'work/validation/SimpleText-typing.ppm'
    command=['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--application-name','SimpleText','--trace','--instruction-limit','200000','--event-script',str(script),'--dump-documents',str(documents),'--capture-framebuffer',str(ppm)]
    result=subprocess.run(command,cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (OUT/'SimpleText-typing-runtime-trace.txt').write_text(result.stdout)
    if result.returncode!=1 or not result.stdout.strip().splitlines()[-1].startswith('instruction limit reached'):raise RuntimeError('Original SimpleText did not reach bounded event loop: '+result.stdout[-2000:])
    expected=(OUT/'SimpleText-reference-document.txt').read_bytes()
    # Read the one-run style from the preserved native saved file, solely in
    # this external verifier; the runtime never imports expected observations.
    fork=(OUT/'SimpleText-reference-document.rsrc').read_bytes()
    if hashlib.sha256(fork).hexdigest()!=native['resource_sha256']:raise RuntimeError('Native saved resource fork identity changed')
    data_base,map_base,_,_=struct.unpack_from('>IIII',fork)
    types=map_base+struct.unpack_from('>H',fork,map_base+24)[0]
    style=None
    for i in range(struct.unpack_from('>H',fork,types)[0]+1):
        kind,count,offset=struct.unpack_from('>4sHH',fork,types+2+8*i)
        for j in range(count+1):
            ref=types+offset+12*j
            if kind==b'styl' and struct.unpack_from('>h',fork,ref)[0]==128:
                pos=data_base+int.from_bytes(fork[ref+5:ref+8],'big');length=struct.unpack_from('>I',fork,pos)[0];style=fork[pos+4:pos+4+length]
    if style is None or len(style)!=22 or struct.unpack_from('>HI',style)!=(1,0):raise RuntimeError('Expected one native saved style run from character zero')
    expected_style=[struct.unpack_from('>h',style,10)[0],style[12],struct.unpack_from('>h',style,14)[0]]
    state=json.loads(documents.read_text());rows=state.get('documents',[])
    if len(rows)!=1 or not rows[0]['styled'] or bytes(rows[0]['text_bytes'])!=expected or rows[0]['selection']!=[len(expected),len(expected)]:raise RuntimeError('Original styled document does not match native reference text and final caret')
    if rows[0]['font']!=expected_style:raise RuntimeError('Default font/face/size differs from native saved document')
    delivered=result.stdout.count('script event:');count=len(json.loads(script.read_text())['events'])
    if delivered!=count:raise RuntimeError(f'Only {delivered}/{count} input events delivered')
    subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/'SimpleText-typing-runtime.png')],check=True,capture_output=True)
    report={'application':'SimpleText1.4','application_resource_sha256':sha(app),'unmodified_native_reference_binary':True,'script_sha256':sha(script),'events_delivered':delivered,'text_bytes':len(expected),'text_sha256':hashlib.sha256(expected).hexdigest(),'matches_native_saved_document_text':True,'styled_record':True,'native_default_style':expected_style,'default_style_matches_native_saved_document':True,'insert_delete_edit_verified':True,'instruction_limit':200000,'scope':'Scripted New/type/insert/delete through original application code. Actual hosted keys captured separately. Save/reopen remains incomplete. No glyph artwork equivalence claim.','save_reopen_verified':False,'goal_complete':False}
    (OUT/'SimpleText-typing-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
