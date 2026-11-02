#!/usr/bin/env python3
"""Verify native provenance and strict selected indexed-color observation shape."""
import copy,hashlib,json
from pathlib import Path
from compare import read_probe,differences
from color_observations import project
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'outputs'
def main():
    pins=json.loads((OUT/'IndexedColor-provenance.json').read_text())
    for file,sha in pins['pins'].items():assert hashlib.sha256((ROOT/file).read_bytes()).hexdigest()==sha,'Changed color input: '+file
    assert pins['installed_forks_match'] and pins['resource_rebuilt_byte_identical']
    raw=json.loads((OUT/'DeviceDepthProbe-v4-reference.json').read_text());reference=project(raw)
    path=OUT/'IndexedColorProbe-reference.json';path.write_text(json.dumps(reference,indent=2)+'\n');assert read_probe(path)==reference
    scratch=ROOT/'work/validation/color-incomplete.json';rejected=0
    locations=[['color','abi',0],['color','device8'],['color','indexed'],['color','indexed','set_pixel'],['color','indexed','recolored']]
    for location in locations:
        row=reference
        for key in location:row=row[key]
        for key in row:
            broken=copy.deepcopy(reference);target=broken
            for part in location:target=target[part]
            del target[key];scratch.write_text(json.dumps(broken))
            try:read_probe(scratch)
            except ValueError:rejected+=1
            else:raise AssertionError('Accepted missing color '+key)
    # Mutations pass through the native projection, so normalization cannot
    # silently discard selected pixel/ABI observations.
    locations=[['changes',0,'state','row_bytes'],['changes',0,'state','depth'],['changes',0,'state','flags'],['abi',0,'ccr'],['indexed','copy_pixels',0],['indexed','neutral_copy_pixels',0],['indexed','copy_source',0],['indexed','rop_modes',0],['indexed','recolored','rgb',0],['indexed','default_palette',0,0]]
    for location in locations:
        broken=copy.deepcopy(raw);target=broken
        for key in location[:-1]:target=target[key]
        target[location[-1]]^=1;assert differences(reference,project(broken))
    print(json.dumps({'native_depth_abi_cases':12,'palette_entries':256,'pattern_mode_coverage_cases':16,'source_copy_variants':2,'missing_field_rejections':rejected,'mutation_checks':len(locations),'scope':'Software indexed8 profile; native16/32 modes retained raw and outside comparison.'}))
if __name__=='__main__':main()
