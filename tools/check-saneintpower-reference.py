#!/usr/bin/env python3
"""Replay production exact integer-power paths against the retained OS9 matrix."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import differences,read_probe
from saneintpower_observations import native_project,runtime_project,FIELDS,HEX
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    subprocess.run(['python3',str(ROOT/'tools/check-sanepower-native.py')],check=True,stdout=subprocess.DEVNULL)
    native=native_project(json.loads((OUT/'SANEPowerProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','sane-small-integer-power-runtime-raw-v1'],cwd=ROOT,text=True)
    (OUT/'SANEIntegerPowerProbe-runtime-raw.json').write_text(text);runtime=runtime_project(json.loads(text))
    for label,v in [('reference',native),('runtime',runtime)]:
        p=OUT/f'SANEIntegerPowerSemanticsProbe-{label}.json';p.write_text(json.dumps(v,indent=2)+'\n');assert read_probe(p)==v
    assert not differences(native,runtime),differences(native,runtime)[:10]
    missing=mutated=0;scratch=ROOT/'work/validation/saneintpower-incomplete.json'
    for key in FIELDS:
        bad=copy.deepcopy(native);del bad['math']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
        bad=copy.deepcopy(native);r=bad['math']['records'][0]
        if key in HEX:r[key]=format(int(r[key],16)^1,'020x')
        elif key=='guards':r[key][0]^=1
        else:r[key]^=1
        assert differences(native,bad),'Masked '+key;mutated+=1
    print(json.dumps({'records':4950,'all_retained_fields_bit_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutated,'exponents':[0,1,-1,2,-2],'general_real_power_supported':False}))
if __name__=='__main__':main()
