#!/usr/bin/env python3
"""Replay the shared production extended classification package against native OS 9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import differences,read_probe
from saneclass_observations import project,FIELDS,HEX
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'SANEClass-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed extended classification input: '+f
    assert pin['installed_forks_match']
    subprocess.run(['python3',str(ROOT/'tools/build-sane-conversion-operands.py'),'--check'],check=True,stdout=subprocess.DEVNULL)
    native=project(json.loads((OUT/'SANEClassProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','sane-class-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'SANEClassProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for name,data in [('reference',native),('runtime',runtime)]:
        path=OUT/f'SANEClassSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(native,runtime),differences(native,runtime)[:10]
    missing=mutated=0;scratch=ROOT/'work/validation/saneclass-incomplete.json'
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
    print(json.dumps({'records':10350,'classification_words_bit_exact':True,'all_selected_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutated}))
if __name__=='__main__':main()
