#!/usr/bin/env python3
"""Production PICT bitmap decoder/playback against complete native OS9 pixels."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import differences,read_probe
from pictbitmap_observations import project,FIELDS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'PICTBitmap-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed picture input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    subprocess.run(['python3',str(ROOT/'tools/build-pict-bitmap-fixtures.py'),'--check'],check=True,stdout=subprocess.DEVNULL)
    native=project(json.loads((OUT/'PICTBitmapProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','pict-bitmap-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'PICTBitmapProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,v in [('reference',native),('runtime',runtime)]:
        p=OUT/f'PICTBitmapProbe-{label}.json';p.write_text(json.dumps(v,indent=2)+'\n');assert read_probe(p)==v
    assert not differences(native,runtime),differences(native,runtime)[:10]
    missing=mutated=0;scratch=ROOT/'work/validation/pictbitmap-incomplete.json'
    for key in FIELDS:
        bad=copy.deepcopy(native);del bad['pictures']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
    for key in ['depth','row_bytes','destination_palette','records']:
        bad=copy.deepcopy(native);del bad['pictures'][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
    for key in ['seed','flags','count','rgb_hex']:
        bad=copy.deepcopy(native);del bad['pictures']['destination_palette'][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing palette '+key)
    for key in FIELDS:
        bad=copy.deepcopy(native);row=bad['pictures']['records'][0]
        if key=='pixels_hex':row[key]=format(int(row[key][:2],16)^1,'02x')+row[key][2:]
        elif key=='dest_rect':row[key][0]^=1
        else:row[key]^=1
        assert differences(native,bad),'Masked '+key;mutated+=1
    # Both ends of every destination block must participate in comparison.
    for i in range(480):
        for end in [False,True]:
            bad=copy.deepcopy(native);row=bad['pictures']['records'][i]
            if end:row['pixels_hex']=row['pixels_hex'][:-2]+format(int(row['pixels_hex'][-2:],16)^1,'02x')
            else:row['pixels_hex']=format(int(row['pixels_hex'][:2],16)^1,'02x')+row['pixels_hex'][2:]
            assert differences(native,bad),'Masked pixel edge';mutated+=1
    for key in ['seed','flags','count','rgb_hex']:
        bad=copy.deepcopy(native);palette=bad['pictures']['destination_palette']
        if key=='rgb_hex':palette[key]='0000'+palette[key][4:]
        else:palette[key]^=1
        assert differences(native,bad),'Masked palette '+key;mutated+=1
    print(json.dumps({'records':480,'pixel_bytes_compared':737280,'all_selected_fields_bit_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutated,'normalization':'Destination ColorSpec.value bookkeeping excluded; raw table retained, seed/flags/count/RGB and all record fields exact.','full_pict_equivalence':False}))
if __name__=='__main__':main()
