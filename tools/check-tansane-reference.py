#!/usr/bin/env python3
"""Validate real shared SANE calls, exact state and explicit extended error bound."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from tansane_observations import project,statistics
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'SANETan-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed SANE input: '+f
    assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    for script in ['build-sane-sine-operands.py']:subprocess.run(['python3',str(ROOT/'tools'/script),'--check'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    native=json.loads((OUT/'SANETanProbe-v1-reference.json').read_text());reference=project(native)
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','sane-tan-runtime-raw-v1'],cwd=ROOT,text=True);(OUT/'SANETanProbe-runtime-raw.json').write_text(text);raw=json.loads(text);candidate=project(raw)
    for name,data in [('reference',reference),('runtime',candidate)]:
        path=OUT/f'SANETanSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(reference,candidate),differences(reference,candidate)
    from importlib.machinery import SourceFileLoader
    audit=SourceFileLoader('tan_precision_audit',str(ROOT/'tools/audit-tan-precision.py')).load_module().audit(native,raw)
    assert not audit['runtime_errors'],audit['runtime_errors'];(OUT/'SANETan-high-precision-audit.json').write_text(json.dumps(audit,indent=2)+'\n')
    rejected=0;scratch=ROOT/'work/validation/tansane-incomplete.json'
    for section in ['controls','tan','dense']:
        for key in reference['math'][section][0]:
            bad=copy.deepcopy(reference);del bad['math'][section][0][key];scratch.write_text(json.dumps(bad))
            try:read_probe(scratch)
            except ValueError:rejected+=1
            else:raise AssertionError('Accepted missing SANE '+key)
    mutations=0
    for section,key in [('controls','word'),('controls','ccr'),('controls','d0'),('controls','stack_delta'),('tan','d0'),('tan','ccr'),('tan','output_env'),('tan','single_bits'),('tan','single_env'),('tan','stack_delta')]:
        bad=copy.deepcopy(reference);bad['math'][section][0][key]^=1;assert differences(reference,bad),'Masked '+key;mutations+=1
    for key in ['guards','single_guards']:
        bad=copy.deepcopy(reference);bad['math']['tan'][0][key][0]^=1;assert differences(reference,bad);mutations+=1
    for index,delta in [(90,2),(0,1),(24*45,1)]:
        bad=copy.deepcopy(reference);r=bad['math']['tan'][index];r['after_hex']=f"{int(r['after_hex'],16)+delta:020x}";assert differences(reference,bad),'Masked numeric/special corruption';mutations+=1
    bad=copy.deepcopy(reference);r=bad['math']['tan'][90];r['after_hex']=f"{int(r['after_hex'],16)+1:020x}";assert not differences(reference,bad),'Declared one-ULP bound rejected'
    print(json.dumps({'control_records':66,'tan_records':2790,'dense_records':2178,'missing_field_rejections':rejected,'mutations_detected':mutations,**statistics(reference,candidate),'oracle_audit':audit['counts']}))
if __name__=='__main__':main()
