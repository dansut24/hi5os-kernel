#!/usr/bin/env python3
"""VM-only QEMU boot qualification; never attach host disks or network."""
import argparse
import pathlib
import shutil
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
REQUIRED = [b'H5K: BOOT v0.0.1', b'H5K: GOP_FOUND', b'H5K: EXIT_BOOT_SERVICES_OK', b'H5K: KERNEL_ENTERED', b'H5K: MEMORY_MAP_OK', b'H5K: CPUID_MAX=', b'H5K: FRAMEBUFFER_OK', b'H5K: KERNEL_READY']

def validate(data):
    pos = 0
    for marker in REQUIRED:
        found = data.find(marker, pos)
        if found < 0:
            raise ValueError(f'Missing/out-of-order marker: {marker.decode()}')
        pos = found + len(marker)
    if b'H5K: PANIC' in data or b'H5K: EFI_STATUS=' in data:
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
    img = ROOT/'dist/hi5os-v0.0.1.img'
    if not img.is_file(): raise SystemExit('Run scripts/build.sh first')
    log = ROOT / 'dist' / f'boot-{args.memory}m.log'
    log.unlink(missing_ok=True)
    with tempfile.TemporaryDirectory(prefix='hi5os-vm-') as tmp:
        variables = pathlib.Path(tmp)/'vars.fd'
        shutil.copyfile(var, variables)
        cmd = ['qemu-system-x86_64','-machine','q35,accel=tcg','-cpu','qemu64','-m',str(args.memory),'-smp','1', '-drive',f'if=pflash,format=raw,readonly=on,file={code}', '-drive',f'if=pflash,format=raw,file={variables}', '-drive',f'if=none,id=os,format=raw,readonly=on,file={img}', '-device','qemu-xhci','-device','usb-storage,drive=os,bootindex=1','-vga','std','-nic','none','-monitor','none','-no-reboot','-serial',f'file:{log}']
        if args.interactive:
            return subprocess.call(cmd)
        proc = subprocess.Popen(cmd + ['-display','none'])
        try:
            end = time.monotonic()+args.timeout
            while time.monotonic() < end:
                data = log.read_bytes() if log.exists() else b''
                if b'H5K: KERNEL_READY' in data:
                    validate(data)
                    # Ensure the guest remains halted without a reset/crash.
                    time.sleep(1)
                    if proc.poll() is not None: raise RuntimeError('QEMU exited after kernel-ready marker')
                    validate(log.read_bytes())
                    print(log.read_text(errors='replace'))
                    print(f'PASS: {args.memory} MiB UEFI post-handoff boot')
                    return 0
                if proc.poll() is not None: raise RuntimeError(f'QEMU exited: {proc.returncode}')
                if b'H5K: PANIC' in data or b'H5K: EFI_STATUS=' in data: raise RuntimeError('Guest reported a boot failure')
                time.sleep(.2)
            raise RuntimeError('Timed out waiting for kernel-ready marker')
        finally:
            if proc.poll() is None:
                proc.terminate()
                try: proc.wait(timeout=5)
                except subprocess.TimeoutExpired: proc.kill(); proc.wait()
            if log.exists(): print(f'Boot evidence: {log}')

if __name__ == '__main__': raise SystemExit(main())
