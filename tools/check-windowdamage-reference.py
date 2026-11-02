#!/usr/bin/env python3
"""Native exact window damage/event/update comparison through production code."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from damage_observations import project,FIELDS,REGIONS,ARRAYS,SCALARS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'WindowDamage-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed damage input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'WindowDamageProbe-v2-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','window-damage-raw-v2'],cwd=ROOT,text=True)
    (OUT/'WindowDamageProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,v in [('reference',native),('runtime',runtime)]:
        p=OUT/('WindowDamageProbe-'+label+'.json');p.write_text(json.dumps(v,indent=2)+'\n');assert read_probe(p)==v
    assert not differences(native,runtime),differences(native,runtime)
    missing=mutated=0;p=ROOT/'work/validation/windowdamage-incomplete.json'
    for k in FIELDS:
        bad=copy.deepcopy(native);del bad['windows']['records'][0][k];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+k)
    # All region fields remain byte-material, including translated empty bboxes.
    for i in range(0,96,3):
        for k in REGIONS:
            for j in range(2,len(native['windows']['records'][i][k])//2):
                bad=copy.deepcopy(native);r=bad['windows']['records'][i];b=bytearray.fromhex(r[k]);b[j]^=1;r[k]=b.hex();assert differences(native,bad);mutated+=1
        for k in SCALARS|ARRAYS:
            for j in range(4 if k in ARRAYS else 1):
                bad=copy.deepcopy(native);r=bad['windows']['records'][i]
                if k in ARRAYS:r[k][j]^=1
                else:r[k]^=1
                assert differences(native,bad);mutated+=1
    print(json.dumps({'records':96,'all_fields_exact':True,'normalization':False,'missing_fields_rejected':missing,'mutations_detected':mutated,'scope':'Region/input/port/ABI state and selected repeat-update event results/Begin-End visibility; no pixel or wall-clock timing equivalence claimed.'}))
if __name__=='__main__':main()
