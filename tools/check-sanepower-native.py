#!/usr/bin/env python3
"""Verify pinned native power observations without claiming runtime support."""
import copy,hashlib,json,subprocess
from pathlib import Path
from sanepower_observations import project,validate,FIELDS
ROOT=Path(__file__).resolve().parents[1]
def main():
    pin=json.loads((ROOT/'outputs/SANEPower-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native power input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    subprocess.run(['python3',str(ROOT/'tools/build-sane-power-operands.py'),'--check'],check=True,stdout=subprocess.DEVNULL)
    v=project(json.loads((ROOT/'outputs/SANEPowerProbe-v1-reference.json').read_text()));rows=v['math']['records']
    for r in rows:
        assert r['d0']==0x12345678 and r['stack_delta']==0
        assert r['guards']==[0x11223344,0x55667788,0x99aabbcc,0xddeeff00]
        assert r['source_before_hex']==r['source_after_hex']and r['ccr']in[0,16]
    for i in range(220):assert len({r['dest_after_hex']for r in rows[i*45:(i+1)*45]})==1
    missing=0
    for key in FIELDS:
        bad=copy.deepcopy(v);del bad['math']['records'][0][key]
        try:validate(bad)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+key)
    print(json.dumps({'records':len(rows),'native_only':True,'operand_stack_register_guards_verified':True,'base_pairs_numerically_independent_of_selected_environment':220,'missing_fields_rejected':missing,'runtime_power_implemented':False}))
if __name__=='__main__':main()
