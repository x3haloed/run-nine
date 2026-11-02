#!/usr/bin/env python3
"""Check the real production datafork open ABI against native OS9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from opendf_observations import project,FIELDS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'OpenDF2-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed OpenDF input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'OpenDFProbe-v2-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','open-df-runtime-raw-v2'],cwd=ROOT,text=True)
    (OUT/'OpenDFProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for name,data in [('reference',native),('runtime',runtime)]:
        p=OUT/f'OpenDFSemanticsProbe-{name}.json';p.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(p)==data
    assert not differences(native,runtime),differences(native,runtime)[:10]
    missing=mutated=0;p=ROOT/'work/validation/opendf-incomplete.json'
    for key in sorted(FIELDS):
        bad=copy.deepcopy(native);del bad['files']['records'][0][key];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
        bad=copy.deepcopy(native);r=bad['files']['records'][0]
        if isinstance(r[key],list):r[key][0]^=1
        else:r[key]^=1
        assert differences(native,bad),'Masked '+key;mutated+=1
    print(json.dumps({'records':72,'all_selected_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutated,'normalization':'Only successful positive file refs; failed refs, status, guards and read bytes exact.'}))
if __name__=='__main__':main()
