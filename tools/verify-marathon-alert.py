#!/usr/bin/env python3
"""Replay one real acknowledgement in the unchanged original Marathon binary."""
import hashlib,json,re,subprocess,sys,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    color='--color-screen' in sys.argv
    prefix='Marathon-indexed' if color else 'Marathon-alert'
    trace_name=prefix+'-attempt.txt'
    pin=json.loads((OUT/'Marathon-provenance.json').read_text());volume=ROOT/'work/volumes/marathon-source-v1'
    before={str(p.relative_to(volume)):digest(p) for p in volume.rglob('*') if p.is_file()}
    for asset in pin['assets']:
        for fork in ['data','resource']:
            path=volume/'files'/asset['name'].lower().encode().hex()/fork
            assert path.stat().st_size==asset[fork+'_bytes'] and digest(path)==asset[fork+'_sha256']
    app=ROOT/'work/apps/Marathon/Marathon.rsrc';data=ROOT/'work/apps/Marathon/Marathon.data'
    original=next(a for a in pin['assets'] if a['name']=='Marathon v1.2')
    assert digest(app)==original['resource_sha256'] and digest(data)==original['data_sha256']
    script=ROOT/'work/validation/marathon-ack-sound.json';script.parent.mkdir(parents=True,exist_ok=True)
    script.write_text(json.dumps({'schema':1,'events':[{'what':3,'message':0x240d,'modifiers':128}]},indent=2)+'\n')
    command=['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--',str(app),'--application-name','Marathon v1.2','--application-data',str(data),'--application-files',str(volume),'--trace','--trace-files','--trace-alerts','--trace-sound','--event-script',str(script),'--instruction-limit','20000000' if color else '300000']
    if color:command+=['--color-screen','--trace-timers','--trace-vbl','--trace-sane','--capture-framebuffer',str(ROOT/'work/validation/marathon-indexed-current.ppm')]
    picture_frames=None
    if color and '--capture-picture-frames'in sys.argv:
        picture_frames=Path(tempfile.mkdtemp(prefix='marathon-picture-frames-',dir=ROOT/'work/validation'))
        command+=['--capture-picture-frames',str(picture_frames)]
    process=subprocess.run(command,cwd=ROOT,capture_output=True,text=True)
    trace=process.stdout+process.stderr;(OUT/trace_name).write_text(trace)
    opened=trace.index('Alert opened: id=129');closed=trace.index('Alert closed: result=1',opened)
    assert 'sound allocation: synth=5 init=0x80' in trace and 'error=-200' in trace[:opened]
    status=trace.strip().splitlines()[-1];assert process.returncode==1
    assert ('unimplemented Macintosh trap' in status and not any(t in status for t in ['0xa807','0xa985'])) or status.startswith('instruction limit reached;') or (color and status.startswith('resource mutation/release of an unowned handle is not implemented:'))
    assert closed<len(trace)-len(status),'No real execution after acknowledgement'
    if color:
        assert status!='unsupported font selection 22/12/0','Previously implemented Courier startup support regressed'
        assert 'StopAlert request: id=128' not in trace,'Color-capable game regressed to unsupported-display warning'
        assert not any(('trap '+op+' ') in status for op in ['0xa058','0xa05a','0xa064']),'A previously implemented startup service regressed'
    after={str(p.relative_to(volume)):digest(p) for p in volume.rglob('*') if p.is_file()};assert before==after,'Original source volume was modified'
    result={'original_application_sha256':digest(app),'original_data_sha256':digest(data),'original_assets_checked':len(pin['assets']),'original_volume_unchanged':True,
        'input':'One ordinary Return key event; actual hosted mouse and keyboard input verified separately with CUA.','input_script_sha256':digest(script),'warning_id':129,'acknowledged_item':1,'actual_game_executed_beyond_warning':True,
        'trace':trace_name,'trace_sha256':digest(OUT/trace_name),'terminal_exit_code':process.returncode,'terminal_status':status,
        'counter_scope':'Main runtime diagnostic counts execution-loop iterations, including modal waits/callback scheduling; it is not a retired CPU-instruction counter.',
        'color_capability_available':color,'scope':'Honest absent-sound-backend failure and resource-backed warning acknowledgement in unchanged original game. No title/color/gameplay/audio completion claim.','goal_complete':False}
    if color:
        subprocess.run(['sips','-s','format','png',str(ROOT/'work/validation/marathon-indexed-current.ppm'),'--out',str(OUT/(prefix+'-runtime.png'))],check=True,capture_output=True)
        result['framebuffer']=prefix+'-runtime.png'
        result['framebuffer_scope']='Actual terminal framebuffer; no title/gameplay or native pixel-equivalence claim.'
    if picture_frames is not None:
        result['picture_frames']=[]
        for source in sorted(picture_frames.glob('*.ppm')):
            name='Marathon-'+source.stem+'.png'
            subprocess.run(['sips','-s','format','png',str(source),'--out',str(OUT/name)],check=True,capture_output=True)
            result['picture_frames'].append({'file':name,'sha256':digest(OUT/name)})
        result['picture_frames_scope']='Actual screen framebuffer after original DrawPicture/CopyBits calls, first32 maximum; optional diagnostics change timing. No source-asset substitution or title/menu/gameplay equivalence claim.'
    (OUT/(prefix+'-result.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
if __name__=='__main__':main()
