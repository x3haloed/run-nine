#!/usr/bin/env python3
"""Assemble the same owned Sound Manager caller for both execution contexts."""
from pathlib import Path
import subprocess
ROOT=Path(__file__).resolve().parents[1]
source=ROOT/'probes/soundguest'
work=ROOT/'work/probes/soundguest';work.mkdir(parents=True,exist_ok=True)
prefix=ROOT/'work/Retro68-build/toolchain/bin'
subprocess.run([str(prefix/'m68k-apple-macos-as'),'-m68020','-o',str(work/'program.o'),str(source/'program.s')],check=True)
subprocess.run([str(prefix/'m68k-apple-macos-objcopy'),'-O','binary','-j','.text',str(work/'program.o'),str(source/'program.bin')],check=True)
data=(source/'program.bin').read_bytes()
(source/'program.h').write_text('/* Owned program.s assembled bytes. */\nstatic const unsigned char sound_guest_program[] = {'+','.join(f'0x{b:02x}' for b in data)+'};\n')
print(f'{len(data)} owned68K bytes')
