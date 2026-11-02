#!/usr/bin/env python3
"""Production eight-color assignment and RGB reader comparisons."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from legacycolor_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    reports=[]
    for name,pinfile,nativefile,probe in [('LegacyColorProbe','LegacyColor-provenance.json','LegacyColorProbe-v1-reference.json','legacy-color-runtime-raw-v1'),('ColorReadProbe','ColorRead-provenance.json','ColorReadProbe-v1-reference.json','color-read-runtime-raw-v1')]:
        pin=json.loads((OUT/pinfile).read_text())
        for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed native color input: '+f
        assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
        native=project(json.loads((OUT/nativefile).read_text()))
        text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe',probe],cwd=ROOT,text=True);(OUT/f'{name}-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
        for label,value in [('reference',native),('runtime',runtime)]:
            p=OUT/f'{name}-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
        assert not differences(native,runtime),differences(native,runtime)[:10]
        scratch=ROOT/'work/validation/legacycolor-incomplete.json';missing=mutations=0
        for key in native['colors']['records'][0]:
            bad=copy.deepcopy(native);del bad['colors']['records'][0][key];scratch.write_text(json.dumps(bad))
            try:read_probe(scratch)
            except ValueError:missing+=1
            else:raise AssertionError('Accepted missing '+key)
        for key in ['depth','records']:
            bad=copy.deepcopy(native);del bad['colors'][key];scratch.write_text(json.dumps(bad))
            try:read_probe(scratch)
            except ValueError:missing+=1
            else:raise AssertionError('Accepted missing '+key)
        for i,row in enumerate(runtime['colors']['records']):
            for key,old in list(row.items()):
                row[key]=[old[0]^1,*old[1:]]if isinstance(old,list)else old^1
                assert differences(native,runtime),'Masked color field';mutations+=1;row[key]=old
        reports.append({'probe':native['probe'],'records':192,'all_fields_bit_exact':True,'normalization':'None for retained named-color records. Raw assignment capture retains96 nonstandard-color cases outside documented-color support.'if name=='LegacyColorProbe'else'None.','missing_fields_rejected':missing,'mutations_detected':mutations})
    print(json.dumps(reports))
if __name__=='__main__':main()
