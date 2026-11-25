#!/usr/bin/env python3
"""Two cold boots with the same temporary, dedicated data disk."""
import pathlib, tempfile, shutil, subprocess, time, socket, json, zlib
from smoke import ROOT, firmware, validate
def main():
    code,var=firmware()
    with tempfile.TemporaryDirectory(prefix="hi5os-data-") as td:
        tmp=pathlib.Path(td);data=tmp/"data.img"
        shutil.copyfile(ROOT/"dist/hi5os-data-v1.img",data)
        system=ROOT/"dist/hi5os-v0.1.3.img"
        for boot in (1,2):
            vars=tmp/f"vars{boot}.fd";shutil.copyfile(var,vars)
            log=ROOT/"dist"/f"persistence-{boot}.log";log.unlink(missing_ok=True)
            qmp=tmp/f"qmp{boot}.sock"
            cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","qemu64","-m","512","-smp","1",
                "-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}",
                "-drive",f"if=none,id=os,format=raw,readonly=on,file={system}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1",
                "-drive",f"if=none,id=data,format=raw,file={data}","-device","ide-hd,bus=ide.1,drive=data,bootindex=2",
                "-vga","std","-nic","none","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
            proc=subprocess.Popen(cmd)
            def wait(marker,seconds=60):
                end=time.monotonic()+seconds
                while time.monotonic()<end:
                    b=log.read_bytes() if log.exists() else b""
                    if marker in b:return b
                    if proc.poll() is not None or b"H5K: FAIL " in b or b"H5K: PANIC" in b:raise RuntimeError(f"Guest failed: {b!r}")
                    time.sleep(.1)
                raise RuntimeError(f"Missing {marker!r}: {b!r}")
            try:
                validate(wait(b"H5K: KERNEL_READY"))
                if boot==1:
                    wait(b"H5K: DATA_DISK_EMPTY",5)
                    with socket.socket(socket.AF_UNIX) as sock:
                        sock.settimeout(5);sock.connect(str(qmp));stream=sock.makefile("rwb");json.loads(stream.readline())
                        def call(name,args=None):
                            stream.write((json.dumps({"execute":name,**({"arguments":args} if args else {})})+"\n").encode());stream.flush()
                            while True:
                                reply=json.loads(stream.readline())
                                if "error" in reply:raise RuntimeError(reply["error"])
                                if "return" in reply:return reply["return"]
                        call("qmp_capabilities")
                        def key(k):
                            call("human-monitor-command",{"command-line":"sendkey "+k});time.sleep(.25)
                        key("5");wait(b"GUI_NOTEPAD_OPEN",5)
                        for _ in range(len(b"WELCOME TO HI5OS")):key("backspace")
                        for k in "persist":key(k)
                        key("f6");wait(b"GUI_NOTE_SAVED",10)
                        wait(f"H5K: DATA_SAVED_CRC=0x{zlib.crc32(b'persist'):016x}".encode(),2)
                        key("f7");wait(b"GUI_FILE_SELECTED=0x0000000000000001",5)
                        for k in "office":key(k)
                        key("f6");wait(f"H5K: DATA_SAVED_CRC=0x{zlib.crc32(b'office'):016x}".encode(),5)
                        call("screendump",{"filename":str(ROOT/"dist"/"notepad-512m.ppm")})
                        call("quit")
                    proc.wait(timeout=10)
                    with data.open("rb") as fh:
                        fh.seek(512);record=fh.read(512);fh.seek(3*512);second=fh.read(512)
                    assert record[:8]==b"H5NOTE01" and record[20:27]==b"persist"
                    assert int.from_bytes(record[508:512],"little")==zlib.crc32(record[:508])
                    assert second[:8]==b"H5NOTE01" and second[20:26]==b"office"
                    assert int.from_bytes(second[508:512],"little")==zlib.crc32(second[:508])
                else:
                    wait(b"H5K: DATA_DISK_LOADED",2)
                    wait(f"H5K: DATA_CRC=0x{zlib.crc32(b'persist'):016x}".encode(),2)
                    wait(b"H5K: DATA_LENGTH=0x0000000000000007",2)
                    wait(f"H5K: DATA_CRC=0x{zlib.crc32(b'office'):016x}".encode(),2)
                    print("PASS: two typed documents survived cold restart")
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    try:proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:proc.kill();proc.wait()
                if log.exists():print(log.read_text(errors="replace"))
if __name__=="__main__":main()
