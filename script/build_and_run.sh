#!/bin/bash
set -euo pipefail
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mode=${1:-run}
case "$mode" in run|--verify|--debug|--logs) ;; *) echo 'usage: build_and_run.sh [run|--verify|--debug|--logs] [TeachText|SimpleText|Marathon|IndexedColorProbe]' >&2; exit 2;; esac
application=${2:-TeachText}
case "$application" in TeachText|SimpleText|Marathon|IndexedColorProbe) ;; *) echo 'supported applications: TeachText, SimpleText, Marathon, IndexedColorProbe' >&2; exit 2;; esac
session_dir="$root/work/hosted"
volume="$root/work/volumes/interactive"
framebuffer="$root/work/interactive-final.ppm"
if [ "$application" = SimpleText ]; then
    session_dir="$root/work/hosted/SimpleText"
    volume="$root/work/volumes/interactive-simpletext"
    framebuffer="$session_dir/final.ppm"
fi
if [ "$application" = Marathon ] || [ "$application" = IndexedColorProbe ]; then
    session_dir="$root/work/hosted/$application"
    volume="$root/work/volumes/interactive-$application"
    framebuffer="$session_dir/final.ppm"
fi
volume=${CLASSIC_RUNTIME_VOLUME:-$volume}
session_dir=${CLASSIC_RUNTIME_SESSION_DIR:-$session_dir}
# Explicit input overrides also allow verified local copies when FileProvider
# delays reads under Documents before the native event loop can start.
application_dir=${CLASSIC_RUNTIME_APPLICATION_DIR:-$root/work/apps/$application}
# Generated application bundles stay with the checkout's ignored local data.
app="$root/work/hosted/Classic Runtime.app"
binary="$app/Contents/MacOS/ClassicRuntime"
pkill -x ClassicRuntime >/dev/null 2>&1 || true
# Keep the GUI executable stable while headless cargo run/test checks execute.
gui_target="$root/work/hosted/gui-target"
# Keep dev overflow checks/debug info while making interpreted execution usable.
CARGO_PROFILE_DEV_OPT_LEVEL="${CLASSIC_RUNTIME_OPT_LEVEL:-2}" cargo build --manifest-path "$root/runtime/Cargo.toml" --features gui --target-dir "$gui_target"
mkdir -p "$app/Contents/MacOS" "$root/work/hosted"
printf '%s\n' "$app" > "$root/work/hosted/current-bundle-path.txt"
rm -f "$binary"
cp "$gui_target/debug/classic-runtime" "$binary"
python3 - "$app/Contents/Info.plist" <<'PY'
import plistlib,sys
with open(sys.argv[1],'wb') as f:
    plistlib.dump(dict(CFBundleExecutable='ClassicRuntime',CFBundleIdentifier='org.classicruntime.hosted',CFBundleName='Classic Runtime',CFBundlePackageType='APPL',LSMinimumSystemVersion='11.0',NSPrincipalClass='NSApplication',NSHighResolutionCapable=True),f)
PY
xattr -d com.apple.FinderInfo "$app" 2>/dev/null || true
codesign --force --sign - "$app"
codesign --verify --strict "$app"
args=("$application_dir/$application.rsrc" --application-name "$application" --gui --trace --volume "$volume" --dump-documents "$session_dir/documents.json" --capture-framebuffer "$framebuffer")
if [ "$application" = Marathon ]; then
    args=("$application_dir/Marathon.rsrc" --application-name 'Marathon v1.2' --application-data "$application_dir/Marathon.data" --application-files "${CLASSIC_RUNTIME_APPLICATION_FILES:-$root/work/volumes/marathon-source-v1}" --gui --color-screen --trace --trace-timers --trace-alerts --trace-files --volume "$volume" --capture-framebuffer "$framebuffer")
elif [ "$application" = IndexedColorProbe ]; then
    args=(--probe indexed-color-v1 --gui)
fi
# Generic audio/capture options apply to any application using the shared API.
if [ -n "${CLASSIC_RUNTIME_AUDIO_OUTPUT:-}" ]; then args+=(--audio-output "$CLASSIC_RUNTIME_AUDIO_OUTPUT"); fi
if [ -n "${CLASSIC_RUNTIME_AUDIO_CAPTURE:-}" ]; then args+=(--capture-audio "$CLASSIC_RUNTIME_AUDIO_CAPTURE"); fi
if [ -n "${CLASSIC_RUNTIME_SOUND_LOG:-}" ]; then args+=(--sound-log "$CLASSIC_RUNTIME_SOUND_LOG" --trace-sound); fi
if [ -n "${CLASSIC_RUNTIME_AUDIO_INPUTS:-}" ]; then args+=(--capture-audio-inputs "$CLASSIC_RUNTIME_AUDIO_INPUTS"); fi
cd "$root"
if [ "$mode" = --debug ]; then exec lldb -- "$binary" "${args[@]}"; fi
mkdir -p "$session_dir"
# Launch the bundled Rust executable through the authorized development host.
# LaunchServices starts a separate permission context that blocks at the first
# Documents file read before minifb can initialize its native event loop.
"$binary" "${args[@]}" > "$session_dir/stdout.log" 2> "$session_dir/stderr.log" &
host_pid=$!
if [ "$mode" = --verify ]; then
    sleep 1
    pgrep -x ClassicRuntime >/dev/null
elif [ "$mode" = --logs ]; then
    exec tail -f "$session_dir/stdout.log" "$session_dir/stderr.log"
fi
# Keep the development host alive for the native window's lifetime, including
# when a terminal runner cleans up descendants as soon as its shell exits.
wait "$host_pid"
