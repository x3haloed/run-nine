#!/usr/bin/env python3
"""Replay allocation/lifecycle and reject incomplete or masked observations."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from off_device_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pins=json.loads((OUT/'OffDevice-provenance.json').read_text())
    for f,sha in pins['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed offscreen input: '+f
    assert pins['installed_forks_match'] and pins['resource_rebuilt_byte_identical']
    native=json.loads((OUT/'OffDeviceProbe-v3-reference.json').read_text());reference=project(native)
    output=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','off-device-runtime-raw-v1'],cwd=ROOT,text=True);(OUT/'OffDeviceProbe-runtime-raw.json').write_text(output);candidate=project(json.loads(output))
    for name,data in [('reference',reference),('runtime',candidate)]:
        path=OUT/f'OffDeviceSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(reference,candidate),differences(reference,candidate)
    rejected=0;scratch=ROOT/'work/validation/offdevice-incomplete.json'
    for section in ['records','tables','switches','disposals']:
        for key in reference['devices'][section][0]:
            bad=copy.deepcopy(reference);del bad['devices'][section][0][key];scratch.write_text(json.dumps(bad))
            try:read_probe(scratch)
            except ValueError:rejected+=1
            else:raise AssertionError('Accepted missing '+key)
    mutations=0
    for section,key in [('records','d0'),('records','ccr'),('records','gd_size'),('records','pm_size'),('records','itable_size'),('records','ccdata_size'),('records','children_distinct'),('tables','replacement_distinct'),('switches','selected_same'),('switches','ccr'),('disposals','current_same'),('disposals','ccr')]:
        bad=copy.deepcopy(native);bad[section][0][key]^=1
        try:mutated=project(bad)
        except ValueError:pass
        else:assert differences(reference,mutated),'Masked '+key
        mutations+=1
    for key in ['gd_hex','pm_hex','ccdata_hex','ccmask_hex','table_hex']:
        bad=copy.deepcopy(native);b=bytearray.fromhex(bad['records'][0][key]);b[0]^=1;bad['records'][0][key]=b.hex();assert differences(reference,project(bad));mutations+=1
    bad=copy.deepcopy(native);b=bytearray.fromhex(bad['records'][0]['gd_hex']);b[6:10]=bytes(4);bad['records'][0]['gd_hex']=b.hex();assert differences(reference,project(bad));mutations+=1
    print(json.dumps({'allocations':12,'table_replacements':3,'switches':9,'disposals':12,'missing_field_rejections':rejected,'mutations_detected':mutations,'equal':True}))
if __name__=='__main__':main()
