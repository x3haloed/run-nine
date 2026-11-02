#!/usr/bin/env python3
"""Stage the installed QEMU binary as a discoverable native macOS application."""
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile
root=Path(__file__).resolve().parents[1]
app=root/'work/os9/reference-app/OS9 Reference.app'
exe=app/'Contents/MacOS/QEMUReference';exe.parent.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(dir=exe.parent) as staging:
    incoming=Path(staging)/exe.name
    shutil.copy2('/opt/homebrew/bin/qemu-system-ppc',incoming)
    incoming.chmod(0o755)
    incoming.replace(exe)
with (app/'Contents/Info.plist').open('wb') as f:
    plistlib.dump(dict(CFBundleExecutable='QEMUReference',CFBundleIdentifier='org.classicruntime.os9reference',CFBundleName='OS9 Reference',CFBundlePackageType='APPL',LSMinimumSystemVersion='11.0',NSPrincipalClass='NSApplication',NSHighResolutionCapable=True),f)
subprocess.run(['xattr','-dr','com.apple.FinderInfo',str(app)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
subprocess.run(['codesign','--force','--sign','-',str(app)],check=True)
print(exe)
