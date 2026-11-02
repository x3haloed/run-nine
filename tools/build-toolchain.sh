#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$root/work/Retro68-build"
cd "$root/work/Retro68-build"
export PATH="/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/texinfo/bin:$PATH"
exec ../Retro68/build-toolchain.bash --no-ppc --multiversal
