#!/usr/bin/env python3
"""Real Color Manager replay, complete selected public state and strict checks."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from inverse_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'Inverse-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed inverse input: '+f
    assert pin['installed_forks_match'] and pin['resource_rebuilt_byte_identical']
    native=json.loads((OUT/'InverseProbe-v3-reference.json').read_text());reference=project(native)
    output=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','inverse-runtime-raw-v1'],cwd=ROOT,text=True);(OUT/'InverseProbe-runtime-raw.json').write_text(output);candidate=project(json.loads(output))
    for name,data in [('reference',reference),('runtime',candidate)]:
        path=OUT/f'InverseSemanticsProbe-{name}.json';path.write_text(json.dumps(data,indent=2)+'\n');assert read_probe(path)==data
    assert not differences(reference,candidate),differences(reference,candidate)
    scratch=ROOT/'work/validation/inverse-incomplete.json';rejected=0
    for key in reference['color']['records'][0]:
        bad=copy.deepcopy(reference);del bad['color']['records'][0][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:rejected+=1
        else:raise AssertionError('Accepted missing inverse '+key)
    for key in reference['color']['records'][0]['port']:
        bad=copy.deepcopy(reference);del bad['color']['records'][0]['port'][key];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:rejected+=1
        else:raise AssertionError('Accepted missing port '+key)
    mutations=0
    for key in ['stack_delta','qd_error','current_same','list_same']:
        bad=copy.deepcopy(native);bad['records'][0][key]^=1;assert differences(reference,project(bad));mutations+=1
    for key,offset in [('before_hex',8),('after_hex',8),('itable_hex',0),('itable_hex',6),('guard_hex',0)]:
        bad=copy.deepcopy(native);b=bytearray.fromhex(bad['records'][0][key]);b[offset]^=1;bad['records'][0][key]=b.hex();assert differences(reference,project(bad));mutations+=1
    for key in ['indices','exact_indices']:
        bad=copy.deepcopy(native);bad['records'][0][key][0]^=1;assert differences(reference,project(bad));mutations+=1
    for key in ['row_bytes','pixel_size','base_same','table_same','map_distinct']:
        bad=copy.deepcopy(native);bad['records'][0]['port'][key]^=1;assert differences(reference,project(bad));mutations+=1
    for key in ['port_rect','bitmap_bounds','pen_indices','rgb_fg','rgb_bg']:
        bad=copy.deepcopy(native);bad['records'][0]['port'][key][0]^=1;assert differences(reference,project(bad));mutations+=1
    # These variations are deliberately outside the declared comparison domain.
    bad=copy.deepcopy(native);bad['records'][0]['d0']^=1;bad['records'][0]['ccr']^=1;b=bytearray.fromhex(bad['records'][0]['itable_hex']);b[-1]^=1;bad['records'][0]['itable_hex']=b.hex();assert not differences(reference,project(bad))
    print(json.dumps({'contexts':96,'public_grid_cells':96*512,'color_queries':96*512,'exact_color_queries':sum(len(r['exact_indices'])for r in native['records']),'missing_field_rejections':rejected,'mutations_detected':mutations,'equal':True,'deliberately_uncompared':['void MakeITable caller-scratch D0/CCR','518 bytes of private inverse cache/padding']}))
if __name__=='__main__':main()
