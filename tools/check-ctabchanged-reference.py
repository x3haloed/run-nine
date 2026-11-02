#!/usr/bin/env python3
"""Production QDExtensions color notification matches native OS9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from ctabchanged_observations import normalize,FIELDS
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'CTabChanged-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed notification input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    raw=json.loads((OUT/'CTabChangedProbe-v1-reference.json').read_text());native=normalize(raw)
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','ctab-changed-raw-v1'],cwd=ROOT,text=True)
    (OUT/'CTabChangedProbe-runtime-raw.json').write_text(text);runtime=normalize(json.loads(text))
    for label,v in [('reference',native),('runtime',runtime)]:
        p=OUT/('CTabChangedProbe-'+label+'.json');p.write_text(json.dumps(v,indent=2)+'\n');assert read_probe(p)==v
    assert not differences(native,runtime),differences(native,runtime)
    missing=mutated=corrupt=0;p=ROOT/'work/validation/ctabchanged-incomplete.json'
    for k in FIELDS:
        bad=copy.deepcopy(native);del bad['color']['records'][0][k];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+k)
    for i,row in enumerate(native['color']['records']):
        for k in FIELDS-{'before_hex','after_hex'}:
            bad=copy.deepcopy(native);bad['color']['records'][i][k]^=1;assert differences(native,bad);mutated+=1
        for k in ['before_hex','after_hex']:
            for j in range(4 if k=='after_hex'else 0,row['size']):
                bad=copy.deepcopy(native);r=bad['color']['records'][i];b=bytearray.fromhex(r[k]);b[j]^=1;r[k]=b.hex();assert differences(native,bad);mutated+=1
        for k in ['base','after','after_hex']:
            bad=copy.deepcopy(raw);r=bad['records'][i]
            if k=='after_hex':r[k]='00000000'+r[k][8:]
            else:r[k]^=1
            try:normalize(bad)
            except ValueError:corrupt+=1
            else:raise AssertionError('Masked corrupt notification counter')
    print(json.dumps({'records':6,'all_retained_fields_exact':True,'normalization':'Initial dynamic offset only; shared counter links, all other table bytes, handle/error/CPU state exact.','missing_fields_rejected':missing,'mutations_detected':mutated,'corrupt_counter_links_rejected':corrupt}))
if __name__=='__main__':main()
