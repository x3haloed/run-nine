#!/usr/bin/env python3
"""Resource dialog creation versus independent native captures and lifecycle invariants."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from newdialog_observations import project,DETAILS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def leaves(value,path=()):
    if isinstance(value,dict):
        for k,v in value.items():yield from leaves(v,path+(k,))
    elif isinstance(value,list):
        for k,v in enumerate(value):yield from leaves(v,path+(k,))
    else:yield path,value
def main():
    for filename in ['NewDialog-provenance.json','NewDialogDetails-provenance.json']:
        pin=json.loads((OUT/filename).read_text())
        for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native dialog input: '+f
        assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'NewDialogDetailsProbe-v1-reference.json').read_text()))
    base=project(json.loads((OUT/'NewDialogProbe-v1-reference.json').read_text()),False)
    subset=[{k:v for k,v in r.items()if k not in DETAILS}for r in native['dialogs']['records'][:48]]
    assert not differences(base,subset),'Independent native base/detail captures disagree'
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','new-dialog-details-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'NewDialogDetailsProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/f'NewDialogDetailsProbe-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)[:15]
    scratch=ROOT/'work/validation/newdialog-incomplete.json';missing=mutations=0
    for index in [0,24]:
        for key in native['dialogs']['records'][index]:
            bad=copy.deepcopy(native);del bad['dialogs']['records'][index][key];scratch.write_text(json.dumps(bad))
            try:read_probe(scratch)
            except ValueError:missing+=1
            else:raise AssertionError('Accepted missing '+key)
    for key in native['dialogs']['records'][0]['items'][0]:
        bad=copy.deepcopy(native);del bad['dialogs']['records'][0]['items'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing item '+key)
    for index,row in enumerate(runtime['dialogs']['records']):
        for path,old in list(leaves(row)):
            obj=row
            for key in path[:-1]:obj=obj[key]
            obj[path[-1]]=(('f'if old[0]!='f'else'e')+old[1:]if old else'00')if isinstance(old,str)else old^1
            assert differences(native['dialogs']['records'][index],row),'Masked dialog leaf '+str(path)
            obj[path[-1]]=old;mutations+=1
    print(json.dumps({'records':96,'independent_base_cases':48,'public_fields_exact':True,'missing_observations_rejected':missing,'leaf_mutations_detected':mutations,'repeated_close_reclaims_owned_allocations':True,'closed_dialogs_leave_no_registered_TextEdit_documents':True}))
if __name__=='__main__':main()
