#!/usr/bin/env python3
"""Replay the shared production basic arithmetic package against native OS 9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import differences,read_probe
from sanebasicx_observations import project,FIELDS,HEX
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'SANEBasicX-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed basic arithmetic input: '+f
    assert pin['installed_forks_match']
    subprocess.run(['python3',str(ROOT/'tools/build-sane-operands.py'),'--check'],check=True,stdout=subprocess.DEVNULL)
    native=project(json.loads((OUT/'SANEBasicXProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','sane-basicx-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'SANEBasicXProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for name,data in [('reference',native),('runtime',runtime)]:
        path=OUT/f'SANEBasicXSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(native,runtime),differences(native,runtime)[:10]
    missing=mutated=0;scratch=ROOT/'work/validation/sanebasicx-incomplete.json'
    for key in sorted(FIELDS):
        bad=copy.deepcopy(native);del bad['math']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
        bad=copy.deepcopy(native);r=bad['math']['records'][0]
        if key in HEX:r[key]=f'{int(r[key],16)^1:020x}'
        elif key=='guards':r[key][0]^=1
        else:r[key]^=1
        assert differences(native,bad),'Masked '+key;mutated+=1
    print(json.dumps({'records':12960,'all_basic_results_bit_exact':True,'all_selected_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutated}))
if __name__=='__main__':main()
