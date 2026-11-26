#!/usr/bin/env python3
"""Isolated VM: fresh account, incorrect password, correct login, lock and cold restart."""
import shutil, subprocess, tempfile, time, socket, json, pathlib, hashlib, zlib
from smoke import ROOT, firmware, validate
def main():
    code,var=firmware()
    with tempfile.TemporaryDirectory(prefix="hi5os-account-") as td:
        tmp=pathlib.Path(td); data=tmp/"data.img"; shutil.copyfile(ROOT/"dist/hi5os-setup-data.img",data)
        for boot in (1,2):
            vars=tmp/f"vars{boot}.fd";shutil.copyfile(var,vars);qmp=tmp/f"qmp{boot}.sock";log=ROOT/"dist"/f"account-{boot}.log";log.unlink(missing_ok=True)
            cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","max,+rdrand","-m","512","-smp","1","-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}","-drive",f"if=none,id=os,format=raw,readonly=on,file={ROOT/'dist/hi5os-v0.3.4.img'}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1","-drive",f"if=none,id=data,format=raw,file={data}","-device","ide-hd,bus=ide.1,drive=data,bootindex=2","-vga","std","-nic","none","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
            proc=subprocess.Popen(cmd)
            def wait(mark,timeout=120,prior=0):
                end=time.monotonic()+timeout
                while time.monotonic()<end:
                    b=log.read_bytes() if log.exists() else b""
                    if proc.poll() is not None or b"H5K: PANIC" in b or b"H5K: FAIL " in b:raise RuntimeError("Guest failed")
                    if mark in b[prior:]:return b
                    time.sleep(.1)
                raise RuntimeError(f"Missing {mark!r}")
            try:
                validate(wait(b"H5K: KERNEL_READY",90));wait(b"ACCOUNT_SETUP_READY" if boot==1 else b"ACCOUNT_LOGIN_READY")
                with socket.socket(socket.AF_UNIX) as sock:
                    sock.settimeout(10);sock.connect(str(qmp));stream=sock.makefile("rwb");json.loads(stream.readline())
                    def call(name,args=None):
                        stream.write((json.dumps({"execute":name,**({"arguments":args} if args else {})})+"\n").encode());stream.flush()
                        while True:
                            r=json.loads(stream.readline())
                            if "error" in r:raise RuntimeError(r["error"])
                            if "return" in r:return r["return"]
                    call("qmp_capabilities")
                    def key(k):call("human-monitor-command",{"command-line":"sendkey "+k});time.sleep(.23)
                    def typed(s):
                        for k in s:key(k)
                        key("ret")
                    # Public, synthetic CI-only credential; never a user password.
                    password="testpassword123"
                    if boot==1:
                        call("screendump",{"filename":str(ROOT/"dist"/"account-setup.ppm")})
                        key("ret");typed("testuser");typed("hi5test");typed(password);typed(password);key("ret");wait(b"ACCOUNT_CREATED",180)
                    key("f4");assert b"GUI_TERMINAL_OPEN" not in log.read_bytes()
                    typed("incorrectpass");wait(b"ACCOUNT_LOGIN_REJECTED",180);time.sleep(4)
                    typed(password);wait(b"ACCOUNT_LOGIN_OK",180);key("f4");wait(b"GUI_TERMINAL_OPEN")
                    key("ctrl-shift-l");wait(b"ACCOUNT_LOCKED");call("screendump",{"filename":str(ROOT/"dist"/f"account-lock-{boot}.ppm")})
                    prior=len(log.read_bytes());key("f1");assert b"GUI_SYSTEM_OPEN" not in log.read_bytes()[prior:]
                    typed(password);wait(b"ACCOUNT_LOGIN_OK",180,prior)
                    call("quit")
                proc.wait(timeout=10)
                with data.open("rb") as f:f.seek(12*512);b=f.read(512)
                assert b[:8]==b"H5ACC001" and b[16:24]==b"testuser" and b[48:55]==b"hi5test"
                assert zlib.crc32(b[:508])==int.from_bytes(b[508:],"little")
                assert b[96:128]==hashlib.pbkdf2_hmac("sha256",password.encode(),b[80:96],600000)
                assert password.encode() not in b
            finally:
                if proc.poll() is None:proc.terminate();proc.wait(timeout=10)
                if log.exists():print(log.read_text(errors="replace"))
    print("PASS: account setup, independent password verifier, cold sign-in, wrong password rejection and lock")
if __name__=="__main__":main()
