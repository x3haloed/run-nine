#!/usr/bin/env python3
"""Pin the native duplication matrix and verify strict observation fidelity."""
import copy,hashlib,json
from pathlib import Path
from compare import read_probe,differences
from clone_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    p=json.loads((OUT/'HandClone-provenance.json').read_text())
    for f,sha in p['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed clone input: '+f
    assert p['installed_forks_match'] and p['resource_rebuilt_byte_identical']
    reference=project(json.loads((OUT/'HandCloneProbe-v1-reference.json').read_text()));path=OUT/'HandCloneProbe-reference.json';path.write_text(json.dumps(reference,indent=2)+'\n');assert read_probe(path)==reference
    scratch=ROOT/'work/validation/clone-incomplete.json';rejected=0
    for key in reference['memory']['records'][0]:
        bad=copy.deepcopy(reference);del bad['memory']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:rejected+=1
        else:raise AssertionError('Accepted missing '+key)
    mutations=0
    for key in ['d0','ccr','stack_delta','mem_error','output_nonnull','distinct_source','source_size','copy_size','source_state','copy_state']:
        bad=copy.deepcopy(reference);bad['memory']['records'][27][key]^=1;assert differences(reference,bad);mutations+=1
    for key in ['source_hex','copy_hex','guard_hex']:
        bad=copy.deepcopy(reference);b=bytearray.fromhex(bad['memory']['records'][27][key]);b[0],b[1]=b[1],b[0];bad['memory']['records'][27][key]=b.hex();assert differences(reference,bad),'Masked checksum-preserving payload change';mutations+=1
    print(json.dumps({'cases':51,'missing_field_rejections':rejected,'mutations_detected':mutations,'normalization':'none'}))
if __name__=='__main__':main()
