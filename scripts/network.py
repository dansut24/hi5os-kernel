#!/usr/bin/env python3
"""Actual guest Ethernet/ARP/TCP/HTTP download over restricted QEMU SLIRP.
Host fixture is loopback only. No public internet or executable execution.
"""
import pathlib, tempfile, shutil, subprocess, time, socket, json, zlib, threading, http.server, struct
from smoke import ROOT, firmware, validate
def fixture():
    b=bytearray(1024)
    b[:2]=b"MZ";struct.pack_into("<I",b,60,64);b[64:68]=b"PE\0\0"
    for off,val in [(68,0x8664),(70,1),(84,112),(86,2),(88,0x20b)]:struct.pack_into("<H",b,off,val)
    for off,val in [(104,4096),(144,8192),(148,512),(208,1),(212,4096),(216,512),(220,512)]:struct.pack_into("<I",b,off,val)
    b[512]=0xc3
    return bytes(b)
PAYLOAD=fixture()
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path!="/hello.exe":self.send_error(404);return
        self.send_response(200);self.send_header("Content-Type","application/octet-stream");self.send_header("Content-Length",str(len(PAYLOAD)));self.end_headers();self.wfile.write(PAYLOAD)
def main():
    code,var=firmware()
    with http.server.HTTPServer(("127.0.0.1",0),Handler) as server,tempfile.TemporaryDirectory(prefix="hi5os-net-") as td:
        threading.Thread(target=server.serve_forever,daemon=True).start()
        tmp=pathlib.Path(td);data=tmp/"data.img";shutil.copyfile(ROOT/"dist/hi5os-data-v1.img",data)
        vars=tmp/"vars.fd";shutil.copyfile(var,vars);log=ROOT/"dist"/"network-lab.log";log.unlink(missing_ok=True)
        qmp=tmp/"qmp.sock";system=ROOT/"dist/hi5os-v0.1.3.img"
        cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","qemu64","-m","512","-smp","1",
        "-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}",
        "-drive",f"if=none,id=os,format=raw,readonly=on,file={system}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1",
        "-drive",f"if=none,id=data,format=raw,file={data}","-device","ide-hd,bus=ide.1,drive=data,bootindex=2",
        "-netdev",f"user,id=n0,restrict=on,guestfwd=tcp:10.0.2.100:8080-tcp:127.0.0.1:{server.server_port}","-device","e1000,netdev=n0,mac=52:54:00:12:34:56",
        "-vga","std","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
        proc=subprocess.Popen(cmd)
        def wait(marker,seconds=30):
            end=time.monotonic()+seconds
            while time.monotonic()<end:
                b=log.read_bytes() if log.exists() else b""
                if marker in b:return b
                if proc.poll() is not None or b"H5K: FAIL " in b or b"H5K: PANIC" in b:raise RuntimeError(f"Guest failed: {b!r}")
                time.sleep(.1)
            raise RuntimeError(f"Missing {marker!r}: {b!r}")
        try:
            validate(wait(b"H5K: KERNEL_READY",60))
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
                key("6");wait(b"GUI_BROWSER_OPEN",5);key("ret");wait(b"DHCP_ACK_OK",20);wait(b"HTTP_DOWNLOAD_OK",20)
                wait(b"HTTP_DOWNLOAD_BYTES=0x0000000000000400",2)
                key("f9");wait(b"DOWNLOAD_SAVED",10)
                wait(f"DOWNLOAD_SAVED_CRC=0x{zlib.crc32(PAYLOAD):016x}".encode(),2)
                key("f10");wait(b"GUI_DOWNLOAD_REOPENED",5)
                call("screendump",{"filename":str(ROOT/"dist"/"browser-512m.ppm")});call("quit")
            proc.wait(timeout=10)
            with data.open("rb") as fh:
                fh.seek(16*512);h=fh.read(512);body=fh.read(4096)
            assert h[:8]==b"H5DL0001" and struct.unpack_from("<I",h,12)[0]==len(PAYLOAD)
            assert body[:len(PAYLOAD)]==PAYLOAD
            assert struct.unpack_from("<I",h,508)[0]==zlib.crc32(h[:508])
            print("PASS: guest HTTP downloaded binary and saved byte-exact fixture to dedicated data image")
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
            if log.exists():print(log.read_text(errors="replace"))
            server.shutdown()
if __name__=="__main__":main()
