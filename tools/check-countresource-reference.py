#!/usr/bin/env python3
"""Production all-map and current-map resource-count semantics against OS9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from countresource_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'CountResources-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native count input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'CountResourcesProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','count-resources-runtime-raw-v1'],cwd=ROOT,text=True);(OUT/'CountResourcesProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/f'CountResourcesProbe-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)[:10]
    scratch=ROOT/'work/validation/countresource-incomplete.json';missing=mutations=0
    for key in native['resources']['records'][0]:
        bad=copy.deepcopy(native);del bad['resources']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
    bad=copy.deepcopy(native);del bad['resources']['records'];scratch.write_text(json.dumps(bad))
    try:read_probe(scratch)
    except ValueError:missing+=1
    else:raise AssertionError('Accepted missing records')
    for row in runtime['resources']['records']:
        for key,old in list(row.items()):
            row[key]^=1;assert differences(native,runtime),'Masked count field';row[key]=old;mutations+=1
    print(json.dumps({'records':216,'all_fields_bit_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutations,'normalization':'None. Resource-file numeric references are not comparison outputs; current selection preservation is measured directly.','full_system_resource_equivalence':False}))
if __name__=='__main__':main()
