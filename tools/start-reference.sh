#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then echo 'usage: start-reference.sh WORKING_QCOW2 [--cocoa]' >&2; exit 2; fi
display=none
emulator=${CLASSIC_REFERENCE_EMULATOR:-/opt/homebrew/bin/qemu-system-ppc}
firmware=${CLASSIC_REFERENCE_FIRMWARE_DIR:-/opt/homebrew/share/qemu}
if [ "$#" -eq 2 ]; then
 if [ -n "${CLASSIC_REFERENCE_EMULATOR:-}" ]; then echo 'a custom reference emulator must use its VNC viewer, not --cocoa staging' >&2; exit 2; fi
 case "$2" in --cocoa) display=cocoa; emulator=$(python3 "$root/tools/stage-reference-app.py");; *) echo 'unknown display option' >&2; exit 2;; esac
fi
disk=$(cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")
test -f "$disk"
# Optional internal qcow2 VM snapshot; independent of the presentation backend.
snapshot=${CLASSIC_REFERENCE_SNAPSHOT:-}
vnc=127.0.0.1:5
case ${CLASSIC_REFERENCE_VNC_AUTH:-0} in
 0) ;;
 1) vnc=$vnc,password=on ;;
 *) echo 'CLASSIC_REFERENCE_VNC_AUTH must be 0 or 1' >&2; exit 2;;
esac
set --
if [ -n "$snapshot" ]; then set -- -loadvm "$snapshot"; fi
# Optional native Sound Manager reference capture. Keep normal probe hardware
# unchanged unless explicitly requested; a saved VM must match its device tree.
audio_capture=${CLASSIC_REFERENCE_AUDIO_WAV:-}
if [ -n "$audio_capture" ]; then
 if [ -n "$snapshot" ]; then echo 'audio hardware requires a fresh boot, not an existing VM snapshot' >&2; exit 2; fi
 if [ -e "$audio_capture" ]; then echo 'audio capture output must be a new file' >&2; exit 2; fi
 set -- "$@" -audiodev "wav,id=reference-audio,path=$audio_capture,out.frequency=48000,out.channels=2,out.format=s16"
 case ${CLASSIC_REFERENCE_AUDIO_DEVICE:-usb} in
  usb) set -- "$@" -device usb-audio,audiodev=reference-audio ;;
  screamer) set -- "$@" -global screamer.audiodev=reference-audio -bios "$firmware/openbios-ppc" ;;
  *) echo 'CLASSIC_REFERENCE_AUDIO_DEVICE must be usb or screamer' >&2; exit 2;;
 esac
fi
exec "$emulator" -L "$firmware" -name 'OS 9 Automated Reference' \
 -M mac99,via=pmu -cpu G4 -m 512 -accel tcg \
 -drive "file=$disk,format=qcow2,if=ide,index=0" \
 -boot c -g 1024x768x32 -usb \
 -nic none -display "$display" -vnc "$vnc" \
 -qmp "unix:$root/work/os9/run/qmp.sock,server=on,wait=off" "$@"
