#!/usr/bin/env python3
"""Check independently collected addressing/owned-program inputs and observations."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    for short,name in [('Addressing','AddressingProbe'),('Owned68K','Owned68KProbe')]:
        pin=json.loads((OUT/(short+'-provenance.json')).read_text())
        assert digest(ROOT/pin['source'])==pin['source_sha256']
        assert digest(ROOT/pin['fixture'])==pin['resource_sha256']
        assert digest(OUT/pin['reference'])==pin['reference_sha256'] and pin['installed_forks_match']
        reference=read_probe(OUT/(name+'-reference.json'))
        scratch=ROOT/'work/validation/owned-incomplete.json';broken=copy.deepcopy(reference)
        if short=='Addressing':del broken['addressing']['cases'][0]['ccr']
        else:del broken['execution']['result_hex']
        scratch.write_text(json.dumps(broken))
        try:read_probe(scratch)
        except ValueError:pass
        else:raise AssertionError('Accepted missing execution observation')
    pin=json.loads((OUT/'Owned68K-provenance.json').read_text())
    assert digest(ROOT/pin['program_source'])==pin['program_source_sha256']
    assert digest(ROOT/pin['program'])==pin['program_sha256']
    subprocess.run(['python3',str(ROOT/'tools/build-owned68k.py')],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    assert digest(ROOT/pin['program'])==pin['program_sha256'],'Owned image does not rebuild identically'
    assert digest(OUT/'Owned68K-program.bin')==pin['program_sha256']
    assert not differences(read_probe(OUT/'Owned68KProbe-reference.json'),read_probe(OUT/'Owned68KProbe-native.json'))
    execution=json.loads((OUT/'Owned68K-host-execution.json').read_text())
    image=(ROOT/pin['program']).read_bytes()
    assert execution['instructions']==195 and execution['registers_preserved'] and execution['return_pc']==0x1800 and execution['return_sp']==0xf004
    assert [t['opcode'] for t in execution['traps']]==[0xa322,0xa025,0xa029,0xa02a,0xa024,0xa025,0xa029,0xa02a,0xa023,0xa025]
    for t in execution['traps']:
        assert int.from_bytes(image[t['pc']-0x1000:t['pc']-0x1000+2],'big')==t['opcode']
    print(json.dumps({'address_cases':12,'owned_program_bytes':len(image),'hosted_instructions':execution['instructions'],'trap_calls':10,'same_image_rebuilt':True,'native_result_matches_os9':True,'missing_observation_rejections':2}))
if __name__=='__main__':main()
