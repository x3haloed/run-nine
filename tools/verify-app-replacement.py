#!/usr/bin/env python3
"""Verify original TeachText cancellation, replacement, persistence and recovery."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
spec=importlib.util.spec_from_file_location('roundtrip',ROOT/'tools/verify-app-roundtrip.py')
workflow=importlib.util.module_from_spec(spec);spec.loader.exec_module(workflow)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def saved(volume):
    catalog=json.loads((volume/'catalog.json').read_text())
    if len(catalog['files'])!=1:raise RuntimeError('Unexpected files after replacement')
    entry=catalog['files'][0];name=bytes(entry['name'])
    if name!=b'reference.txt' or bytes(entry['finder'])[:8]!=b'TEXTttxt':raise RuntimeError('Saved identity changed')
    folder=volume/'files'/name.decode('mac_roman').lower().encode().hex()
    return entry,(folder/'data').read_bytes(),(folder/'resource').read_bytes()
def main():
    app=ROOT/'work/apps/TeachText/TeachText.rsrc'
    if sha(app)!=json.loads((OUT/'TeachText-reference-result.json').read_text())['resource_sha256']:raise RuntimeError('Original application identity mismatch')
    parent=ROOT/'work/app-validation';parent.mkdir(exist_ok=True,parents=True)
    session=Path(tempfile.mkdtemp(prefix='replacement-',dir=parent));seed=session/'seed'
    baseline=(OUT/'TeachText-reference-document.txt').read_bytes()
    workflow.execute('save-close-open.json',seed,'replacement-baseline')
    original=saved(seed)
    if original[1]!=baseline or original[2]:raise RuntimeError('Baseline differs from OS9 saved file')
    volume=session/'cancel';shutil.copytree(seed,volume)
    before={str(p.relative_to(volume)):sha(p) for p in (volume/'recovery').glob('**/manifest.json')} if (volume/'recovery').exists() else {}
    trace,state,count=workflow.execute('cancel-replacement.json',volume,'replacement-cancel')
    after={str(p.relative_to(volume)):sha(p) for p in (volume/'recovery').glob('**/manifest.json')} if (volume/'recovery').exists() else {}
    if saved(volume)!=original or before!=after or not workflow.text_matches(state,b'text') or 'replacement confirmation closed, accepted=false' not in trace:raise RuntimeError('Cancel changed saved bytes/metadata or failed to return to Save')
    _,new_state,new_count=workflow.execute('save-close-new.json',session/'new-after-close','save-close-new')
    if not workflow.text_matches(new_state,b'text') or saved(session/'new-after-close')[1]!=baseline:raise RuntimeError('New after Close leaked prior document selection or altered saved bytes')
    cases=[{'case':'new-after-close','events':new_count,'fresh_document_and_old_saved_file_verified':True},{'case':'cancel','events':count,'original_bytes_metadata_and_versions_unchanged':True,'new_unsaved_document_preserved':True}]
    for script,label in [('replace-existing.json','replacement-accept'),('save-existing-document.json','save-existing')]:
        volume=session/label;shutil.copytree(seed,volume)
        trace,state,count=workflow.execute(script,volume,label)
        if saved(volume)[1:]!=(b'text',b'') or not workflow.text_matches(state,b'text'):raise RuntimeError(f'{label}: persisted/reopened file differs, including trailing bytes')
        _,fresh,_=workflow.execute('reopen-document.json',volume,label+'-fresh')
        if not workflow.text_matches(fresh,b'text'):raise RuntimeError(f'{label}: fresh reopen differs')
        versions=[]
        for manifest in (volume/'recovery').glob('*/manifest.json'):
            v=json.loads(manifest.read_text())
            if bytes(v['name'])==b'reference.txt' and bytes(v['finder'])==bytes(original[0]['finder']) and (manifest.parent/'data').read_bytes()==baseline and (manifest.parent/'resource').read_bytes()==original[2]:versions.append(manifest.parent)
        if not versions:raise RuntimeError(f'{label}: original version was not retained')
        export=session/(label+'-recovered')
        subprocess.run([sys.executable,str(ROOT/'tools/recover-file.py'),str(versions[0]),str(export)],check=True,capture_output=True)
        if saved(export)!=original:raise RuntimeError('Recovery failed to preserve both forks and Finder metadata')
        _,recovered,_=workflow.execute('reopen-document.json',export,label+'-recovered')
        if not workflow.text_matches(recovered,baseline):raise RuntimeError('Original app cannot read recovered version')
        cases.append({'case':label,'events':count,'saved_data_bytes':4,'saved_data_sha256':hashlib.sha256(b'text').hexdigest(),'same_and_fresh_process_reopen':True,'prior_version_recovery_verified_in_original_app':True,'volume':str(volume),'recovery':str(versions[0])})
    report={'application':'TeachText7.0','resource_sha256':sha(app),'unchanged_binary':True,'cases':cases,'interface':'Scripted classic keys; original application hooks, file calls and CPU execution','scope':'Successful root-directory saves, confirmation cancellation and recoverable prior versions. Power-loss atomicity and all I/O failure dialog paths are not claimed. Native UI exercised separately.','goal_complete':False}
    (OUT/'TeachText-replacement-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':
    try:main()
    except Exception as e:
        (OUT/'TeachText-replacement-result.json').write_text(json.dumps({'verification_error':str(e),'goal_complete':False},indent=2)+'\n');raise
