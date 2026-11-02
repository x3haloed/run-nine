#!/usr/bin/env python3
"""Compare production pixel-map services against measured native OS9."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from pixmap_observations import normalize,FIELDS,SNAP,ABI,POINTER_BYTES
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'PixMap-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed PixMap input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    raw=json.loads((OUT/'PixMapProbe-v1-reference.json').read_text());native=normalize(raw)
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','pixmap-raw-v1'],cwd=ROOT,text=True)
    (OUT/'PixMapProbe-runtime-raw.json').write_text(text);runtime=normalize(json.loads(text))
    for label,v in [('reference',native),('runtime',runtime)]:
        p=OUT/('PixMapProbe-'+label+'.json');p.write_text(json.dumps(v,indent=2)+'\n');assert read_probe(p)==v
    assert not differences(native,runtime),differences(native,runtime)
    missing=mutated=0;p=ROOT/'work/validation/pixmap-incomplete.json'
    paths=[('pixmaps','records',0,k)for k in FIELDS]+[('pixmaps','records',0,k,j)for k in ['initial','copied']for j in SNAP]+[('pixmaps','records',0,k,j)for k in ['new_abi','copy_abi','dispose_abi']for j in ABI]+[('pixmaps','nil_dispose',0,'abi',j)for j in ABI]
    for path in paths:
        bad=copy.deepcopy(native);r=bad
        for key in path[:-1]:r=r[key]
        del r[path[-1]];p.write_text(json.dumps(bad))
        try:read_probe(p)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted missing '+str(path))
    # Every retained public descriptor byte, palette byte and caller pixel byte
    # remains exact. Pointer roles are checked before canonicalization.
    for i,row in enumerate(native['pixmaps']['records']):
        for section,key,n,skip in [('initial','map_hex',50,POINTER_BYTES),('copied','map_hex',50,POINTER_BYTES),('initial','table_hex',row['initial']['table_size'],set()),('copied','table_hex',row['copied']['table_size'],set()),(None,'copy_table_after_source_mutation',row['copied']['table_size'],set()),(None,'base_bytes_after_dispose',64,set())]:
            for j in range(n):
                if j in skip:continue
                bad=copy.deepcopy(native);r=bad['pixmaps']['records'][i];r=r[section]if section else r;b=bytearray.fromhex(r[key]);b[j]^=1;r[key]=b.hex();assert differences(native,bad);mutated+=1
    for section,offset in [('initial',0),('copied',0),('initial',42),('copied',42)]:
        bad=copy.deepcopy(raw);s=bad['records'][0][section];b=bytearray.fromhex(s['map_hex']);b[offset:offset+4]=b'\0'*4;s['map_hex']=b.hex()
        try:normalize(bad)
        except ValueError:pass
        else:raise AssertionError('Masked invalid pointer')
    print(json.dumps({'records':6,'nil_dispose_cases':3,'all_retained_fields_exact':True,'normalized_fields':'Only baseAddr and pmTable pointer values; raw addresses retained and presence/alias roles checked.','missing_fields_rejected':missing,'byte_mutations_detected':mutated,'context32_scope':'Public current-device descriptor fixture only; no32bit display backend/rendering claim.'}))
if __name__=='__main__':main()
