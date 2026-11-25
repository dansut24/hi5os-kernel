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
REQUIRED = [b'H5K: BOOT v0.2.7', b'H5K: GOP_FOUND', b'H5K: EXIT_BOOT_SERVICES_OK', b'H5K: KERNEL_ENTERED', b'H5K: STACK_OK', b'H5K: MEMORY_MAP_OK', b'H5K: ALLOCATOR_OK', b'H5K: FRAME_RW_REUSE_OK', b'H5K: STACK_CANARY_OK', b'H5K: CPUID_MAX=', b'H5K: GDT_IDT_BREAKPOINT_OK', b'H5K: OWN_PAGING_OK', b'H5K: GUARD_FAULT_OK', b'H5K: USER_SYSCALL_OK', b'H5K: HEAP_ALLOC_FREE_OK', b'H5K: SECURITY_FOUNDATIONS_OK', b'H5K: SHEET_FORMULAS_OK', b'H5K: COOPERATIVE_SCHEDULER_OK', b'H5K: RAMFS_OK', b'H5K: COMPAT_FORMAT_PROBE_OK', b'H5K: PE_STAGE_OK', b'H5K: PE_IMPORT_KERNEL32_EXITPROCESS_RESOLVED', b'H5K: PE_EXIT_CODE=0x000000000000002a', b'H5K: PE_WINDOWS_FIXTURE_EXECUTED', b'H5K: WINDOW_OBJECT_TEST_OK', b'H5K: MSI_CONTAINER_TEST_OK', b'H5K: PE_IMPORT_INVENTORY_OK', b'H5K: PE_RELOC_SELFTEST_OK', b'H5K: PE_RUNTIME_FOUR_IMPORTS_RESOLVED', b'H5K: PE_RUNTIME_EXIT=0x000000000000002a', b'H5K: PE_RUNTIME_EXECUTED', b'H5K: PE_CONSOLE_IMPORTS_RESOLVED', b'H5K: PE_CONSOLE_EXIT=0x000000000000002a', b'H5K: PE_CONSOLE_OUTPUT=Hello from Windows console!', b'H5K: PE_CONSOLE_EXECUTED', b'H5K: PCI_SCAN_OK', b'H5K: DOUBLE_BUFFER_OK', b'H5K: FRAMEBUFFER_OK', b'H5K: DESKTOP_READY', b'H5K: TIMER_IRQ_OK', b'H5K: NET_FORMAT_TEST_OK', b'H5K: KERNEL_READY']

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
    parser.add_argument('--cpu', default='qemu64')
    parser.add_argument('--security-test', choices=['available','unavailable'])
    args = parser.parse_args()
    if args.memory < 128 or args.timeout < 1:
        parser.error('memory must be >=128 MiB and timeout positive')
    code, var = firmware()
    img = ROOT/'dist/hi5os-v0.2.7.img'
    if not img.is_file(): raise SystemExit('Run scripts/build.sh first')
    log = ROOT / 'dist' / f'boot-{args.memory}m.log'
    log.unlink(missing_ok=True)
    with tempfile.TemporaryDirectory(prefix='hi5os-vm-') as tmp:
        variables = pathlib.Path(tmp)/'vars.fd'
        shutil.copyfile(var, variables)
        cmd = ['qemu-system-x86_64','-machine','q35,accel=tcg','-cpu',args.cpu,'-m',str(args.memory),'-smp','1', '-drive',f'if=pflash,format=raw,readonly=on,file={code}', '-drive',f'if=pflash,format=raw,file={variables}', '-drive',f'if=none,id=os,format=raw,readonly=on,file={img}', '-device','qemu-xhci','-device','usb-storage,drive=os,bootindex=1','-vga','std','-nic','none','-monitor','none','-no-reboot','-serial',f'file:{log}']
        if args.security_test: cmd += ['-rtc','base=2026-01-02T03:04:05,clock=vm']
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
                    if args.security_test:
                        import re, datetime
                        m=re.search(rb'H5K: CLOCK_EPOCH=0x([0-9a-fA-F]+)',data)
                        expected=int(datetime.datetime(2026,1,2,3,4,5,tzinfo=datetime.timezone.utc).timestamp())
                        if not m or not expected<=int(m.group(1),16)<=expected+30:raise RuntimeError(f'Unexpected VM clock: {m}')
                        if args.security_test=='available' and not any(k in data for k in [b'H5K: ENTROPY_RDSEED',b'H5K: ENTROPY_RDRAND']):raise RuntimeError('Hardware entropy expected')
                        if args.security_test=='unavailable' and b'H5K: ENTROPY_UNAVAILABLE' not in data:raise RuntimeError('Entropy should fail closed')
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
                        if args.security_test=='available':
                            def type_command(cmd):
                                for ch in cmd:
                                    key={' ':'spc','=':'equal','(':'shift-9',')':'shift-0',':':'shift-semicolon'}.get(ch,ch)
                                    call('human-monitor-command',{'command-line':'sendkey '+key});time.sleep(.18)
                                call('human-monitor-command',{'command-line':'sendkey ret'});time.sleep(.3)
                            type_command('security')
                            type_command('pehello')
                            if b'SHELL_PEINFO_OK' not in log.read_bytes():raise RuntimeError('PE inventory shell failed')
                            type_command('winconsole')
                            if b'SHELL_WINCONSOLE_OK' not in log.read_bytes():raise RuntimeError('Windows console interactive execution failed')
                            type_command('winruntime')
                            if b'SHELL_WINRUNTIME_OK' not in log.read_bytes():raise RuntimeError('Windows runtime interactive execution failed')
                            type_command('winhello')
                            if b'SHELL_WINHELLO_OK' not in log.read_bytes():raise RuntimeError('Windows fixture interactive execution failed')
                            type_command('set a1 12')
                            type_command('set a2 30')
                            type_command('set b1 =sum(a1:a2)')
                            deadline=time.monotonic()+5
                            while b'SHEET_RESULT=0x000000000000002a' not in log.read_bytes() and time.monotonic()<deadline:time.sleep(.1)
                            if b'SHEET_RESULT=0x000000000000002a' not in log.read_bytes():raise RuntimeError('Hi5Sheet interactive SUM failed')
                            call('human-monitor-command',{'command-line':'sendkey f11'});time.sleep(.3)
                            if b'GUI_SHEET_OPEN' not in log.read_bytes():raise RuntimeError('Hi5Sheet GUI open failed')
                            for key in ['2','0','ret','tab']:
                                call('human-monitor-command',{'command-line':'sendkey '+key});time.sleep(.25)
                            if b'GUI_SHEET_VALUE=0x0000000000000014' not in log.read_bytes():raise RuntimeError('Hi5Sheet GUI cell update failed')
                            call('screendump',{'filename':str(ROOT/'dist'/'sheet-512m.ppm')})
                            call('human-monitor-command',{'command-line':'sendkey esc'});time.sleep(.25)
                            call('human-monitor-command',{'command-line':'sendkey 4'});time.sleep(.25)
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
