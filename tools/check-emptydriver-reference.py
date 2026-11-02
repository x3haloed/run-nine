#!/usr/bin/env python3
"""Native bit-exact shared Device Manager lookup/error comparison."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from driver_observations import project,FIELDS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'EmptyDriver-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed driver input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    subprocess.run(['cargo','test','--quiet','--manifest-path',str(ROOT/'compat-core/Cargo.toml'),'--features','hosted-cpu','drivers::tests'],cwd=ROOT,check=True,capture_output=True,text=True)
    native=project(json.loads((OUT/'EmptyDriverProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','empty-driver-raw-v1'],cwd=ROOT,text=True)
    (OUT/'EmptyDriverProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,v in [('reference',native),('runtime',runtime)]:
        p=OUT/('EmptyDriverProbe-'+label+'.json');p.write_text(json.dumps(v,indent=2)+'\n');assert read_probe(p)==v
    assert not differences(native,runtime),differences(native,runtime)
    missing=mutated=0;p=ROOT/'work/validation/emptydriver-incomplete.json'
    for key in FIELDS:
        bad=copy.deepcopy(native);del bad['drivers']['records'][0][key];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
    for i in range(0,192,12):
        for key in FIELDS-{'pb_hex'}:
            bad=copy.deepcopy(native);bad['drivers']['records'][i][key]^=1;assert differences(native,bad);mutated+=1
        for j in range(50):
            bad=copy.deepcopy(native);r=bad['drivers']['records'][i];b=bytearray.fromhex(r['pb_hex']);b[j]^=1;r['pb_hex']=b.hex();assert differences(native,bad);mutated+=1
    bad=copy.deepcopy(native);del bad['drivers']['unit_count'];p.write_text(json.dumps(bad))
    try:read_probe(p)
    except ValueError:missing+=1
    else:raise AssertionError('Missing unit count accepted')
    print(json.dumps({'records':192,'all_fields_exact':True,'normalization':False,'missing_fields_rejected':missing,'mutations_detected':mutated,'shared_core_tests':2,'scope':'Synchronous missing/range errors against public table; occupied slots remain explicit unsupported dispatch.'}))
if __name__=='__main__':main()
