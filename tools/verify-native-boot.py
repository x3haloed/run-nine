#!/usr/bin/env python3
"""Boot a freestanding ARM64 kernel and compare its shared-core observations."""
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import struct
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'outputs'
WORK=ROOT/'work/native'
BOARD='virt-11.1,gic-version=3'

def checked(*args):
    result=subprocess.run(args,cwd=ROOT,capture_output=True,text=True)
    if result.returncode:raise RuntimeError(result.stdout+result.stderr)
    return result.stdout
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def elf_info(path):
    data=path.read_bytes()
    if data[:6]!=b'\x7fELF\x02\x01':raise RuntimeError('Kernel is not a little-endian ELF64 file')
    kind,machine=struct.unpack_from('<HH',data,16)
    entry,offset=struct.unpack_from('<QQ',data,24)
    size,count=struct.unpack_from('<HH',data,54)
    segments=[]
    for i in range(count):
        p=offset+i*size;tag,flags=struct.unpack_from('<II',data,p)
        if tag in (2,3):raise RuntimeError('Kernel unexpectedly has a dynamic linker or interpreter segment')
        if tag==1:
            _,virtual,physical,file_size,memory_size,alignment=struct.unpack_from('<QQQQQQ',data,p+8)
            if physical!=virtual or physical<0x40000000 or physical+memory_size>0x48000000:raise RuntimeError('Kernel load segment is outside expected physical RAM')
            segments.append({'address':physical,'file_bytes':file_size,'memory_bytes':memory_size,'flags':flags})
    if kind!=2 or machine!=183 or entry!=0x40080000 or not segments:raise RuntimeError('Unexpected ARM64 kernel executable layout')
    return {'type':'ELF64 executable','machine':machine,'entry':entry,'dynamic_linker':False,'segments':segments}
def device_tree(path):
    data=path.read_bytes();header=struct.unpack_from('>10I',data)
    magic,total,structure,strings=header[:4]
    if magic!=0xd00dfeed or total!=len(data):raise RuntimeError('Invalid QEMU device tree')
    position=structure;stack=[];nodes={}
    while True:
        token=struct.unpack_from('>I',data,position)[0];position+=4
        if token==1:
            end=data.index(0,position);stack.append(data[position:end].decode());position=(end+4)&~3;nodes['/'.join(stack)]={}
        elif token==2:stack.pop()
        elif token==3:
            length,name=struct.unpack_from('>II',data,position);position+=8
            end=data.index(0,strings+name);key=data[strings+name:end].decode();nodes['/'.join(stack)][key]=data[position:position+length];position=(position+length+3)&~3
        elif token==4:continue
        elif token==9:break
        else:raise RuntimeError('Unknown device-tree token')
    uarts=[(name,p) for name,p in nodes.items() if b'arm,pl011\0' in p.get('compatible',b'')]
    ram=[(name,p) for name,p in nodes.items() if p.get('device_type')==b'memory\0']
    uart=next((name for name,p in uarts if struct.unpack_from('>Q',p['reg'])[0]==0x9000000),None)
    memory=next((name for name,p in ram if struct.unpack_from('>QQ',p['reg'])==(0x40000000,128*1024*1024)),None)
    if uart is None or memory is None:raise RuntimeError('QEMU platform does not match the kernel UART/RAM map')
    return {'pl011_node':uart,'uart_address':0x9000000,'ram_address':0x40000000,'ram_bytes':128*1024*1024}
