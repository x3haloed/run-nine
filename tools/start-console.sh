#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cp "$root/tools/reference-console.html" "$root/work/novnc/index.html"
exec "$root/work/novnc/venv/bin/websockify" --web "$root/work/novnc" 127.0.0.1:6085 127.0.0.1:5905
