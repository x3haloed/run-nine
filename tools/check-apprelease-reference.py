#!/usr/bin/env python3
"""Compare actual production primary-resource lifecycles to native OS9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from apprelease_observations import validate,FIELDS,STRINGS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'AppRelease-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed application release input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    native=json.loads((OUT/'AppReleaseProbe-v1-reference.json').read_text());validate(native)
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','app-resource-release-v1'],cwd=ROOT,text=True)
    (OUT/'AppReleaseProbe-runtime.json').write_text(text);runtime=json.loads(text);validate(runtime)
    (OUT/'AppReleaseProbe-reference.json').write_text(json.dumps(native,indent=2)+'\n');assert read_probe(OUT/'AppReleaseProbe-reference.json')==native
    assert read_probe(OUT/'AppReleaseProbe-runtime.json')==runtime;assert not differences(native,runtime),differences(native,runtime)
    missing=mutated=0;p=ROOT/'work/validation/apprelease-incomplete.json'
    for key in sorted(FIELDS):
        bad=copy.deepcopy(native);del bad['resources']['records'][0][key];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
        bad=copy.deepcopy(native);r=bad['resources']['records'][0]
        if key in STRINGS:r[key]=r[key]+'00'
        else:r[key]^=1
        assert differences(native,bad),'Masked '+key;mutated+=1
    print(json.dumps({'records':12,'all_fields_exact':True,'normalization':False,'missing_fields_rejected':missing,'mutations_detected':mutated,'scope':'Primary cached resource ownership, actual map-entry clearing, release/reload and retained detach/plain bytes through production ABI.'}))
if __name__=='__main__':main()
