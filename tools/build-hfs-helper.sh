#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$root/work/hfs-lib"
cd "$root/work/hfs-lib"
if [ ! -f Makefile ]; then ../Retro68/hfsutils/libhfs/configure; fi
make -j4
cc -I "$root/work/Retro68/hfsutils/libhfs" "$root/tools/hfs-image.c" libhfs.a -o "$root/work/hfs-image"
