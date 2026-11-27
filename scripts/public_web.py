#!/usr/bin/env python3
"""Optional direct BBC HTTPS diagnostic. No proxy, cookies, or credentials."""
import pathlib,tempfile,shutil,subprocess,time,socket,json,sys
from smoke import ROOT,firmware,validate
def main():
    google="--google" in sys.argv;name="google" if google else "bbc";address="https://www.google.com/" if google else "https://www.bbc.co.uk/"
    code,var=firmware()
    with tempfile.TemporaryDirectory(prefix="hi5-bbc-") as td:
        t=pathlib.Path(td);vars=t/"vars.fd";shutil.copyfile(var,vars);qmp=t/"qmp.sock";log=ROOT/"dist"/f"{name}-public.log";log.unlink(missing_ok=True)
        cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","max,+rdrand","-m","512","-smp","1","-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}","-drive",f"if=none,id=os,format=raw,readonly=on,file={ROOT/'dist/hi5os-v0.3.4.img'}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1","-netdev","user,id=n0","-device","e1000,netdev=n0,mac=52:54:00:12:34:56","-vga","std","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
        p=subprocess.Popen(cmd)
        try:
            end=time.monotonic()+60
            while time.monotonic()<end:
                b=log.read_bytes() if log.exists() else b""
                if b"H5K: KERNEL_READY" in b:break
                if p.poll() is not None:raise RuntimeError("VM exited")
                time.sleep(.1)
            validate(b)
            with socket.socket(socket.AF_UNIX) as s:
                s.settimeout(5);s.connect(str(qmp));f=s.makefile("rwb");json.loads(f.readline())
                def call(name,args=None):
                    f.write((json.dumps({"execute":name,**({"arguments":args} if args else {})})+"\n").encode());f.flush()
                    while True:
                        v=json.loads(f.readline())
                        if "error" in v:raise RuntimeError(v["error"])
                        if "return" in v:return v["return"]
                call("qmp_capabilities")
                def key(k):call("human-monitor-command",{"command-line":"sendkey "+k});time.sleep(.25)
                key("6");key("ctrl-l")
                for ch in address:key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                key("ret")
                end=time.monotonic()+85
                while time.monotonic()<end:
                    b=log.read_bytes()
                    if b"BROWSER_PAGE_RENDERED" in b or b"HTTP_DOWNLOAD_FAILED" in b or p.poll() is not None:break
                    time.sleep(.2)
                call("screendump",{"filename":str(ROOT/"dist"/f"{name}-public.ppm")})
                assert b"TLS_VERIFIED" in b and b"HTTP_STATUS=0x00000000000000c8" in b and b"BROWSER_PAGE_RENDERED" in b,"Public BBC not yet qualified"
                if google:
                    assert b"WEB_CORE_FORM_READY" in b,"Public Google did not expose a supported GET search form"
                    print("PASS: Google HTTPS and GET search field recognised")
                else:print("PASS: BBC HTTPS HTTP 200 and HTML rendered by Hi5Browser")
                call("quit")
        finally:
            if p.poll() is None:p.terminate();p.wait(timeout=10)
            if log.exists():print(log.read_text(errors="replace"))
if __name__=="__main__":main()
