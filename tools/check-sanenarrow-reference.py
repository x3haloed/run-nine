#!/usr/bin/env python3
"""Replay the shared production signed integer conversion package against native OS 9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import differences,read_probe
from sanenarrow_observations import native_project,runtime_project,FIELDS,HEX
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    for name in ['SANENarrow','SANERound','SANEIntFlags']:
        pin=json.loads((OUT/(name+'-provenance.json')).read_text())
        for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed integer input: '+f
        if name!='SANENarrow':assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    for script in ['build-sane-round-operands.py','build-sane-intflags-operands.py']:subprocess.run(['python3',str(ROOT/'tools'/script),'--check'],check=True,stdout=subprocess.DEVNULL)
    native=native_project([json.loads((OUT/f).read_text())for f in ['SANERoundProbe-v1-reference.json','SANEIntFlagsProbe-v1-reference.json']])
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','sane-narrow-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'SANENarrowProbe-runtime-raw.json').write_text(text);runtime=runtime_project(json.loads(text))
    for name,data in [('reference',native),('runtime',runtime)]:
        path=OUT/f'SANENarrowSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(native,runtime),differences(native,runtime)[:10]
    missing=mutated=0;scratch=ROOT/'work/validation/sanenarrow-incomplete.json'
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
    print(json.dumps({'records':26010,'supported_integer_results_bit_exact':True,'all_selected_fields_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutated}))
if __name__=='__main__':main()
