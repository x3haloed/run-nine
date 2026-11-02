#!/usr/bin/env python3
"""Replay production MoveHHi and validate strict native relocation observations."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from move_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'MoveHHi-provenance.json').read_text())
    for file,sha in pin['pins'].items():assert hashlib.sha256((ROOT/file).read_bytes()).hexdigest()==sha,'Changed relocation input: '+file
    assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    raw=json.loads((OUT/'MoveHHiProbe-v4-reference.json').read_text());reference=project(raw)
    output=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','move-high-raw-v4'],cwd=ROOT,text=True)
    (OUT/'MoveHHiProbe-runtime-raw.json').write_text(output);candidate=project(json.loads(output))
    for name,data in [('reference',reference),('runtime',candidate)]:
        path=OUT/f'MoveHHiSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(reference,candidate),differences(reference,candidate)
    scratch=ROOT/'work/validation/move-incomplete.json';rejected=0
    for key in reference['memory']['cases'][0]:
        bad=copy.deepcopy(reference);del bad['memory']['cases'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:rejected+=1
        else:raise AssertionError('Accepted missing relocation '+key)
    for key in ['result','ccr','stack_delta','mem_error','state_after','guard_ok']:
        bad=copy.deepcopy(raw);bad['cases'][0][key]^=1;assert differences(reference,project(bad))
    # Keep the sum unchanged while corrupting two payload bytes: full-byte
    # comparison must catch what a checksum-only test would miss.
    bad=copy.deepcopy(raw);b=bytearray.fromhex(bad['cases'][0]['payload_hex']);b[0]+=1;b[1]-=1;bad['cases'][0]['payload_hex']=b.hex();assert differences(reference,project(bad))
    for key in ['other_hex','guard_hex']:
        bad=copy.deepcopy(raw);b=bytearray.fromhex(bad['cases'][0][key]);b[0]^=1;bad['cases'][0][key]=b.hex();assert differences(reference,project(bad))
    print(json.dumps({'native_cases':27,'missing_field_rejections':rejected,'mutation_checks':9,'full_payload_and_guard_bytes_compared':True,'scope':'Address-independent movement direction and exact API/data/state; unrelated unlocked payload relocation addresses are allocator-specific and retained raw.'}))
if __name__=='__main__':main()
