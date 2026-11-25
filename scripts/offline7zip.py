#!/usr/bin/env python3
"""UTM-equivalent offline package installation and actual trial launch."""
import pathlib,tempfile,shutil,subprocess,time,socket,json,struct,zlib,hashlib
from smoke import ROOT,firmware,validate
code,var=firmware()
with tempfile.TemporaryDirectory(prefix="hi5os-offline-") as td:
    tmp=pathlib.Path(td);data=tmp/"data.img";shutil.copyfile(ROOT/"dist/hi5os-data-v1.img",data)
    log=ROOT/"dist"/"7zip-offline.log";log.unlink(missing_ok=True)
    qmp=tmp/"qmp.sock";vars=tmp/"vars.fd";shutil.copyfile(var,vars)
    cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","qemu64","-m","512","-smp","1",
         "-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}",
         "-drive",f"if=none,id=os,format=raw,readonly=on,file={ROOT/'dist/hi5os-v0.2.3.img'}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1",
         "-drive",f"if=none,id=data,format=raw,file={data}","-device","ide-hd,bus=ide.1,drive=data,bootindex=2",
         "-nic","none","-vga","std","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
    proc=subprocess.Popen(cmd)
    def wait(marker,seconds=60):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            b=log.read_bytes() if log.exists() else b""
            if b"H5K: FAIL " in b or b"H5K: PANIC" in b or proc.poll() is not None:raise RuntimeError(f"Guest failed: {b!r}")
            if marker in b:return b
            time.sleep(.1)
        raise RuntimeError(f"Missing {marker!r}: {b!r}")
    try:
        validate(wait(b"H5K: KERNEL_READY"))
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(5);sock.connect(str(qmp));stream=sock.makefile("rwb");json.loads(stream.readline())
            def call(name,args=None):
                stream.write((json.dumps({"execute":name,**({"arguments":args} if args else {})})+"\n").encode());stream.flush()
                while True:
                    reply=json.loads(stream.readline())
                    if "error" in reply:raise RuntimeError(reply["error"])
                    if "return" in reply:return reply["return"]
            call("qmp_capabilities")
            def key(k):call("human-monitor-command",{"command-line":"sendkey "+k});time.sleep(.3)
            key("4");wait(b"GUI_TERMINAL_OPEN",5)
            for ch in "install7zip":key(ch)
            key("ret");wait(b"SHELL_INSTALL7ZIP_OK",120)
            for ch in "7za -h":key({" ":"spc","-":"minus"}.get(ch,ch))
            key("ret");wait(b"APP_7ZIP_LOADED",120);wait(b"TRIAL_ENTER_7ZIP",30);wait(b"SHELL_7ZIP_RETURNED",30)
            wait(b"TRIAL_DLL_RESOLVER_OK",2)
            call("screendump",{"filename":str(ROOT/"dist"/"7zip-offline-512m.ppm")});call("quit")
        proc.wait(timeout=10)
        assert proc.returncode==0
        with data.open("rb") as fh:
            fh.seek(8192*512);h=fh.read(512);body=fh.read(1328640)
        assert h[:8]==b"H5APP001" and struct.unpack_from("<I",h,12)[0]==len(body)
        assert hashlib.sha256(body).hexdigest()=="dd3757f31c97e33776f2590317d4fb12f2da075db3dbb1e3f94d0f2a01d84ffa"
        assert b"HTTP_DOWNLOAD_OK" not in log.read_bytes()
        print("PASS: offline original portable 7-Zip installation and controlled startup probe")
        if b"SEVENZIP_HELP_OK" not in log.read_bytes():print("::warning::Offline trial returned, help not qualified")
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:proc.wait(timeout=5)
            except subprocess.TimeoutExpired:proc.kill();proc.wait()
        if log.exists():print(log.read_text(errors="replace"))
