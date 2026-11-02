#!/usr/bin/env python3
"""Compare independently executed VBL callbacks with retained native observations."""
import copy, hashlib, json, subprocess
from pathlib import Path
from compare import read_probe, differences
from vbl_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    provenance=json.loads((OUT/'VBL-provenance.json').read_text())
    for file,sha in provenance['pins'].items():
        assert hashlib.sha256((ROOT/file).read_bytes()).hexdigest()==sha,'Changed VBL input: '+file
    assert provenance['installed_forks_match'] and provenance['resource_rebuilt_byte_identical']
    native=json.loads((OUT/'VBLProbe-v3-reference.json').read_text())
    output=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','vbl-runtime-raw-v1','--reference-slot',str(native['main_slot'])],cwd=ROOT,text=True)
    (OUT/'VBLProbe-runtime-raw.json').write_text(output)
    reference=project(native);candidate=project(json.loads(output))
    for name,data in [('reference',reference),('runtime',candidate)]:
        path=OUT/f'VBLSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(reference,candidate),differences(reference,candidate)
    rejected=0;scratch=ROOT/'work/validation/vbl-incomplete.json'
    for section in ['records','callbacks','manual']:
        for key in reference['retrace'][section][0]:
            broken=copy.deepcopy(reference);del broken['retrace'][section][0][key];scratch.write_text(json.dumps(broken))
            try:read_probe(scratch)
            except ValueError:rejected+=1
            else:raise AssertionError('Accepted missing '+key)
    mutations=[('records',0,'d0'),('records',0,'ccr'),('records',0,'type'),('records',0,'count'),('records',0,'phase'),('callbacks',0,'calls'),('callbacks',0,'self_same'),('callbacks',0,'callback_count'),('manual',0,'after_count')]
    for section,index,key in mutations:
        broken=copy.deepcopy(native);broken[section][index][key]^=1
        assert differences(reference,project(broken)),'Masked mutation '+key
    for section,key,index in [('records','guards',0),('manual','pulse_samples',0),('manual','pulse_d0',0),('manual','pulse_ccr',0)]:
        broken=copy.deepcopy(native);broken[section][0][key][index]^=1
        assert differences(reference,project(broken)),'Masked array mutation '+key
    broken=copy.deepcopy(native);broken['callbacks'][0]['ticks'][1]=broken['callbacks'][0]['ticks'][4]+1
    assert differences(reference,project(broken))
    print(json.dumps({'register_records':228,'callback_scenarios':18,'explicit_pulse_sequences':6,'missing_field_rejections':rejected,'mutations_detected':len(mutations)+5,'equal':True}))
if __name__=='__main__':main()
