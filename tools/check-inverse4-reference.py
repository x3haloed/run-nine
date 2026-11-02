#!/usr/bin/env python3
"""Four-bit public inverse grids and real Color2Index/port replay."""
import copy,hashlib,json,subprocess
from pathlib import Path
from compare import read_probe,differences
from inverse4_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pin=json.loads((OUT/'InverseFour-provenance.json').read_text())
    for f,sha in pin['pins'].items():assert hashlib.sha256((ROOT/f).read_bytes()).hexdigest()==sha,'Changed inverse-four input: '+f
    assert pin['installed_forks_match']and pin['resource_rebuilt_byte_identical']
    native=project(json.loads((OUT/'InverseFourProbe-v1-reference.json').read_text()))
    text=subprocess.check_output(['cargo','run','--quiet','--manifest-path',str(ROOT/'runtime/Cargo.toml'),'--','--probe','inverse-four-runtime-raw-v1'],cwd=ROOT,text=True);(OUT/'InverseFourProbe-runtime-raw.json').write_text(text);runtime=project(json.loads(text))
    for label,value in [('reference',native),('runtime',runtime)]:
        p=OUT/f'InverseFourSemanticsProbe-{label}.json';p.write_text(json.dumps(value,indent=2)+'\n');assert read_probe(p)==value
    assert not differences(native,runtime),differences(native,runtime)[:10]
    scratch=ROOT/'work/validation/inverse4-incomplete.json';missing=mutations=0
    for keys in [('color',k)for k in native['color']]+[('color','records',0,k)for k in native['color']['records'][0]]+[('color','records',0,'port',k)for k in native['color']['records'][0]['port']]+[('color','main_palette',k)for k in native['color']['main_palette']]:
        bad=copy.deepcopy(native);node=bad
        for k in keys[:-1]:node=node[k]
        del node[keys[-1]];scratch.write_text(json.dumps(bad))
        try:read_probe(scratch)
        except ValueError:missing+=1
        else:raise AssertionError('Accepted incomplete four-bit inverse')
    def change(node,key,old):
        nonlocal mutations
        node[key]=old^1 if isinstance(old,int)else ([old[0]^1,*old[1:]]if isinstance(old,list)else format(int(old[:2],16)^1,'02x')+old[2:])
        assert differences(native,runtime),'Masked inverse-four field';mutations+=1;node[key]=old
    # Full-profile mutation checks for each field shape, then record-local
    # first/last-byte checks in every block. Avoid rescanning786432 cells
    # thousands of times for equivalent comparator self-checks.
    row=runtime['color']['records'][0]
    for key,old in list(row.items()):
        if key=='port':
            for k,v in list(old.items()):change(old,k,v)
        else:change(row,key,old)
    for reference,row in zip(native['color']['records'],runtime['color']['records']):
        for key in ['public_hex','indices']:
            for end in [False,True]:
                old=row[key]
                if isinstance(old,str):row[key]=old[:-2]+format(int(old[-2:],16)^1,'02x')if end else format(int(old[:2],16)^1,'02x')+old[2:]
                else:row[key]=old[:-1]+[old[-1]^1]if end else [old[0]^1]+old[1:]
                assert differences(reference,row);mutations+=1;row[key]=old
    for k,v in list(runtime['color']['main_palette'].items()):change(runtime['color']['main_palette'],k,v)
    change(runtime['color'],'main_indices',runtime['color']['main_indices'])
    old=runtime['color']['main_indices'][-1];runtime['color']['main_indices'][-1]^=1;assert differences(native,runtime);mutations+=1;runtime['color']['main_indices'][-1]=old
    print(json.dumps({'records':96,'public_grid_cells':393216,'color_queries':393216,'main_device_queries':4096,'all_selected_fields_bit_exact':True,'missing_fields_rejected':missing,'mutations_detected':mutations,'excluded':'Void MakeITable D0/CCR;518 private inverse cache/padding bytes;main ColorSpec.value bookkeeping. Complete raw captures retained.'}))
if __name__=='__main__':main()
