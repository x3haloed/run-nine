#!/usr/bin/env python3
"""Compare actual ColorTable ABI/ownership/palette bytes against native OS9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from getctable_observations import normalize,validate,FIELDS,OWN,STRINGS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'GetCTable-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed color table input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    native=normalize(json.loads((OUT/'GetCTableProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','get-color-table-raw-v1'],cwd=ROOT,text=True)
    (OUT/'GetCTableProbe-runtime-raw.json').write_text(text);runtime=normalize(json.loads(text))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/('GetCTableProbe-'+label+'.json');p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)
    missing=mutated=0;p=ROOT/'work/validation/getctable-incomplete.json'
    for section,fields,index in [('records',FIELDS,42),('ownership',OWN,0)]:
        for key in sorted(fields):
            bad=copy.deepcopy(native);del bad['color'][section][index][key];p.write_text(json.dumps(bad))
            try:read_probe(p)
            except ValueError:missing+=1
            else:raise AssertionError('Accepted missing '+key)
            bad=copy.deepcopy(native);r=bad['color'][section][index]
            if key in STRINGS:r[key]=r[key][:-2]+f'{int(r[key][-2:],16)^1:02x}'
            else:r[key]^=1
            assert differences(native,bad),'Masked '+key;mutated+=1
    # A corrupt relative seed cannot be hidden by normalization.
    raw=json.loads((OUT/'GetCTableProbe-v1-reference.json').read_text());r=raw['records'][42];r['repeat_bytes']='00000000'+r['repeat_bytes'][8:]
    try:normalize(raw)
    except ValueError:pass
    else:raise AssertionError('Masked corrupted dynamic seed')
    print(json.dumps({'records':57,'ownership_cases':3,'all_selected_fields_exact':True,'normalization':'Only initial dynamic seed offset; all9 seed increments, stable seeds and remaining palette bytes exact. Raw captures retained.','missing_fields_rejected':missing,'mutations_detected':mutated}))
if __name__=='__main__':main()
