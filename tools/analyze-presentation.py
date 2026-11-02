#!/usr/bin/env python3
"""Summarize an opt-in hosted frame log; timing describes submissions, not scanout."""
import argparse
import hashlib
import json
from pathlib import Path

def summarize(path):
    raw=path.read_bytes()
    rows=[json.loads(line) for line in raw.splitlines()]
    if len(rows)<2:raise ValueError('At least two complete frame rows are required')
    integers=('frame','elapsed_us','interval_us','conversion_us','present_us','pc','instruction','update_depth','dark_pixels')
    for index,row in enumerate(rows):
        for key in integers:
            if type(row.get(key)) is not int or row[key]<0:raise ValueError(f'Invalid {key} at frame{index}')
        if row['frame']!=index:raise ValueError('Frame sequence is incomplete')
        fingerprint=row.get('framebuffer_fnv64','')
        if len(fingerprint)!=16 or any(c not in '0123456789abcdef' for c in fingerprint):raise ValueError('Invalid framebuffer fingerprint')
        if index and row['elapsed_us']<=rows[index-1]['elapsed_us']:raise ValueError('Non-monotonic elapsed time')
    def stats(values):
        values=sorted(values);n=len(values)
        return {'min':values[0],'median':(values[(n-1)//2]+values[n//2])/2,'p99':values[((n-1)*99)//100],'max':values[-1]}
    updating=[(i,r) for i,r in enumerate(rows) if r['update_depth']]
    return {'schema':1,'frames':len(rows),'duration_us':rows[-1]['elapsed_us'],'raw_sha256':hashlib.sha256(raw).hexdigest(),
        'timings_us':{'inter_submission':stats([rows[i]['elapsed_us']-rows[i-1]['elapsed_us'] for i in range(1,len(rows))]),
                      **{key:stats([r[key] for r in rows[1:]]) for key in ('interval_us','conversion_us','present_us')}},
        'frames_inside_update':len(updating),
        'inside_update_identical_to_previous':sum(i>0 and r['framebuffer_fnv64']==rows[i-1]['framebuffer_fnv64'] for i,r in updating),
        'changed_frames_after_first':sum(rows[i]['framebuffer_fnv64']!=rows[i-1]['framebuffer_fnv64'] for i in range(1,len(rows))),
        'all_white_frames':sum(r['dark_pixels']==0 for r in rows),
        'limits':'Excludes first artificial interval from timing statistics. Inter-submission is elapsed log timestamp difference after native update_with_buffer returns; interval_us excludes conversion/presentation. This is an instrumented debug build, not an input-to-photon or display scanout measurement. Whole-frame fingerprints/counts do not identify every transient blank document region. FNV is a change fingerprint; raw provenance uses SHA256.'}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('log',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
    result=summarize(a.log);a.output.write_text(json.dumps(result,indent=2)+'\n');print(a.output)
if __name__=='__main__':main()
