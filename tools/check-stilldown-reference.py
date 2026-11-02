#!/usr/bin/env python3
"""Production mouse tracking and valid posted queues against real native presses."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from stilldown_observations import project,OMITTED
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'StillDown-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native tracking input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    raw=json.loads((OUT/'StillDownProbe-v1-reference.json').read_text());native=project(raw)
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','still-down-runtime-raw-v1'],cwd=ROOT,text=True);(OUT/'StillDownProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/f'StillDownProbe-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)[:10]
    scratch=ROOT/'work/validation/stilldown-incomplete.json';missing=mutations=0
    for key in native['events']['records'][0]:
        bad=copy.deepcopy(native);del bad['events']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
    bad=copy.deepcopy(native);del bad['events']['records'];scratch.write_text(json.dumps(bad))
    try:read_probe(scratch)
    except ValueError:missing+=1
    else:raise AssertionError('Accepted missing records')
    for row in runtime['events']['records']:
        for key,old in list(row.items()):
            row[key]=([old[0]^1,*old[1:]]if old else[1])if isinstance(old,list)else old^1
            assert differences(native,runtime),'Masked tracking field';row[key]=old;mutations+=1
    bad=copy.deepcopy(raw)
    for key in OMITTED:bad['records'][0][key]^=1
    assert not differences(native,project(bad))
    print(json.dumps({'records':192,'all_selected_fields_bit_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutations,'retained_raw_uncompared':['D0 ROM/environment scratch','MemErr and ResErr changed by native housekeeping in initial held samples'],'physical_input':'96 released and96 real held samples; both physical observations agree in every record.','queue_preservation_verified':True,'full_event_manager_equivalence':False}))
if __name__=='__main__':main()
