#!/usr/bin/env python3
"""VM-only QEMU boot qualification; never attach host disks or network."""
import argparse
import pathlib
import shutil
import subprocess
import tempfile
import time
import socket
import json

ROOT = pathlib.Path(__file__).resolve().parents[1]
REQUIRED = [b'H5K: BOOT v0.1.1', b'H5K: GOP_FOUND', b'H5K: EXIT_BOOT_SERVICES_OK', b'H5K: KERNEL_ENTERED', b'H5K: STACK_OK', b'H5K: MEMORY_MAP_OK', b'H5K: ALLOCATOR_OK', b'H5K: FRAME_RW_REUSE_OK', b'H5K: STACK_CANARY_OK', b'H5K: CPUID_MAX=', b'H5K: GDT_IDT_BREAKPOINT_OK', b'H5K: OWN_PAGING_OK', b'H5K: GUARD_FAULT_OK', b'H5K: USER_SYSCALL_OK', b'H5K: COOPERATIVE_SCHEDULER_OK', b'H5K: RAMFS_OK', b'H5K: COMPAT_FORMAT_PROBE_OK', b'H5K: PCI_SCAN_OK', b'H5K: DOUBLE_BUFFER_OK', b'H5K: FRAMEBUFFER_OK', b'H5K: DESKTOP_READY', b'H5K: TIMER_IRQ_OK', b'H5K: KERNEL_READY']

def validate(data):
    pos = 0
    for marker in REQUIRED:
        found = data.find(marker, pos)
        if found < 0:
            raise ValueError(f'Missing/out-of-order marker: {marker.decode()}')
        pos = found + len(marker)
    if b'H5K: PANIC' in data or b'H5K: EFI_STATUS=' in data or b'H5K: FAIL ' in data:
        raise ValueError('Kernel or boot failure marker present')

