#!/usr/bin/env python3
"""Pin and compare controlled Window Manager versus application background RGB."""
import copy,hashlib,json
from pathlib import Path
from compare import read_probe,differences
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'WindowBackground-provenance.json').read_text())
    for file,sha in pin['pins'].items():assert hashlib.sha256((ROOT/file).read_bytes()).hexdigest()==sha,'Changed background input: '+file
    assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    raw=json.loads((OUT/'WindowBackgroundProbe-native-raw.json').read_text());assert raw['restore_error']==0;reference={k:v for k,v in raw.items() if k!='restore_error'}
    path=OUT/'WindowBackgroundProbe-reference.json';path.write_text(json.dumps(reference,indent=2)+'\n');assert read_probe(path)==reference
    scratch=ROOT/'work/validation/window-background-incomplete.json';rejected=0
    for key in reference['windows']['cases'][0]:
        bad=copy.deepcopy(reference);del bad['windows']['cases'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:rejected+=1
        else:raise AssertionError('Accepted missing background '+key)
    for key in ['seed_rgb','shown_rgb','port_bg','erased_rgb','after_set_rgb','reshown_rgb']:
        bad=copy.deepcopy(reference);bad['windows']['cases'][0][key][0]^=1;assert differences(reference,bad)
    print(json.dumps({'contexts':4,'rgb_samples':24,'missing_field_rejections':rejected,'mutation_checks':6,'scope':'Controlled seeded pixels, standard-window background/table versus app RGB, show/erase/set/hide/show; no frame-art/pixel-universal claim.'}))
if __name__=='__main__':main()
