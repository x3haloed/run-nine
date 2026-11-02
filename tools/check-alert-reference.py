#!/usr/bin/env python3
"""Verify complete native alert state and explicitly scoped absent-output calls."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def project_sound(raw):
    assert set(raw)=={'schema','probe','cases'} and raw['schema']==1 and raw['probe']=='sound-channels-raw-v1'
    assert len(raw['cases'])==9
    fields={'synth','init','input_ccr','error','ccr','stack_delta','stack_guard','channel_null','pointer_guards'}
    for i,row in enumerate(raw['cases']):
        assert set(row)==fields and row['input_ccr']==[16,31,0][i%3]
        assert row['synth']==(5 if i<3 else -1) and row['init']==[0,0x7a5a3921,0x74737421][i//3]
        assert row['error']==(0 if i<3 else -2004) and row['channel_null']==(0 if i<3 else 1)
    # The actual native default output is available. The independent runtime
    # explicitly has no default hardware/backend, so its default allocation
    # failure is an owned capability-policy check, NOT native equivalence.
    # Every field from the six nonexistent-component calls is retained exactly.
    return {'schema':1,'probe':'sound-no-output-v1','sound':{'cases':copy.deepcopy(raw['cases'][3:])}}
def main():
    for short in ['Alerts','SoundChannels']:
        pin=json.loads((OUT/(short+'-provenance.json')).read_text())
        for filename,expected in pin['pins'].items():assert digest(ROOT/filename)==expected,'Reference input changed: '+filename
        assert pin['installed_forks_match']
    fixture=ROOT/'probes/alerts/resources.rsrc';before=digest(fixture)
    subprocess.run(['python3',str(ROOT/'tools/build-alert-resources.py')],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    assert digest(fixture)==before,'Portable unit resources differ from native probe payloads'
    reference=read_probe(OUT/'AlertProbe-reference.json')
    sound=project_sound(json.loads((OUT/'SoundChannelProbe-reference.json').read_text()))
    path=OUT/'SoundNoOutputProbe-reference.json';path.write_text(json.dumps(sound,indent=2)+'\n');assert read_probe(path)==sound
    scratch=ROOT/'work/validation/alert-incomplete.json'
    for key in ['content','structure','text_hex','default_item','current_dialog','event_nonnull']:
        broken=copy.deepcopy(reference);del broken['alerts']['cases'][0]['callback'][key];scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:pass
        else:raise AssertionError('Accepted missing '+key)
    for key in ['ccr','stack_guard','channel_null','pointer_guards']:
        broken=copy.deepcopy(sound);del broken['sound']['cases'][0][key];scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:pass
        else:raise AssertionError('Accepted missing '+key)
    raw=json.loads((OUT/'SoundChannelProbe-reference.json').read_text());raw['cases'][3]['stack_guard']^=1
    assert differences(sound,project_sound(raw)),'Lost native failure stack overwrite'
    print(json.dumps({'alert_cases':16,'absent_component_cases':6,'native_success_cases_retained_separately':3,'missing_observation_rejections':10,'guard_mutation_detected':True,'normalization':'No alert normalization; all six selected sound-failure rows retain every field. Native default-success rows are outside the declared absent-backend profile.'}))
if __name__=='__main__':main()
