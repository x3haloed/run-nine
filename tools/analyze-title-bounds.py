#!/usr/bin/env python3
"""Test a glyph-interval model calibrated only from anchored native titles."""
import argparse
import hashlib
import json
from pathlib import Path

def width(bounds, advances, text):
    intervals=[]
    x=0
    for byte in text:
        left,right=bounds[byte]
        # Blank glyphs still contribute their pen origin, so leading/trailing
        # spaces differ without treating either as an application special case.
        intervals.append((x+left,x+right))
        x+=advances[byte]
    return max(r for l,r in intervals)-min(l for l,r in intervals) if intervals else 0

def analyze(observed):
    advances=observed['advances']
    def derive(letter,key):
        anchor=ord(letter)
        return [[advances[c]+observed['single'][anchor]-observed['suffix_'+key][c],
                 observed['prefix_'+key][c]-advances[anchor]] if observed['single'][c] else [0,0] for c in range(256)]
    bounds=derive('M','m')
    other=derive('R','r')
    shifts=sorted({(b[0]-a[0],b[1]-a[1]) for a,b in zip(bounds,other) if a[0]!=a[1]})
    failures=[]
    for c in range(256):
        for key,text in [('single',[c]),('leading_space',[32,c]),('trailing_space',[c,32])]:
            actual=width(bounds,advances,text)
            expected=observed[key][c]
            if actual!=expected:failures.append({'case':key,'byte':c,'reference':expected,'candidate':actual})
    for i,item in enumerate(observed['heldout']):
        actual=width(bounds,advances,item['bytes'])
        if actual!=item['width']:failures.append({'case':'heldout','index':i,'bytes':item['bytes'],'reference':item['width'],'candidate':actual})
    consistent=len(shifts)==1 and shifts[0][0]==shifts[0][1]
    return bounds,{'anchor_shifts':shifts,'anchors_consistent':consistent,'comparison_count':3*256+len(observed['heldout']),'equal':consistent and not failures,'differences':failures}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('reference',type=Path)
    p.add_argument('--report',type=Path)
    p.add_argument('--metrics',type=Path)
    args=p.parse_args()
    raw=args.reference.read_bytes();observed=json.loads(raw)
    if observed.get('probe')!='title-bounds-v1':raise ValueError('Unexpected probe')
    bounds,report=analyze(observed)
    if args.report:args.report.write_text(json.dumps(report,indent=2)+'\n')
    if args.metrics:
        if not report['equal']:raise ValueError('Refusing to export an unverified glyph-interval model')
        data={'font':0,'family':2002,'name':'Charcoal','size':12,'face':0,'reference_sha256':hashlib.sha256(raw).hexdigest(),'calibration':'M-prefix and M-suffix title measurements; R anchor independently checks translation consistency','bounds':bounds}
        args.metrics.write_text(json.dumps(data,indent=2)+'\n')
    print(json.dumps(report,indent=2))
    return 0 if report['equal'] else 1
if __name__=='__main__':raise SystemExit(main())
