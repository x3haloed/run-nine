#!/usr/bin/env python3
"""Pin native GetMaxDevice and validate complete role-based observations."""
import copy,hashlib,json
from pathlib import Path
from compare import read_probe,differences
from device_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'MaxDevice-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed device input: '+f
    assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    reference=project(json.loads((OUT/'MaxDeviceProbe-v1-reference.json').read_text()));path=OUT/'MaxDeviceProbe-reference.json';path.write_text(json.dumps(reference,indent=2)+'\n');assert read_probe(path)==reference
    scratch=ROOT/'work/validation/device-incomplete.json';rejected=0
    for key in reference['devices']['cases'][0]:
        broken=copy.deepcopy(reference);del broken['devices']['cases'][0][key];scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:rejected+=1
        else:raise AssertionError('Accepted missing '+key)
    mutations=0
    for key in ['d0','ccr','stack_delta','selected_role','main_same','null','current_same','rect']:
        bad=copy.deepcopy(reference);row=bad['devices']['cases'][0]
        if key=='rect':row[key][0]^=1
        else:row[key]^=1
        assert differences(reference,bad),'Masked '+key;mutations+=1
    print(json.dumps({'contexts':36,'missing_field_rejections':rejected,'mutations_detected':mutations,'normalization':'opaque device handle to nil/main/other; all remaining selected state exact'}))
if __name__=='__main__':main()
