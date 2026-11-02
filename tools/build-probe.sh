#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
probe=${1:-core}
case "$probe" in soundamplitude|sounddiscovery|soundguest|soundcommands|sounddevices|soundpcm|mousecoupling|roundrect|textbox|dialogitemtext|truncation|qdline|modaldialog|newdialogdetails|newdialog|indresource|penpat|hgetvinfo|mmumode|osevents|getkeys|flushfile|qdrandom|countresources|stilldown|inverse4|legacycolors|colorread|pictbitmap|cursormotion|cursorstate|sanepower|sanei32widen|sanef32widen|ctabchanged|ctseed|windowdamage|emptydriver|soundcontrol|pixmap|courier|getctable|apprelease|sanecompare|sanemixedadd|sanecos|opendf2|opendf|sanetan|saneintflags|saneround|sanemixed|saneenv|sanebasicx|saneclass|saneextended|sanesine|saneinteger|sanediv|sanewiden|saneconvert|sanemul|sane|inverse|handclone|offdevice|maxdevice|origin|vbl|windowbackground|windowcolors|movehhi|devicedepth|timemanager|alerts|soundchannels|owned68k|addressing|soundversion|cataloginfo|fcbinfo|pathpb|pathfiles|controlthumbedge|controlthumb|inputevents|controltrack|tedouble|controlhit|windowtracking|windowgeometry|styleuse|handlestate|doclifecycle|stylescrap|resio|rescreate|delay|modalfilter|scriptcontext|fontavailability|stringcompare|resourceenum|tecontinuous|tearrows|teoutline|tefeatures|teautoview|stylequeries|styledtext|windowresources|appleevents|menusemantics|menuitems|fontcatalog|systemstyles|menubars|launchmarathon|gestalt|environs|segments|appsource|dialogparams|deletefiles|saveconfirm|processes|clipboard|core|quickdraw|fonts|windows|memoryflags|regions|colorports|textedit|textlayout|controls|windowflow|titlemetrics|inkmetrics|hittest|themefonts|colortext|titlebounds|titlepairs|files|resfiles|textdirect) ;; *) echo "unknown probe" >&2; exit 2;; esac
case "$probe" in sane|sanemul|sanediv|sanebasicx) python3 "$root/tools/build-sane-operands.py";; esac
if [ "$probe" = soundguest ]; then python3 "$root/tools/build-sound-guest-fixture.py"; fi
if [ "$probe" = pictbitmap ]; then python3 "$root/tools/build-pict-bitmap-fixtures.py"; fi
if [ "$probe" = sanepower ]; then python3 "$root/tools/build-sane-power-operands.py"; fi
if [ "$probe" = sanei32widen ]; then python3 "$root/tools/build-sane-long-widen-operands.py"; fi
if [ "$probe" = sanef32widen ]; then python3 "$root/tools/build-sane-single-widen-operands.py"; fi
if [ "$probe" = sanecompare ]; then python3 "$root/tools/build-sane-compare-operands.py"; fi
if [ "$probe" = saneintflags ]; then python3 "$root/tools/build-sane-intflags-operands.py"; fi
if [ "$probe" = saneround ]; then python3 "$root/tools/build-sane-round-operands.py"; fi
if [ "$probe" = sanesine ] || [ "$probe" = sanetan ] || [ "$probe" = sanecos ]; then python3 "$root/tools/build-sane-sine-operands.py"; fi
if [ "$probe" = saneinteger ]; then python3 "$root/tools/build-sane-integer-operands.py"; fi
if [ "$probe" = sanewiden ]; then python3 "$root/tools/build-sane-widen-operands.py"; fi
if [ "$probe" = sanemixed ] || [ "$probe" = sanemixedadd ]; then python3 "$root/tools/build-sane-widen-operands.py"; python3 "$root/tools/build-sane-operands.py"; fi
if [ "$probe" = saneclass ]; then python3 "$root/tools/build-sane-conversion-operands.py"; fi
if [ "$probe" = saneextended ]; then python3 "$root/tools/build-sane-conversion-operands.py"; fi
if [ "$probe" = saneconvert ]; then python3 "$root/tools/build-sane-conversion-operands.py"; fi
if [ "$probe" = vbl ]; then python3 "$root/tools/build-vbl-callback.py"; fi
if [ "$probe" = timemanager ]; then python3 "$root/tools/build-timer-callback.py"; fi
cmake -S "$root/probes/$probe" -B "$root/work/probes/$probe" \
 -DCMAKE_TOOLCHAIN_FILE="$root/work/Retro68-build/toolchain/m68k-apple-macos/cmake/retro68.toolchain.cmake"
cmake --build "$root/work/probes/$probe"

if [ "$probe" = alerts ]; then
 "$root/work/Retro68-build/toolchain/bin/m68k-apple-macos-as" -m68020 -o "$root/work/probes/alerts/runtime-filter.o" "$root/probes/alerts/runtime-filter.s"
 "$root/work/Retro68-build/toolchain/bin/m68k-apple-macos-objcopy" -O binary -j .text "$root/work/probes/alerts/runtime-filter.o" "$root/work/probes/alerts/runtime-filter.bin"
 python3 "$root/tools/build-alert-resources.py"
fi

if [ "$probe" = windowcolors ]; then python3 "$root/tools/build-window-colors-resources.py"; fi

if [ "$probe" = modaldialog ]; then
 "$root/work/Retro68-build/toolchain/bin/m68k-apple-macos-as" -m68020 -o "$root/work/probes/modaldialog/runtime-filter.o" "$root/probes/modaldialog/runtime-filter.s"
 "$root/work/Retro68-build/toolchain/bin/m68k-apple-macos-objcopy" -O binary -j .text "$root/work/probes/modaldialog/runtime-filter.o" "$root/work/probes/modaldialog/runtime-filter.bin"
fi
