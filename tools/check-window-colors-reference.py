#!/usr/bin/env python3
"""Verify native window color provenance and complete selected public state."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from window_color_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pins=json.loads((OUT/'WindowColors-provenance.json').read_text())
    for file,sha in pins['pins'].items():assert hashlib.sha256((ROOT/file).read_bytes()).hexdigest()==sha,'Changed window color input: '+file
    assert pins['installed_forks_match'] and pins['resource_rebuilt_byte_identical']
    fixture=ROOT/'probes/windowcolors/resources.rsrc';before=hashlib.sha256(fixture.read_bytes()).hexdigest()
    subprocess.run(['python3',str(ROOT/'tools/build-window-colors-resources.py')],cwd=ROOT,check=True,stdout=subprocess.DEVNULL);assert hashlib.sha256(fixture.read_bytes()).hexdigest()==before
    raw=json.loads((OUT/'WindowColorsProbe-v3-reference.json').read_text());reference=project(raw);path=OUT/'WindowColorsProbe-reference.json';path.write_text(json.dumps(reference,indent=2)+'\n');assert read_probe(path)==reference
    scratch=ROOT/'work/validation/window-colors-incomplete.json';rejected=0
    for index in [0,3]:
        for key in reference['windows']['cases'][index]:
            bad=copy.deepcopy(reference);del bad['windows']['cases'][index][key];scratch.write_text(json.dumps(bad))
            try:read_probe(scratch)
            except ValueError:rejected+=1
            else:raise AssertionError('Accepted missing window color '+key)
    for location in [[0,'found'],[0,'owner_same'],[1,'table_source_same'],[8,'aux_flags'],[0,'bg',0],[1,'table',0,1],[10,'structure',0],[3,'erased_rgb',0]]:
        bad=copy.deepcopy(raw);target=bad['cases']
        for key in location[:-1]:target=target[key]
        target[location[-1]]^=1;assert differences(reference,project(bad))
    print(json.dumps({'window_variants':3,'state_snapshots':9,'erased_rgb_cases':3,'missing_field_rejections':rejected,'mutation_checks':8,'normalization':'None within selected state. Native cleanup restore result checked separately. Whole C startup, theme frame pixels and unobserved window operations are outside this replay.'}))
if __name__=='__main__':main()
