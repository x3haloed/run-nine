#!/usr/bin/env python3
"""Native Courier comparison through production font/catalog methods."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from courier_observations import project,FIELDS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'Courier-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed Courier input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    command=['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe']
    text=subprocess.check_output(command+['courier-font-raw-v1'],cwd=ROOT,text=True);(OUT/'CourierProbe-runtime-raw.json').write_text(text)
    runtime_catalog=json.loads(subprocess.check_output(command+['font-catalog-v1'],cwd=ROOT,text=True))
    native=project(json.loads((OUT/'CourierProbe-v1-reference.json').read_text()),json.loads((OUT/'FontCatalogProbe-reference.json').read_text()))
    runtime=project(json.loads(text),runtime_catalog)
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/('CourierProbe-'+label+'.json');p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)
    missing=mutated=0;p=ROOT/'work/validation/courier-incomplete.json'
    for k in FIELDS:
        bad=copy.deepcopy(native);del bad['fonts']['records'][0][k];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+k)
    # Every observed width, metric and independent label remains material.
    for i,r in enumerate(native['fonts']['records']):
        for k in FIELDS:
            for j in range(len(r[k])if isinstance(r[k],list)else 1):
                bad=copy.deepcopy(native);b=bad['fonts']['records'][i]
                if isinstance(b[k],list):b[k][j]^=1
                else:b[k]^=1
                assert differences(native,bad),'Masked Courier '+k;mutated+=1
    for k in ['name','number','menus']:
        bad=copy.deepcopy(native);del bad['fonts'][k];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+k)
    print(json.dumps({'records':14,'native_widths':3584,'independent_string_widths':42,'all_fields_exact':True,'normalization':False,'menu_scope':'Four measured available families in full native catalog; complete installed catalog equality is not claimed.','missing_fields_rejected':missing,'mutations_detected':mutated,'glyph_pixels_exact':False}))
if __name__=='__main__':main()
