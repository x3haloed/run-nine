#!/usr/bin/env python3
"""Ensure explicit font-profile projection preserves its claimed observations."""
import copy
import importlib.util
import json
from pathlib import Path
from compare import differences,read_probe
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('font_contract',ROOT/'tools/font-profile-contract.py')
contract=importlib.util.module_from_spec(spec);spec.loader.exec_module(contract)
def main():
    raw=json.loads((ROOT/'outputs/FontCatalogProbe-reference.json').read_text())
    expected=read_probe(ROOT/'outputs/FontProfileProbe-reference.json')
    assert not differences(expected,contract.project(raw))
    runtime=json.loads((ROOT/'outputs/FontCatalogProbe-runtime.json').read_text())
    assert not differences(contract.project(runtime),read_probe(ROOT/'outputs/FontProfileProbe-runtime.json'))
    assert differences(raw,runtime), 'raw catalogs must retain their installed-profile differences'
    checks=3
    def rejected(change):
        nonlocal checks
        changed=copy.deepcopy(raw);change(changed['fonts'])
        assert differences(expected,contract.project(changed));checks+=1
    rejected(lambda f:f['names'][0]['name'].append(65))
    rejected(lambda f:f['numbers'].__setitem__(2,999))
    rejected(lambda f:f['menus'][0]['items'][0].append(65))
    rejected(lambda f:f['menus'][0]['items'].remove(list(b'Geneva')))
    rejected(lambda f:f['menus'][0]['items'].append(list(b'Geneva')))
    def reorder(f):
        items=f['menus'][1]['items'];a=items.index(list(b'Charcoal'));b=items.index(list(b'Monaco'));items[a],items[b]=items[b],items[a]
    rejected(reorder)
    changed=copy.deepcopy(raw)
    for menu in changed['fonts']['menus']:menu['items'].remove(list(b'Arial'))
    assert not differences(expected,contract.project(changed));checks+=1
    print(json.dumps({'checks':checks,'passed':True,'scope':'Explicit shared font profile only; full installed catalogs differ and remain preserved.'}))
if __name__=='__main__':main()
