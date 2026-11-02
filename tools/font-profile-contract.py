#!/usr/bin/env python3
"""Project explicitly shared font-profile observations; preserve raw catalogs.
Native OS9 has more installed families than this runtime. This contract compares
all captured name/ID aliases, queries for the runtime's available families and
unknown names, and ordering/presence of those families after an existing item.
Chicago queries and other installed families remain in the raw reports, not in
this portable contract. This is not raw catalog equality.
"""
import argparse,json
from pathlib import Path
FAMILIES=[list(b'Charcoal'),list(b'Geneva'),list(b'Monaco')]
QUERIES=[(2,'Geneva'),(3,'GENEVA'),(4,'Monaco'),(5,'Charcoal'),(6,'System'),(7,'Application'),(8,'not-a-font')]
def project(raw):
    if raw.get('schema')!=1 or raw.get('probe')!='font-catalog-v1':raise ValueError('expected raw font-catalog-v1')
    fonts=raw['fonts'];names=fonts['names'];numbers=fonts['numbers'];menus=fonts['menus']
    if [r['id'] for r in names]!=[0,1,3,4,2002,32767,-1] or len(numbers)!=9 or [r['kind'] for r in menus]!=[int.from_bytes(b'FONT','big'),int.from_bytes(b'FOND','big')]:raise ValueError('incomplete raw font observations')
    result={'names':names,'queries':[{'name':list(name.encode('ascii')),'number':numbers[index]} for index,name in QUERIES],'menus':[]}
    for menu in menus:
        items=menu['items']
        if not items:raise ValueError('missing existing menu prefix')
        result['menus'].append({'kind':menu['kind'],'prefix':items[0],'counts':[items.count(f) for f in FAMILIES],'known_order':[item for item in items[1:] if item in FAMILIES]})
    return {'schema':1,'probe':'font-profile-v1','fonts':result}
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('raw',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
    if a.output.exists():raise FileExistsError(a.output)
    a.output.write_text(json.dumps(project(json.loads(a.raw.read_text())),indent=2)+'\n')
if __name__=='__main__':main()