def firmware():
    for code, var in [('/usr/share/OVMF/OVMF_CODE_4M.fd','/usr/share/OVMF/OVMF_VARS_4M.fd'),('/usr/share/OVMF/OVMF_CODE.fd','/usr/share/OVMF/OVMF_VARS.fd')]:
        if pathlib.Path(code).is_file() and pathlib.Path(var).is_file():
            return code, var
    raise SystemExit('Matching non-Secure-Boot OVMF CODE/VARS firmware not found')

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--memory', type=int, default=256)
    parser.add_argument('--timeout', type=int, default=60)
    parser.add_argument('--interactive', action='store_true')
    args = parser.parse_args()
    if args.memory < 128 or args.timeout < 1:
        parser.error('memory must be >=128 MiB and timeout positive')
    code, var = firmware()
    img = ROOT/'dist/hi5os-v0.1.1.img'
    if not img.is_file(): raise SystemExit('Run scripts/build.sh first')
    log = ROOT / 'dist' / f'boot-{args.memory}m.log'
    log.unlink(missing_ok=True)
    with tempfile.TemporaryDirectory(prefix='hi5os-vm-') as tmp:
        variables = pathlib.Path(tmp)/'vars.fd'
        shutil.copyfile(var, variables)
        cmd = ['qemu-system-x86_64','-machine','q35,accel=tcg','-cpu','qemu64','-m',str(args.memory),'-smp','1', '-drive',f'if=pflash,format=raw,readonly=on,file={code}', '-drive',f'if=pflash,format=raw,file={variables}', '-drive',f'if=none,id=os,format=raw,readonly=on,file={img}', '-device','qemu-xhci','-device','usb-storage,drive=os,bootindex=1','-vga','std','-nic','none','-monitor','none','-no-reboot','-serial',f'file:{log}']
        if args.interactive:
            return subprocess.call(cmd)
        qmp = str(pathlib.Path(tmp)/'qmp.sock')
        proc = subprocess.Popen(cmd + ['-display','none','-qmp',f'unix:{qmp},server=on,wait=off'])
        try:
            end = time.monotonic()+args.timeout
            while time.monotonic() < end:
                data = log.read_bytes() if log.exists() else b''
                if b'H5K: KERNEL_READY' in data:
                    validate(data)
                    # Exercise real keyboard input and RAM workspace through QMP.
                    with socket.socket(socket.AF_UNIX) as sock:
                        sock.settimeout(5)
                        sock.connect(qmp)
                        stream = sock.makefile('rwb')
                        json.loads(stream.readline())
                        def call(name, args=None):
                            stream.write((json.dumps({'execute':name, **({'arguments':args} if args else {})})+'\n').encode());stream.flush()
                            while True:
                                reply=json.loads(stream.readline())
                                if 'error' in reply: raise RuntimeError(reply['error'])
                                if 'return' in reply:return reply['return']
                        call('qmp_capabilities')
                        call('screendump',{'filename':str(ROOT/'dist'/f'desktop-{args.memory}m.ppm')})
                        for key,marker in [('1',b'GUI_SYSTEM_OPEN'),('2',b'GUI_FILES_OPEN'),('n',b'GUI_FILE_CREATED'),('d',b'GUI_FILE_DELETED'),('3',b'GUI_NETWORK_OPEN'),('m',b'GUI_MENU_TOGGLED'),('t',b'GUI_THEME_DARK'),('4',b'GUI_TERMINAL_OPEN')]:
                            call('human-monitor-command',{'command-line':'sendkey '+key})
                            deadline=time.monotonic()+5
                            while marker not in log.read_bytes() and time.monotonic()<deadline:time.sleep(.1)
                            if marker not in log.read_bytes():raise RuntimeError('GUI input failed: '+key)
                        for key in ['h','e','l','p','ret']:
                            call('human-monitor-command',{'command-line':'sendkey '+key})
                            time.sleep(.18)
                        deadline=time.monotonic()+8
                        while b'SHELL_HELP_OK' not in log.read_bytes() and time.monotonic()<deadline:time.sleep(.1)
                        if b'SHELL_HELP_OK' not in log.read_bytes():raise RuntimeError('Shell command failed')
                        call('screendump',{'filename':str(ROOT/'dist'/f'terminal-{args.memory}m.ppm')})
                        call('human-monitor-command',{'command-line':'sendkey esc'})
                        time.sleep(.25)
                        call('human-monitor-command',{'command-line':'sendkey 1'})
                        time.sleep(.25)
                        call('human-monitor-command',{'command-line':'sendkey x'})
                        deadline=time.monotonic()+5
                        while b'GUI_MAXIMIZED' not in log.read_bytes() and time.monotonic()<deadline:time.sleep(.1)
                        if b'GUI_MAXIMIZED' not in log.read_bytes():raise RuntimeError('Maximise failed')
                        call('screendump',{'filename':str(ROOT/'dist'/f'settings-{args.memory}m.ppm')})
                    time.sleep(1)
                    if proc.poll() is not None: raise RuntimeError('QEMU exited after kernel-ready marker')
                    validate(log.read_bytes())
                    print(log.read_text(errors='replace'))
                    print(f'PASS: {args.memory} MiB UEFI post-handoff boot')
                    return 0
                if proc.poll() is not None: raise RuntimeError(f'QEMU exited: {proc.returncode}')
                if b'H5K: PANIC' in data or b'H5K: EFI_STATUS=' in data or b'H5K: FAIL ' in data: raise RuntimeError('Guest reported a boot failure')
                time.sleep(.2)
            raise RuntimeError('Timed out waiting for kernel-ready marker')
        finally:
            if proc.poll() is None:
                proc.terminate()
                try: proc.wait(timeout=5)
                except subprocess.TimeoutExpired: proc.kill(); proc.wait()
            if log.exists(): print(f'Boot evidence: {log}')

if __name__ == '__main__': raise SystemExit(main())
