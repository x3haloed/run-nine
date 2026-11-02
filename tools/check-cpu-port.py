#!/usr/bin/env python3
"""Verify every vendored CPU file against its pin and the upstream archive."""
import hashlib,json,tarfile,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def sha(b):return hashlib.sha256(b).hexdigest()
def main():
    vendor=ROOT/'vendor/m68k';pin=json.loads((vendor/'PORT.json').read_text())
    archives=list((Path.home()/'.cargo/registry/cache').glob('*/m68k-0.14.7.crate'))
    archive=archives[0] if archives else ROOT/'work/ports/upstream-m68k-0.14.7.crate'
    if not archive.exists():
        archive.parent.mkdir(parents=True,exist_ok=True)
        payload=urllib.request.urlopen('https://static.crates.io/crates/m68k/m68k-0.14.7.crate').read()
        assert sha(payload)==pin['crate_sha256'];archive.write_bytes(payload)
    assert sha(archive.read_bytes())==pin['crate_sha256']
    actual={p.relative_to(vendor).as_posix() for p in vendor.rglob('*') if p.is_file() and p.relative_to(vendor).parts[0] not in ['target','PORT.md','PORT.json','Cargo.lock']}
    assert actual==set(pin['files']),'CPU source set changed without provenance update'
    with tarfile.open(archive) as upstream:
        for name,pair in pin['files'].items():
            assert sha((vendor/name).read_bytes())==pair['vendored_sha256'],'Changed vendored CPU file: '+name
            if pair['upstream_sha256'] is not None:
                original=upstream.extractfile('m68k-0.14.7/'+name).read()
                assert sha(original)==pair['upstream_sha256'],'Unverified upstream CPU file: '+name
    print(json.dumps({'upstream_crate_sha256':pin['crate_sha256'],'files_pinned':len(actual),'modified_files':len(pin['modified_files']),'added_files':pin['added_files'],'license':'MIT'}))
if __name__=='__main__':main()
