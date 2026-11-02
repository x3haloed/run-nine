#!/usr/bin/env python3
"""Production ResourceManager/QuickDraw share the measured color counter."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from ctseed_observations import normalize,FIELDS,DYNAMIC
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'CTSeed-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed color-seed input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    raw=json.loads((OUT/'CTSeedProbe-v1-reference.json').read_text());native=normalize(raw)
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','color-seed-raw-v1'],cwd=ROOT,text=True)
    (OUT/'CTSeedProbe-runtime-raw.json').write_text(text);runtime=normalize(json.loads(text))
    for label,v in [('reference',native),('runtime',runtime)]:
        p=OUT/('CTSeedProbe-'+label+'.json');p.write_text(json.dumps(v,indent=2)+'\n');assert read_probe(p)==v
    assert not differences(native,runtime),differences(native,runtime)
    missing=mutated=corrupt=0;p=ROOT/'work/validation/ctseed-incomplete.json'
    for k in FIELDS:
        bad=copy.deepcopy(native);del bad['color']['records'][0][k];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+k)
    for i in range(6):
        for k in FIELDS:
            for j in range(3 if k=='device_rgb'else 1):
                bad=copy.deepcopy(native);r=bad['color']['records'][i]
                if k=='device_rgb':r[k][j]^=1
                else:r[k]^=1
                assert differences(native,bad);mutated+=1
        for k in DYNAMIC+['device_before']:
            bad=copy.deepcopy(raw);bad['records'][i][k]^=1
            try:normalize(bad)
            except ValueError:corrupt+=1
            else:raise AssertionError('Masked corrupted shared counter '+k)
    print(json.dumps({'records':6,'dynamic_values':36,'all_selected_fields_exact':True,'normalization':'Initial dynamic offset only; all increments/device links, stable8seeds, ABI/guards and RGBs exact. Raw captures retained.','missing_fields_rejected':missing,'mutations_detected':mutated,'corrupted_sequences_rejected':corrupt}))
if __name__=='__main__':main()
