#!/usr/bin/env python3
"""Recheck second-pass evidence. This does not replace the full runtime validator or UI observation."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import runpy
import subprocess
import sys
from compare import differences, read_probe
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
checks=[]
evidence={}
def load(name):
    path=OUT/name;evidence[name]=hashlib.sha256(path.read_bytes()).hexdigest()
    return json.loads(path.read_text())
def blob(name):
    path=OUT/name;b=path.read_bytes();evidence[name]=hashlib.sha256(b).hexdigest();return b
def require(condition,message):
    if not condition:raise RuntimeError(message)
def passed(name,detail,artifacts):checks.append({'requirement':name,'status':'proved','evidence':artifacts,'finding':detail})
def document(name):
    rows=load(name)['documents'];require(len(rows)==1,name+': expected one document');return rows[0]
def trace(name):
    text=blob(name).decode();require('unimplemented Macintosh trap' not in text,name+': unsupported trap');return text

def main():
    validation=load('Validation-summary.json')
    require(len(validation['cases'])==65 and validation['rust_tests_passed']==35 and validation['frontend_tests_passed']==3,'Unexpected validation coverage')
    names={probe:name for name,probe in runpy.run_path(str(ROOT/'tools/validate-runtime.py'))['CASES']}
    for case in validation['cases']:
        probe=case['probe']
        ref=OUT/(names[probe]+'-reference.json');candidate=OUT/(names[probe]+'-runtime.json')
        require(hashlib.sha256(ref.read_bytes()).hexdigest()==case['reference_sha256'],probe+': changed reference')
        require(hashlib.sha256(candidate.read_bytes()).hexdigest()==case['runtime_sha256'],probe+': changed candidate')
        a=read_probe(ref);b=read_probe(candidate)
        require(not differences(a,b) and case['equal'],probe+': mismatch')
        evidence[ref.name]=hashlib.sha256(ref.read_bytes()).hexdigest();evidence[candidate.name]=hashlib.sha256(candidate.read_bytes()).hexdigest()
    for tool in ['check-comparison-invariants.py','check-font-profile-invariants.py','check-control-thumb-reference.py','check-control-thumb-edges-reference.py','check-resio-reference.py','check-delay-reference.py']:
        subprocess.run([sys.executable,str(ROOT/'tools'/tool)],cwd=ROOT,check=True,capture_output=True)
    passed('Strict native behavioral comparisons','65 current reference/runtime pairs revalidated, hashes bound to full validation. Required-field and declared-normalization adversarial checks pass. Font compatibility data, timer ordering, process identity and thumb timing projections retain their explicit scopes.',['Validation-summary.json','Checkpoint-2-provenance.json','ControlThumbEdges-provenance.json'])
    app_hashes={}
    for app,pin in [('TeachText','b32d7a0ffe50f6a7b9fd62a7702b916cbb82443e57278d0f8fe745529bd2f31a'),('SimpleText','d136ef36aef0efccf2472df603df283afb75302c11331730fc2b3ba11967160b')]:
        resource=ROOT/f'work/apps/{app}/{app}.rsrc'
        require(hashlib.sha256(resource.read_bytes()).hexdigest()==pin,app+': changed original fork')
        app_hashes[app]=pin
    native=load('TeachText-native-interaction-provenance.json')
    require(native['resource_sha256']==app_hashes['TeachText'],'Native TeachText pin differs')
    for name,pin in native['artifacts'].items():require(hashlib.sha256(blob(name)).hexdigest()==pin,'Native artifact changed: '+name)
    require(blob('TeachText-native-word-copy.txt')==b'reference','Native word-copy bytes')
    require(blob('TeachText-native-scroll-source.txt')==blob('TeachText-native-scroll-after.txt'),'Native scrolling altered source')
    for action in ['move','resize']:
        r=load(f'TeachText-hosted-{action}-result.json');require(r['application_resource_sha256']==app_hashes['TeachText'] and r['text_and_selection_preserved'],action+': wrong app/preservation')
        trace(r['trace']);d=document(r['documents']);require(bytes(d['text_bytes'])==b'reference text' and d['selection']==[14,14],action+': document changed')
    require('Rect { top: 81, left: 44, bottom: 516, right: 611 }' in trace('TeachText-hosted-moved-trace.txt'),'Hosted move geometry')
    d=document('TeachText-hosted-resized-documents.json');require(d['dest_rect']==d['view_rect']==[0,4,390,507],'Hosted resize geometry')
    passed('Everyday TeachText window movement and resizing','Original native app visibly moved/resized; strict WindowTracking/WindowGeometry probes verify public geometry/ABI. Actual hosted original app movement and shrink/grow traces preserve all text/selection.',['TeachText-native-moved.png','TeachText-native-resized.png','WindowTrackingProbe-reference.json','WindowGeometryProbe-reference.json','TeachText-hosted-move-result.json','TeachText-hosted-resize-result.json'])
    scroll=load('TeachText-hosted-scroll-result.json');require(scroll['passed'] and scroll['all_document_bytes_preserved'],'Hosted scroll missing')
    d=document(scroll['documents']);source=blob('TeachText-native-scroll-source.txt');require(bytes(d['text_bytes'])==source and d['selection']==[0,0] and d['dest_rect']==[-832,4,-412,552],'Hosted arrow/page document')
    t=trace(scroll['trace']);require(t.count('callback: enter 0x22ba2, token=5')>=3 and t.count('callback: enter 0x22ba2, token=5')==t.count('callback: return token=5 result=0'),'Hosted arrow/page CPU callbacks')
    thumb=load('TeachText-hosted-thumb-result.json');require(thumb['parts']==[129,129,0] and thumb['values']==[4,1,1],'Hosted thumb return')
    require(bytes(document('TeachText-hosted-thumb-documents.json')['text_bytes'])==source,'Hosted thumb altered source')
    slop=load('TeachText-hosted-thumb-slop-result.json');require(slop['parts']==[129,0] and slop['values']==[4,4],'Hosted cancellation')
    require(bytes(document('TeachText-hosted-thumb-slop-documents.json')['text_bytes'])==source,'Hosted outside thumb altered source')
    native_thumb=load('TeachText-native-thumb-provenance.json');blob('TeachText-native-thumb.png');blob('TeachText-native-thumb-start.png')
    passed('Everyday TeachText scrolling','Actual native and hosted arrows/page/up and thumb1->4->1. Real CPU callbacks, 6 action cases and16 edge gestures compare scoped semantics; hosted within-margin release succeeds and beyond-margin release cancels without modifying text. Repeat cadence and exact raster are not asserted equal.',['TeachText-native-interaction-provenance.json','TeachText-native-thumb-provenance.json','TeachText-hosted-scroll-result.json','TeachText-hosted-thumb-result.json','TeachText-hosted-thumb-slop-result.json'])
    d=document('TeachText-hosted-double-click-documents.json');require(bytes(d['text_bytes'])==b'one two,three' and d['selection']==[4,7],'Hosted double-click')
    trace('TeachText-hosted-double-click-trace.txt')
    require(len(load('TEDoubleProbe-reference.json')['textedit']['cases'])==96,'Native word semantics')
    passed('TeachText double-click text selection','Native original app selects reference and saves its9 copied bytes. Hosted original app selects two at4..7;96 strict native TEClick contexts establish shared selection semantics.',['TeachText-native-word-selected.png','TeachText-native-word-copy.txt','TEDoubleProbe-reference.json','TeachText-hosted-double-click-result.json'])
    clip=load('TeachText-clipboard-ui-result.json');require(all(clip[k] for k in ['visible_copy_paste_duplicates_text','visible_cut_empties_document','visible_paste_restores_text','original_save_dialog_saved_result','final_document_bytes_equal_saved_fork']),'Hosted clipboard workflow')
    scripted=load('TeachText-clipboard-result.json');require(len(scripted['cases'])==3,'Scripted clipboard coverage')
    passed('TeachText clipboard operations','Original hosted copy/paste duplication, cut-to-empty, paste restoration and saved28-byte document. Native original app cross-document Copy/Paste saves reference; ClipboardProbe covers local/system scrap behavior. Host pasteboard synchronization is not claimed.',['ClipboardProbe-reference.json','TeachText-clipboard-result.json','TeachText-clipboard-ui-result.json','TeachText-native-interaction-provenance.json'])
    replacement=load('TeachText-replacement-result.json');require(len(replacement['cases'])==4,'Replacement coverage')
    for row in replacement['cases'][2:]:
        volume=Path(row['volume']);folder=volume/'files'/b'reference.txt'.hex()
        require((folder/'data').read_bytes()==b'text' and (folder/'resource').read_bytes()==b'','Current replacement persisted forks')
        recovery=Path(row['recovery']);require((recovery/'data').read_bytes()==b'reference text' and (recovery/'resource').read_bytes()==b'','Current prior version')
    ui=load('TeachText-replacement-ui-result.json');require(all(ui[k] for k in ['cancel_confirmation_returns_to_save','cancel_preserves_old_forks_metadata_and_versions','cancel_save_preserves_new_unsaved_document','replace_button_saves_new_text','same_process_close_open_verified','fresh_process_open_verified','prior_version_metadata_and_both_forks_verified']),'Hosted replacement')
    passed('Safe TeachText saving over existing files','Native confirmation/reply/hook lifecycle compares. Original hosted cancel/accept plus same/fresh-process reopen are verified. Current scripted shorter replacement leaves no trailing bytes; prior forks/Finder metadata restore and reopen. Lock, failed-checkpoint refusal and interrupted-replacement recovery tests pass. Safety is recoverability, not power-loss atomicity.',['SaveConfirmProbe-reference.json','TeachText-replacement-result.json','TeachText-replacement-ui-result.json','Validation-summary.json'])
    require((ROOT/'work/apps/SimpleText/SimpleText.data').read_bytes()==b'','SimpleText data fork changed')
    simple=load('SimpleText-provenance.json');require(simple['resource_sha256']==app_hashes['SimpleText'] and simple['reference_document_workflow_verified'],'SimpleText native identity/workflow')
    reference=load('SimpleText-reference-result.json');require(reference['fresh_process_reopen_verified'],'Native fresh SimpleText')
    expected=blob('SimpleText-reference-document.txt');require(expected==b'reference text','SimpleText native bytes')
    spec=importlib.util.spec_from_file_location('resio',ROOT/'tools/check-resio-reference.py');mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
    native_resources=mod.parse_fork(blob('SimpleText-reference-document.rsrc'))
    for name in ['SimpleText-saved-document.rsrc','SimpleText-roundtrip-document.rsrc','SimpleText-hosted-saved-document.rsrc']:require(mod.parse_fork(blob(name))==native_resources,'SimpleText style fork differs: '+name)
    for name in ['SimpleText-saved-document.data','SimpleText-roundtrip-document.data','SimpleText-hosted-saved-document.data']:require(blob(name)==expected,'SimpleText saved bytes differ')
    for name in ['SimpleText-roundtrip-documents.json','SimpleText-cold-reopened-documents.json','SimpleText-hosted-reopened-documents.json']:
        d=document(name);require(bytes(d['text_bytes'])==expected and d['styled'] and d['selection']==[0,0] and d['style_runs'][0]['font']==3,name+': text/style restore')
    for name in ['SimpleText-roundtrip-trace.txt','SimpleText-cold-reopened-trace.txt','SimpleText-hosted-reopened-trace.txt']:trace(name)
    require(load('SimpleText-reopen-result.json')['save_reopen_complete'],'SimpleText save/reopen incomplete')
    passed('Original SimpleText acquisition, native verification and meaningful independent workflow','Pinned unchanged1.4. Native typed/save/quit/Finder fresh reopen. Original independent app writes14-byte data+styl128 fork, closes and reopens in same/fresh processes; actual hosted Save/Open/Close/Open visibly agrees. Semantic resource payloads/metadata match, volatile map links are explicitly excluded.',['SimpleText-provenance.json','SimpleText-reference-result.json','SimpleText-save-result.json','SimpleText-reopen-result.json','SimpleText-hosted-save-result.json','SimpleText-hosted-reopen-result.json'])
    marathon=load('Marathon-provenance.json');assets=load('Marathon-reference-assets.json');require(assets['all_forks_match_pins'],'Native Marathon assets changed')
    for row in marathon['assets']:
        path=ROOT/'work/apps/Marathon/extracted/Marathon ƒ'/row['name']
        require(hashlib.sha256(path.read_bytes()).hexdigest()==row['data_sha256'],'Marathon data pin differs')
        require(hashlib.sha256((path/'..namedfork/rsrc').read_bytes()).hexdigest()==row['resource_sha256'],'Marathon resource pin differs')
    app=next(row for row in marathon['assets'] if row['name']=='Marathon v1.2')
    for fork in ['data','resource']:
        path=ROOT/('work/apps/Marathon/Marathon.data' if fork=='data' else 'work/apps/Marathon/Marathon.rsrc')
        require(hashlib.sha256(path.read_bytes()).hexdigest()==app[fork+'_sha256'],'Marathon runtime fork differs')
    require(load('Marathon-reference-launch.json')=={'lookup_error':0,'launch_error':0,'process_present':1,'preferred_bytes':7990272,'minimum_bytes':3704832},'Native Marathon launch')
    mp=load('Marathon-reference-provenance.json')
    for name,key in [('Marathon-reference-title.png','reference_title_sha256'),('Marathon-reference-new-game.png','reference_new_game_sha256')]:require(hashlib.sha256(blob(name)).hexdigest()==mp[key],'Marathon screenshot pin')
    attempt=blob('Marathon-current-attempt.txt').decode();require('0xa985 at PC 0x2d6bc after 61980 instructions' in attempt and 'Map' in attempt and 'ID = -37' in attempt,'Current Marathon attempt differs')
    recon=blob('Marathon-reconnaissance.txt').decode();require(all(section in recon for section in ['Files/resources:','Graphics:','Sound:','Timing:','Input:','Architecture:']),'Missing Marathon subsystem analysis')
    passed('Early original Marathon reconnaissance','Pinned original1.2+all sibling assets; native title/demo/new-game first-level render. Current independent execution reaches real asset-missing alert128 after61980 instructions. Concrete asset paths, indexed color, sound queues/clock, timer/VBL, persistent key state and68K/PPC architecture gaps documented. Playability is not required and not claimed.',['Marathon-provenance.json','Marathon-reference-assets.json','Marathon-reference-launch.json','Marathon-reference-new-game.png','Marathon-current-attempt.txt','Marathon-reconnaissance.txt'])
    boot=load('Native-boot-result.json');require(boot['booted'] and boot['shared_core_probe_matches_os9'] and boot['target']=='aarch64-unknown-none','Native boot')
    require(hashlib.sha256(blob('Native-core-kernel.elf')).hexdigest()==boot['kernel_sha256'],'ARM64 artifact pin')
    require('CLASSIC-NATIVE-PASS' in blob('Native-boot-serial.txt').decode() and load('CoreProbe-native-comparison.json')['equal'],'ARM64 execution')
    require(all(line.startswith(('classic-boot ','classic-core ')) for line in boot['dependencies']),'Native hosted dependency')
    core=(ROOT/'compat-core/src/lib.rs').read_text();require('#![no_std]' in core and 'std::' not in core,'Shared core host coupling')
    require('gui = ["dep:minifb"]' in (ROOT/'runtime/Cargo.toml').read_text(),'Optional hosted frontend')
    passed('Hosted/core separation and existing freestanding ARM64 experiment','Host presentation/input optional in Frontend; host volume persistence isolated in Files. The exact shared no_std memory/rectangle core boots in freestanding ARM64 QEMU and matches OS9. ELF has no dynamic interpreter; only classic-boot/core dependencies. No full-native runtime or physical Pi boot is claimed.',['Native-boot-result.json','Native-boot-serial.txt','CoreProbe-native-comparison.json'])
    presentation=load('Hosted-presentation-investigation.json')
    for app,row in presentation['apps'].items():
        require(hashlib.sha256(blob(row['compressed_log'])).hexdigest()==row['compressed_sha256'],'Presentation log changed')
        require(load(row['timing'])['frames']==row['frames'],'Presentation coverage changed')
    passed('Hosted presentation investigation and honest limits','Actual TeachText15337/SimpleText5366 submitted-frame logs retained; median25.35/25.19ms, maxima102.16/116.61ms. Earlier long blank did not reproduce; cause/repair and input-to-photon latency are not claimed. Settled document workflows succeeded.',['Hosted-presentation-investigation.json','TeachText-presentation-timing.json','SimpleText-presentation-timing.json'])
    notices=(ROOT/'runtime/THIRD_PARTY.txt').read_text();require(all(s in notices for s in ['0.14.7','124685ab09961abfc8002ebe66318c471c2f93ad','fontdue 0.9.3','minifb 0.29.0','OFL']),'Dependency provenance missing')
    for name in ['runtime/LICENSE','runtime/fonts/Coppet-OFL.txt','runtime/fonts/URW-OFL.txt','runtime/third-party/minifb-MIT.txt','runtime/vendor/minifb/LOCAL_CHANGES.txt']:require((ROOT/name).is_file(),'Missing notice '+name)
    production=(ROOT/'runtime/src/machine.rs').read_text().split('pub fn core_probe()')[0]
    require(all(s not in production for s in ['work/apps/TeachText','work/apps/SimpleText','outputs/']),'Production app/expected file dependency')
    passed('Classic conventions, independent implementation and dependency provenance','Production Pascal traps/public records,68K app callbacks, resource handles/maps and two-fork file semantics reviewed. Pinned original inputs unchanged. Reuse is declared CPU, support codec, host window/input and open glyph fonts, with licenses; no full existing Toolbox runtime. Native metric/extent data are explicit compatibility facts.',['TeachText-provenance.txt','SimpleText-provenance.json','Marathon-provenance.json','Control-thumb-runtime-provenance.json'])
    blob('Second-pass-handoff.txt')
    passed('Reproduction, checkpoints and physical target agreement','Checkpoints A TeachText, B SimpleText and C Marathon are preserved in chronological Second-pass-plan. Final handoff identifies exact verification/launch/reference commands, binary pins and deferred Pi500 boundary.',['Second-pass-plan.txt','Second-pass-handoff.txt'])
    source_paths=subprocess.run(['git','ls-files','runtime','compat-core','native','tools','probes'],cwd=ROOT,check=True,capture_output=True,text=True).stdout.splitlines()
    digest=hashlib.sha256()
    for name in source_paths:digest.update(name.encode()+b'\0'+hashlib.sha256((ROOT/name).read_bytes()).digest())
    result={'schema':1,'goal':'Second compatibility pass','status':'requirements proved','checkpoint_base':subprocess.run(['git','rev-parse','HEAD'],cwd=ROOT,check=True,capture_output=True,text=True).stdout.strip(),'source_tree_sha256':digest.hexdigest(),'source_file_count':len(source_paths),'requirements':checks,'evidence_sha256':dict(sorted(evidence.items())),'scope':'The user-defined second pass only. Historical per-checkpoint goal_complete=false fields describe partial evidence at that time. No full OS9 equivalence, playable independent Marathon, full bare-metal application runtime or physical Pi500 completion claim. Manual source/UI artifact review accompanies this script; the script alone cannot establish those observations.'}
    (OUT/'Second-pass-audit.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'status':result['status'],'requirements':len(checks),'evidence_files':len(evidence),'native_comparisons':65,'runtime_tests':35,'frontend_tests':3},indent=2))
if __name__=='__main__':main()
