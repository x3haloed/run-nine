"""Exact Courier measurements, plus the explicit four-family menu subset."""
FIELDS={'font','size','face','real','info','widths','labels'}
FAMILIES=[list(b'Charcoal'),list(b'Courier'),list(b'Geneva'),list(b'Monaco')]
SIZES=[0,9,10,12,14,18,24]
def rows(fonts):
    if not isinstance(fonts,list)or len(fonts)!=14:raise ValueError('Incomplete Courier matrix')
    for i,r in enumerate(fonts):
        if not isinstance(r,dict)or set(r)!=FIELDS:raise ValueError('Missing Courier field')
        if any(type(r[k])is not int for k in ['font','size','face','real']):raise ValueError('Invalid Courier scalar')
        if(r['font'],r['size'],r['face'])!=(22,SIZES[i//2],i%2):raise ValueError('Changed Courier context')
        for k,n in [('info',4),('widths',256),('labels',3)]:
            if not isinstance(r[k],list)or len(r[k])!=n or any(type(x)is not int for x in r[k]):raise ValueError('Incomplete Courier '+k)
def project(raw,catalog):
    if not isinstance(raw,dict)or set(raw)!={'schema','probe','name','number','fonts'}or raw['schema']!=1 or raw['probe']!='courier-font-raw-v1':raise ValueError('Wrong Courier capture')
    rows(raw['fonts'])
    if catalog.get('schema')!=1 or catalog.get('probe')!='font-catalog-v1':raise ValueError('Wrong font menu capture')
    menus=catalog['fonts']['menus']
    if len(menus)!=2 or [m['kind']for m in menus]!=[int.from_bytes(x,'big')for x in [b'FONT',b'FOND']]:raise ValueError('Incomplete font menu matrix')
    selected=[]
    for m in menus:
        if not m['items']:raise ValueError('Missing existing menu prefix')
        selected.append({'kind':m['kind'],'prefix':m['items'][0],'known_order':[x for x in m['items'][1:]if x in FAMILIES]})
    result={'schema':1,'probe':'courier-font-v1','fonts':{'name':raw['name'],'number':raw['number'],'records':raw['fonts'],'menus':selected}};validate(result);return result

def validate(v):
    if not isinstance(v,dict)or set(v)!={'schema','probe','fonts'}or v.get('schema')!=1 or v.get('probe')!='courier-font-v1':raise ValueError('Wrong Courier profile')
    f=v['fonts']
    if not isinstance(f,dict)or set(f)!={'name','number','records','menus'}or type(f['number'])is not int or not isinstance(f['name'],list)or any(type(x)is not int or not 0<=x<=255 for x in f['name']):raise ValueError('Missing Courier metadata')
    rows(f['records'])
    if not isinstance(f['menus'],list)or len(f['menus'])!=2:raise ValueError('Missing Courier menus')
    for m in f['menus']:
        if not isinstance(m,dict)or set(m)!={'kind','prefix','known_order'}or type(m['kind'])is not int or not isinstance(m['prefix'],list)or not isinstance(m['known_order'],list):raise ValueError('Missing Courier menu observation')
