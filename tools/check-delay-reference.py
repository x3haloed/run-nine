#!/usr/bin/env python3
"""Audit portable Delay predicates against the retained native tick samples."""
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def main():
    raw=json.loads((ROOT/'outputs/DelayProbe-raw-reference.json').read_text())['ticks']
    if len(raw)!=3 or any(not isinstance(row,list) or len(row)!=5 or any(type(n) is not int for n in row) for row in raw):raise ValueError('Incomplete raw native Delay observations')
    derived=[]
    for expected,(request,before,result,after,arg) in enumerate(raw):
        if request!=expected:raise ValueError('Unexpected native Delay input')
        derived.append([request,arg,int(((after-before)&0xffffffff)>=request),int(((result-before)&0xffffffff)>=request),int(((after-result)&0xffffffff)<0x80000000)])
    if derived!=json.loads((ROOT/'outputs/DelayProbe-reference.json').read_text())['timing']['cases']:raise ValueError('Native Delay predicates do not derive from retained ticks')
    print(json.dumps({'native_raw_delay_derivation':True,'requests':[0,1,2],'absolute_tick_equality_claimed':False}))
if __name__=='__main__':main()
