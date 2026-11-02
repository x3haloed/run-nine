#!/usr/bin/env python3
"""Give the Homebrew QEMU binary a local app identity for desktop controls."""
from pathlib import Path
import plistlib
import shutil

root = Path(__file__).resolve().parents[1]
contents = root / 'work/OS9Lab.app/Contents'
(contents / 'MacOS').mkdir(parents=True, exist_ok=True)
shutil.copy2('/opt/homebrew/bin/qemu-system-ppc', contents / 'MacOS/qemu-system-ppc')
(contents / 'Info.plist').write_bytes(plistlib.dumps({
    'CFBundleIdentifier': 'org.codex.os9lab',
    'CFBundleName': 'OS9Lab',
    'CFBundleExecutable': 'qemu-system-ppc',
    'CFBundlePackageType': 'APPL',
    'CFBundleVersion': '1',
    'NSHighResolutionCapable': True,
}))
print(contents.parent)