def main():
    WORK.mkdir(parents=True,exist_ok=True);OUT.mkdir(exist_ok=True)
    checked('cargo','build','--manifest-path',str(ROOT/'native/Cargo.toml'),'--target','aarch64-unknown-none','--release')
    kernel=ROOT/'native/target/aarch64-unknown-none/release/classic-boot'
    executable=elf_info(kernel)
    tree=checked('cargo','tree','--manifest-path',str(ROOT/'native/Cargo.toml'),'--target','aarch64-unknown-none','--edges','normal','--prefix','none')
    expected={('classic-boot','v0.1.0'),('classic-core','v0.1.0'),('m68k','v0.14.7'),('libm','v0.2.16'),('spin','v0.9.9')}
    if {tuple(line.split()[:2]) for line in tree.splitlines()}!=expected:raise RuntimeError('Unexpected native kernel dependency')
    features=checked('cargo','tree','--manifest-path',str(ROOT/'native/Cargo.toml'),'--target','aarch64-unknown-none','--edges','features')
    if 'm68k feature "std"' in features or 'm68k feature "jit"' in features:raise RuntimeError('Hosted CPU feature in native kernel')
    qemu=shutil.which('qemu-system-aarch64')
    if not qemu:raise RuntimeError('qemu-system-aarch64 is required')
    dtb=WORK/'virt.dtb'
    checked(qemu,'-machine',BOARD+',dumpdtb='+str(dtb),'-cpu','cortex-a76','-m','128M','-smp','1','-display','none','-monitor','none','-serial','null','-nic','none')
    platform=device_tree(dtb)
    command=[qemu,'-machine',BOARD,'-cpu','cortex-a76','-accel','tcg','-m','128M','-smp','1','-display','none','-monitor','none','-serial','stdio','-nic','none','-kernel',str(kernel)]
    process=subprocess.Popen(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    output=bytearray();deadline=time.monotonic()+20;completed=False
    try:
        while time.monotonic()<deadline:
            ready,_,_=select.select([process.stdout],[],[],0.2)
            if ready:
                chunk=os.read(process.stdout.fileno(),4096)
                if not chunk:break
                output.extend(chunk)
                if b'CLASSIC-NATIVE-PANIC' in output:break
                if b'CLASSIC-NATIVE-PASS\n' in output:completed=True;break
            if process.poll() is not None:break
    finally:
        if process.poll() is None:process.terminate()
        tail,_=process.communicate(timeout=5);output.extend(tail)
    serial=output.decode('utf-8',errors='replace');(OUT/'Native-boot-serial.txt').write_text(serial)
    if not completed or 'platform=qemu-virt cpu=aarch64 el=1 mmu=0' not in serial:raise RuntimeError('Kernel did not complete the native experiment:\n'+serial)
    observations=[json.loads(line) for line in serial.splitlines() if line.startswith('{')]
    if {v['probe'] for v in observations}!={'core-v1','owned-68k-v1'}:raise RuntimeError('Missing actual owned 68K execution output')
    observation=next(v for v in observations if v['probe']=='core-v1')
    owned=next(v for v in observations if v['probe']=='owned-68k-v1')
    owned_native=OUT/'Owned68KProbe-native.json';owned_native.write_text(json.dumps(owned,separators=(',',':'))+'\n')
    owned_comparison=json.loads(checked(sys.executable,str(ROOT/'tools/compare.py'),str(OUT/'Owned68KProbe-reference.json'),str(owned_native)))
    (OUT/'Owned68KProbe-native-comparison.json').write_text(json.dumps(owned_comparison,indent=2)+'\n')
    execution_line=next(line for line in serial.splitlines() if line.startswith('OWNED68K '))
    if execution_line!='OWNED68K instructions=195 traps=10 pc=0x1800 sp=0xf004 preserved=1':raise RuntimeError('Owned CPU did not follow the measured instruction/return path')
    native=OUT/'CoreProbe-native.json';native.write_text(json.dumps(observation,separators=(',',':'))+'\n')
    comparison=json.loads(checked(sys.executable,str(ROOT/'tools/compare.py'),str(OUT/'CoreProbe-reference.json'),str(native)))
    (OUT/'CoreProbe-native-comparison.json').write_text(json.dumps(comparison,indent=2)+'\n')
    artifact=OUT/'Native-core-kernel.elf';shutil.copyfile(kernel,artifact)
    report={'booted':True,'target':'aarch64-unknown-none','board':BOARD,'cpu':'cortex-a76','exception_level':1,'mmu_enabled':False,
            'kernel_sha256':digest(kernel),'kernel':executable,'platform':platform,'device_tree_sha256':digest(dtb),'dependencies':tree.splitlines(),
            'qemu_version':checked(qemu,'--version').splitlines()[0],'rust_version':checked('rustc','--version').strip(),
            'feature_tree':features.splitlines(),'owned_68k_execution':execution_line,'owned_program_sha256':digest(ROOT/'probes/owned68k/program.bin'),'owned_68k_probe_matches_os9':owned_comparison['equal'],'shared_core_probe_matches_os9':comparison['equal'],'shutdown':'Host terminates QEMU after the completion marker; kernel otherwise waits in WFE.',
            'scope':'Shared Memory Manager/Rect experiment plus the same owned166-byte 68K program executing195 instructions and10 real shared memory-service traps through the pinned interpreter, in freestanding ARM64 at EL1. Exact owned result/guard bytes match OS9. No Marathon/TeachText execution in the kernel, GUI, storage, interrupts, FPU validation or physical Pi500 validation.',
            'full_goal_complete':False}
    (OUT/'Native-boot-result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':
    try:main()
    except Exception as error:
        (OUT/'Native-boot-result.json').write_text(json.dumps({'verification_error':str(error),'full_goal_complete':False},indent=2)+'\n')
        raise
