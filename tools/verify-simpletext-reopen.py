#!/usr/bin/env python3
"""Run the original SimpleText save/close/reopen path, then reopen in a fresh process."""
import hashlib,importlib.util,json,struct,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    app=ROOT/'work/apps/SimpleText/SimpleText.rsrc';native=json.loads((OUT/'SimpleText-reference-result.json').read_text())
    assert sha(app)==native['application_resource_sha256'],'Original application identity changed'
    expected=(OUT/'SimpleText-reference-document.txt').read_bytes()
    spec=importlib.util.spec_from_file_location('resource_audit',ROOT/'tools/check-resio-reference.py');audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
    native_resources=audit.parse_fork((OUT/'SimpleText-reference-document.rsrc').read_bytes())
    styl=next(r for r in native_resources if r['kind']==int.from_bytes(b'styl','big') and r['id']==128)
    payload=bytes.fromhex(styl['data_hex']);count=struct.unpack_from('>H',payload)[0];native_runs=[]
    for i in range(count):
        offset,height,ascent,font,face,pad,size,red,green,blue=struct.unpack_from('>IhhhBBhHHH',payload,2+20*i)
        native_runs.append({'offset':offset,'font':font,'face':face,'size':size,'color':[red,green,blue],'height':height,'ascent':ascent})
    session=Path(tempfile.mkdtemp(prefix='simpletext-roundtrip-',dir=ROOT/'work/app-validation'));volume=session/'volume'
    def run(phase,script):
        docs=OUT/f'SimpleText-{phase}-documents.json';docs.unlink(missing_ok=True);ppm=session/f'{phase}.ppm'
        result=subprocess.run(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--application-name','SimpleText','--trace','--trace-files','--instruction-limit','1000000','--event-script',str(script),'--volume',str(volume),'--dump-documents',str(docs),'--capture-framebuffer',str(ppm)],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        (OUT/f'SimpleText-{phase}-trace.txt').write_text(result.stdout)
        assert result.returncode==1 and result.stdout.strip().splitlines()[-1].startswith('instruction limit reached'),'Original application did not return to the bounded event loop: '+result.stdout[-1500:]
        delivered=result.stdout.count('script event:')+result.stdout.count('Standard File filter event: what=3 ')
        assert delivered==len(json.loads(script.read_text())['events']),'Incomplete delivered input'
        state=json.loads(docs.read_text())['documents'];assert len(state)==1 and bytes(state[0]['text_bytes'])==expected,'Reopened text differs'
        assert state[0]['styled'] and state[0]['selection']==[0,0],'Unexpected reopened edit record'
        assert state[0]['style_runs']==native_runs,'Reopened style runs differ from native styl128'
        assert 'Standard File: CustomGetFile dialog opened' in result.stdout and 'Standard File: dialog closed, accepted=true' in result.stdout,'Original file selection not accepted'
        assert 'file PB trap=0xa002 error=0' in result.stdout,'Original data fork read missing'
        if phase=='roundtrip':assert 'file PB trap=0xa001 error=0' in result.stdout,'Original close missing'
        subprocess.run(['sips','-s','format','png',str(ppm),'--out',str(OUT/f'SimpleText-{phase}-runtime.png')],check=True,capture_output=True)
        return {'events_delivered':delivered,'script_sha256':sha(script),'text_and_style_restored':True,'returned_to_event_loop':True}
    same=run('roundtrip',ROOT/'probes/teachtext/save-close-open.json')
    catalog_before=(volume/'catalog.json').read_bytes();directory=volume/'files'/b'reference.txt'.hex()
    data=(directory/'data').read_bytes();resource=(directory/'resource').read_bytes()
    assert data==expected,'Saved data differs from native'
    assert audit.parse_fork(resource)==audit.parse_fork((OUT/'SimpleText-reference-document.rsrc').read_bytes()),'Saved style payload differs from native'
    fresh=run('cold-reopened',ROOT/'probes/teachtext/reopen-document.json')
    assert (directory/'data').read_bytes()==data and (directory/'resource').read_bytes()==resource and (volume/'catalog.json').read_bytes()==catalog_before,'Opening mutated the durable document'
    (OUT/'SimpleText-roundtrip-document.data').write_bytes(data);(OUT/'SimpleText-roundtrip-document.rsrc').write_bytes(resource)
    report={'unmodified_application':True,'application_resource_sha256':sha(app),'same_process_save_close_reopen':same,'fresh_process_reopen':fresh,'data_matches_native':True,'style_resource_metadata_and_payload_match_native':True,'reopened_style_runs_match_native':True,'saved_filename':'reference.txt','data_bytes':len(data),'volume':str(volume),'save_reopen_complete':True,'scope':'Scripted original application Save, Close and Open followed by a separate process Open without retyping. Native comparison excludes volatile resource-map handles/links. Default Roman document and three owned mixed-style import ranges plus oversized range are separate evidence; hosted reopen and responsiveness remain separate gates. Full traces retain pre-save metadata and busy cleanup errors.','goal_complete':False}
    (OUT/'SimpleText-reopen-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
