#!/usr/bin/env python3
"""Check completeness/provenance of the native-only directory/path capture."""
import hashlib
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    pin=json.loads((OUT/'PathFiles-provenance.json').read_text())
    source=ROOT/pin['source'];reference=OUT/pin['reference']
    assert sha(source)==pin['source_sha256'],'Native source differs from built pin'
    assert sha(reference)==pin['reference_sha256'],'Native capture differs from pin'
    raw=json.loads(reference.read_text())
    assert set(raw)=={'schema','probe','raw_environment','files'}
    assert raw['schema']==1 and raw['probe']=='path-files-v1'
    environment=raw['raw_environment']
    assert set(environment)=={'volume','base','child','other','original_dir'}
    assert all(type(n) is int for n in environment.values())
    assert len({environment[k] for k in ['base','child','other']})==3
    rows=raw['files']['cases'];assert len(rows)==28
    keys={'case','spec_error','spec_volume_same','spec_volume_zero','spec_parent','spec_name_hex','open_error','failed_ref_preserved','read'}
    for i,row in enumerate(rows):
        assert set(row)==keys and row['case']==i
        assert all(type(row[k]) is int for k in keys-{'read','spec_name_hex'})
        assert row['spec_volume_same'] in (0,1) and row['spec_volume_zero'] in (0,1)
        assert row['failed_ref_preserved'] in (0,1)
        assert isinstance(row['spec_name_hex'],str) and len(bytes.fromhex(row['spec_name_hex']))<=63
        assert isinstance(row['read'],list) and len(row['read'])==6 and all(type(n) is int for n in row['read'])
        if row['spec_error'] not in (0,-43):
            assert row['spec_volume_zero']==1 and row['spec_parent']==0 and row['spec_name_hex']==''
        if row['open_error']==0:assert row['read'][:2]==[0,4]
        else:assert row['read']==[12345,4,85,85,85,85]
    assert bytes.fromhex(rows[8]['spec_name_hex'])==b'Missing' and rows[8]['spec_error']==-43 and rows[8]['spec_parent']==4
    assert rows[9]['spec_error']==-120 and rows[20]['spec_error']==0
    assert rows[0]['read'][2:]==list(b'BASE') and rows[20]['read'][2:]==list(b'CHLD')
    result={'schema':1,'cases':28,'raw_capture_shape_and_pins_valid':True,'native_only':True,'runtime_comparison_count_added':0,'scope':'Native observations only. Strict differential replay is pending independent production directory/path behavior.'}
    (OUT/'PathFiles-native-validation.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))
if __name__=='__main__':main()
