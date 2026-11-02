#!/usr/bin/env python3
"""Verify a real unmodified SimpleText save, with native document/style payloads.

Close/reopen and actual hosted input are separate gates, not inferred here.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    app=ROOT/'work/apps/SimpleText/SimpleText.rsrc';script=ROOT/'probes/simpletext/save-document.json';native=json.loads((OUT/'SimpleText-reference-result.json').read_text())
    if sha(app)!=native['application_resource_sha256']:raise RuntimeError('Original application identity differs from native reference')
    base=ROOT/'work/app-validation';base.mkdir(parents=True,exist_ok=True);session=Path(tempfile.mkdtemp(prefix='simpletext-save-',dir=base));volume=session/'volume';docs=OUT/'SimpleText-saved-documents.json';docs.unlink(missing_ok=True);ppm=session/'save.ppm'
    result=subprocess.run(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--application-name','SimpleText','--trace','--trace-files','--instruction-limit','250000','--event-script',str(script),'--volume',str(volume),'--dump-documents',str(docs),'--capture-framebuffer',str(ppm)],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (OUT/'SimpleText-saved-trace.txt').write_text(result.stdout)
    if result.returncode!=1 or not result.stdout.strip().splitlines()[-1].startswith('instruction limit reached'):raise RuntimeError('Original Save did not return to the bounded event loop: '+result.stdout[-1800:])
    delivered=result.stdout.count('script event:')+result.stdout.count('Standard File filter event: what=3 ')
    if delivered!=len(json.loads(script.read_text())['events']):raise RuntimeError('Incomplete original save input delivery')
    expected=(OUT/'SimpleText-reference-document.txt').read_bytes();state=json.loads(docs.read_text())
    if len(state['documents'])!=1 or bytes(state['documents'][0]['text_bytes'])!=expected or state['documents'][0]['font']!=[3,0,12]:raise RuntimeError('Save changed the original document content/default style')
    catalog=json.loads((volume/'catalog.json').read_text());files=catalog['files']
    if len(files)!=1 or bytes(files[0]['name'])!=b'reference.txt':raise RuntimeError('Unexpected file catalog after Save')
    directory=volume/'files'/b'reference.txt'.hex();data=(directory/'data').read_bytes();resource=(directory/'resource').read_bytes()
    if data!=expected or files[0]['finder'][:8]!=native['finder_info'][:8]:raise RuntimeError('Saved document data/creator/type differ from native')
    spec=importlib.util.spec_from_file_location('resio_audit',ROOT/'tools/check-resio-reference.py');audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
    native_styles=audit.parse_fork((OUT/'SimpleText-reference-document.rsrc').read_bytes());styles=audit.parse_fork(resource)
    if styles!=native_styles or len(styles)!=1 or styles[0]['kind']!=int.from_bytes(b'styl','big') or styles[0]['id']!=128:raise RuntimeError('Saved style payload or resource metadata differs from native document')
    if 'Standard File: dialog closed, accepted=true' not in result.stdout:raise RuntimeError('Original Save acceptance not observed')
    subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/'SimpleText-saved-runtime.png')],check=True,capture_output=True)
    (OUT/'SimpleText-saved-document.data').write_bytes(data);(OUT/'SimpleText-saved-document.rsrc').write_bytes(resource)
    report={'application_resource_sha256':sha(app),'unmodified_application':True,'script_sha256':sha(script),'events_delivered':delivered,'original_save_returned_to_event_loop':True,'saved_filename':'reference.txt','data_bytes':len(data),'saved_data_matches_native':True,'saved_style_resources_match_native_metadata_and_payload':True,'finder_type_creator_match_native':True,'document_memory_preserved':True,'style_resource_bytes':len(bytes.fromhex(styles[0]['data_hex'])),'resource_fork_bytes':len(resource),'data_sha256':hashlib.sha256(data).hexdigest(),'resource_sha256':hashlib.sha256(resource).hexdigest(),'volume':str(volume),'save_reopen_complete':False,'scope':'Scripted original application Save and durable on-disk data/style payload verification. Volatile native map links/handles are excluded from resource payload comparison, with both raw forks retained. Nonfatal untitled-spec metadata and busy cleanup calls remain in the complete trace. Original close/reopen is independently checked by verify-simpletext-reopen.py. Hosted Save and responsiveness are separate evidence.','goal_complete':False}
    (OUT/'SimpleText-save-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
