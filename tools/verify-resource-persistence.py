#!/usr/bin/env python3
"""Verify stored resource bytes and a real cold-process Resource Manager reopen."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
def run(probe,volume):
    result=subprocess.run(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe',probe,'--volume',str(volume)],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    if result.returncode:raise RuntimeError(result.stdout+result.stderr)
    return json.loads(result.stdout)
def main():
    base=ROOT/'work/app-validation';base.mkdir(parents=True,exist_ok=True);session=Path(tempfile.mkdtemp(prefix='resource-persistence-',dir=base));volume=session/'volume'
    first=run('res-io-v1',volume);native=json.loads((OUT/'ResIOProbe-reference.json').read_text())
    if first!=native:raise RuntimeError('Persistent resource-I/O run differs from native API capture')
    reopened=run('res-io-reload-v1',volume);actual=reopened['resources'];expected=native['resources']
    for k in ['read_open','read_info','name_hex','data_hex']:
        if actual[k]!=expected[k]:raise RuntimeError('Cold reopen differs from native resource observation: '+k)
    if actual['home_matches']!=1 or actual['close']!=[0,1]:raise RuntimeError('Cold reopen ownership or current-file restoration failed')
    module=importlib.util.spec_from_file_location('resio_audit',ROOT/'tools/check-resio-reference.py');audit=importlib.util.module_from_spec(module);module.loader.exec_module(audit)
    resource_path=volume/'files'/b'resourcea'.hex()/'resource';entries=audit.parse_fork(resource_path.read_bytes())
    wanted={'id':expected['read_info'][1],'kind':expected['read_info'][2],'attributes':expected['read_info'][3],'name_hex':expected['name_hex'],'data_hex':expected['data_hex']}
    if entries!=[wanted]:raise RuntimeError('Independent file-format reader disagrees with native API values')
    (OUT/'ResIOProbe-persistent-runtime.json').write_text(json.dumps(first,indent=2)+'\n');(OUT/'ResIOProbe-reloaded-runtime.json').write_text(json.dumps(reopened,indent=2)+'\n')
    report={'persistent_native_api_comparison':True,'cold_process_reopen_matches_native':True,'independent_file_format_matches_native_api_values':True,'volume':str(volume),'runtime_comparison_count_added':0,'scope':'Owned resource I/O fixture, distinct host processes, real persisted map/data/name/attributes. Not an original-application save/reopen completion claim.','goal_complete':False}
    (OUT/'Resource-persistence-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
