#!/usr/bin/env python3
"""Exercise metadata-preserving export and rejection before destination creation."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
ROOT=Path(__file__).resolve().parents[1]
def main():
    base=ROOT/'work/validation';base.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='catalog-recovery-',dir=base) as temp:
        root=Path(temp);checkpoint=root/'checkpoint';checkpoint.mkdir()
        manifest={'format':'ClassicRuntimeRecovery','version':1,'id':9,'name':list(b'Original'),'finder':list(b'TEXTttxt'+bytes(8)),'data_bytes':4,'resource_bytes':3,'metadata':{'extended':list(range(16)),'dates':[0x12345678,0x12345679,0x1234567a]}}
        (checkpoint/'data').write_bytes(b'DATA');(checkpoint/'resource').write_bytes(b'RES');(checkpoint/'manifest.json').write_text(json.dumps(manifest))
        destination=root/'restored';tool=ROOT/'tools/recover-file.py'
        subprocess.run([sys.executable,str(tool),str(checkpoint),str(destination)],check=True,capture_output=True)
        record=json.loads((destination/'catalog.json').read_text())['files'][0]
        assert record=={k:manifest[k] for k in ['id','name','finder','metadata']}
        fork=destination/'files'/b'original'.hex();assert (fork/'data').read_bytes()==b'DATA' and (fork/'resource').read_bytes()==b'RES'
        for i,broken in enumerate([{'extended':[0]*15,'dates':[1,2,3]},{'extended':[0]*16,'dates':[1,-1,3]}]):
            manifest['metadata']=broken;(checkpoint/'manifest.json').write_text(json.dumps(manifest));target=root/f'invalid-{i}'
            run=subprocess.run([sys.executable,str(tool),str(checkpoint),str(target)],capture_output=True)
            assert run.returncode!=0 and not target.exists()
    result={'cases':3,'metadata_identity_and_both_forks_preserved':True,'invalid_metadata_refused_before_destination_creation':True}
    (ROOT/'outputs/Catalog-metadata-recovery-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
if __name__=='__main__':main()
