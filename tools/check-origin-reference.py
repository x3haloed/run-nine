#!/usr/bin/env python3
"""Pin native SetOrigin captures and test strict comparison completeness."""
import copy,hashlib,json
from pathlib import Path
from compare import read_probe,differences
from origin_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'Origin-provenance.json').read_text())
    for file,sha in pin['pins'].items():assert hashlib.sha256((ROOT/file).read_bytes()).hexdigest()==sha,'Changed origin input: '+file
    assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    reference=project(json.loads((OUT/'OriginProbe-v2-reference.json').read_text()));path=OUT/'OriginProbe-reference.json';path.write_text(json.dumps(reference,indent=2)+'\n');assert read_probe(path)==reference
    scratch=ROOT/'work/validation/origin-incomplete.json';rejected=0
    for key in reference['ports']['cases'][0]:
        broken=copy.deepcopy(reference);del broken['ports']['cases'][0][key];scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:rejected+=1
        else:raise AssertionError('Accepted missing '+key)
    mutations=0
    for key in ['d0','ccr','stack_delta','current_same','pen','local_global','global_local','port_rect','bitmap','visible','clip']:
        broken=copy.deepcopy(reference);row=broken['ports']['cases'][0]
        if isinstance(row[key],list):row[key][0]^=1
        else:row[key]^=1
        assert differences(reference,broken),'Masked '+key;mutations+=1
    print(json.dumps({'contexts':16,'missing_field_rejections':rejected,'mutations_detected':mutations,'normalization':'none'}))
if __name__=='__main__':main()
