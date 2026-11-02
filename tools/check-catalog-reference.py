#!/usr/bin/env python3
"""Compare catalog roles without losing nonvolatile invalid IDs or sentinel bytes."""
import copy
import json
import hashlib
from pathlib import Path
from compare import read_probe,differences
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def project(raw):
    assert raw['schema']==1 and raw['probe']=='catalog-info-v1'
    assert set(raw)=={'schema','probe','raw_environment','files'}
    env=raw['raw_environment'];assert set(env)=={'volume','base','child','alpha_id'} and all(type(n) is int for n in env.values())
    rows=raw['files']['cases'];assert len(rows)==28
    identifier=lambda i:int.from_bytes(bytes.fromhex(rows[i]['body_hex'])[16:20],'big')
    ids=[env['base'],env['child'],env['alpha_id'],identifier(11),identifier(6)]
    assert len(set(ids))==5 and all(n>2 for n in ids)
    roles={n:i+3 for i,n in enumerate(ids)};roles.update({0:0,1:1,2:2})
    projected=[]
    for row in rows:
        body=bytearray.fromhex(row['body_hex']);assert len(body)==76
        obj=int.from_bytes(body[16:20],'big');parent=int.from_bytes(body[68:72],'big')
        pair=[roles.get(obj,-1),roles.get(parent,-1)]
        # Only known volatile catalog IDs are relocatable. Explicit bad IDs,
        # root constants and unmodified sentinel fields retain every raw byte.
        for at,role in [(16,pair[0]),(68,pair[1])]:
            if role>=3:body[at:at+4]=bytes(4)
        projected.append({**row,'body_hex':body.hex(),'identity_roles':pair})
    value={'schema':1,'probe':'catalog-semantics-v1','files':{'cases':projected}}
    return value

def main():
    pin=json.loads((OUT/'CatalogInfo-provenance.json').read_text())
    assert hashlib.sha256((ROOT/pin['source']).read_bytes()).hexdigest()==pin['source_sha256']
    assert hashlib.sha256((OUT/pin['reference']).read_bytes()).hexdigest()==pin['reference_sha256']
    raw=json.loads((OUT/'CatalogInfoProbe-reference.json').read_text())
    value=project(raw)
    assert bytes.fromhex(value['files']['cases'][8]['body_hex'])[16:20]==(999999).to_bytes(4,'big')
    assert bytes.fromhex(value['files']['cases'][0]['body_hex'])[68:72]==b'\x55'*4
    out=OUT/'CatalogSemanticsProbe-reference.json';out.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(out)==value
    scratch=ROOT/'work/validation/catalog-incomplete.json';keys=['body_hex','name33_hex','identity_roles','ref','attributes','guards']
    for key in keys:
        broken=copy.deepcopy(value);del broken['files']['cases'][0][key];scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:pass
        else:raise AssertionError('Accepted missing '+key)
    for case,at in [(8,16),(0,68)]:
        broken=copy.deepcopy(raw);body=bytearray.fromhex(broken['files']['cases'][case]['body_hex']);body[at]^=1;broken['files']['cases'][case]['body_hex']=body.hex()
        scratch.write_text(json.dumps(project(broken)))
        assert differences(read_probe(out),read_probe(scratch)),'Changed invalid ID/sentinel escaped projection/comparison'
    print(json.dumps({'cases':28,'missing_field_rejections':6,'nonvolatile_byte_mutations_detected':2,'scope':'Only5 observed catalog IDs relocated with explicit roles; all other record/name/sentinel/metadata bytes retained.'}))
if __name__=='__main__':main()
