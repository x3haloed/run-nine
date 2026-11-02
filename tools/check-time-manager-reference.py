#!/usr/bin/env python3
"""Replay real 68K TimerProc code and compare explicitly scoped observations."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from timer_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    provenance=json.loads((OUT/'TimeManager-provenance.json').read_text())
    for file,sha in provenance['pins'].items():assert hashlib.sha256((ROOT/file).read_bytes()).hexdigest()==sha,'Changed timer input: '+file
    assert provenance['installed_forks_match'] and provenance['resource_rebuilt_byte_identical']
    native=json.loads((OUT/'TimeManagerProbe-reference.json').read_text())
    output=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','time-manager-raw-v2'],cwd=ROOT,text=True)
    (OUT/'TimeManagerProbe-runtime-raw.json').write_text(output)
    reference=project(native);candidate=project(json.loads(output))
    for name,data in [('reference',reference),('runtime',candidate)]:
        path=OUT/f'TimeManagerSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(reference,candidate),differences(reference,candidate)
    rejected=0;scratch=ROOT/'work/validation/timer-incomplete.json'
    for section in ['records','callbacks','edges']:
        for key in reference['timers'][section][0]:
            broken=copy.deepcopy(reference);del broken['timers'][section][0][key];scratch.write_text(json.dumps(broken))
            try:read_probe(scratch)
            except ValueError:rejected+=1
            else:raise AssertionError('Accepted missing '+key)
    mutations=[('records',0,'ccr'),('records',0,'wake'),('records',1,'count'),('records',0,'type'),('callbacks',0,'calls'),('callbacks',0,'self_same'),('callbacks',0,'callback_count'),('edges',0,'prime_count')]
    for section,index,key in mutations:
        broken=copy.deepcopy(native);broken[section][index][key]^=1
        assert differences(reference,project(broken)),'Masked mutation '+key
    broken=copy.deepcopy(native);broken['records'][0]['guards'][0]^=1
    assert differences(reference,project(broken))
    broken=copy.deepcopy(native);broken['callbacks'][0]['ticks'][1]=broken['callbacks'][0]['ticks'][4]+1
    assert differences(reference,project(broken))
    broken=copy.deepcopy(native);broken['edges'][0]['remaining']=0
    assert differences(reference,project(broken))
    print(json.dumps({'register_records':18,'callback_scenarios':6,'large_delay_cases':8,'missing_field_rejections':rejected,'mutations_detected':len(mutations)+3,'equal':True}))
if __name__=='__main__':main()
