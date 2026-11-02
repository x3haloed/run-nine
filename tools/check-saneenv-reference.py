#!/usr/bin/env python3
"""Replay the shared production procedure environment package against native OS 9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import differences,read_probe
from saneenv_observations import project,FIELDS,HEX
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'SANEEnv-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed procedure environment input: '+f
    assert pin['installed_forks_match']
    native=project(json.loads((OUT/'SANEEnvProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','sane-env-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'SANEEnvProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for name,data in [('reference',native),('runtime',runtime)]:
        path=OUT/f'SANEEnvSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(native,runtime),differences(native,runtime)[:10]
    missing=mutated=0;scratch=ROOT/'work/validation/saneenv-incomplete.json'
    for key in sorted(FIELDS):
        bad=copy.deepcopy(native);del bad['math']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
        bad=copy.deepcopy(native);r=bad['math']['records'][0]
        if key in HEX:r[key]=format(int(r[key],16)^1,'0'+str(len(r[key]))+'x')
        elif key=='guards':r[key][0]^=1
        else:r[key]^=1
        assert differences(native,bad),'Masked '+key;mutated+=1
    print(json.dumps({'records':876,'procedure_words_bit_exact':True,'all_selected_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutated}))
if __name__=='__main__':main()
