#!/usr/bin/env python3
"""Actual guest Ethernet/ARP/TCP/HTTP download over restricted QEMU SLIRP.
Host fixture is loopback only. No public internet or executable execution.
"""
import pathlib, tempfile, shutil, subprocess, time, socket, json, zlib, threading, http.server, struct, sys
from smoke import ROOT, firmware, validate
def fixture():
    b=bytearray(1024)
    b[:2]=b"MZ";struct.pack_into("<I",b,60,64);b[64:68]=b"PE\0\0"
    for off,val in [(68,0x8664),(70,1),(84,112),(86,2),(88,0x20b)]:struct.pack_into("<H",b,off,val)
    for off,val in [(104,4096),(144,8192),(148,512),(208,1),(212,4096),(216,512),(220,512)]:struct.pack_into("<I",b,off,val)
    b[512]=0xc3
    return bytes(b)
MODE=sys.argv[1] if len(sys.argv)>1 else "length"
assert MODE in ("length","close","chunked","large","sevenzip","msi","browse")
PAYLOAD=pathlib.Path(sys.argv[2]).read_bytes() if MODE in ("sevenzip","msi") else fixture()+ (bytes(range(256))*4092 if MODE=="large" else b"")
assert len(PAYLOAD)<=4*1024*1024
class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def do_GET(self):
        if MODE=="browse" and self.path in ("/form.html","/search?q=hi5+test"):
            body=(b"<html><body><center><form action='/search'><input name='q' type='text'><input type='submit' value='Search'></form></center></body></html>" if self.path=="/form.html" else b"<html><head><style>div{display:none}.result{display:block}.result a.link{color:#123456}</style></head><body><div class='result'><h1>Search received</h1><a class='link' href='/index.html'>Open real result</a></div></body></html>")
            if self.path.startswith("/search?"):self.server.form_received=True
            self.send_response(200);self.send_header("Content-Type","text/html");self.send_header("Content-Length",str(len(body)));self.send_header("Connection","close");self.end_headers();self.wfile.write(body);self.close_connection=True;return
        if MODE=="browse" and self.path in ("/module.html","/lib.js","/main.js"):
            body,ctype={
                "/module.html":(b"<html><body><p id='module'>Before</p><p id='promise'>Before</p><script type='module' src='/main.js'></script><script type='module' src='/lib.js'></script></body></html>","text/html"),
                "/lib.js":(b"export const value=42;","application/javascript"),
                "/main.js":(b"import {value} from './lib.js'; document.getElementById('module').textContent='Module '+value; Promise.resolve(value+1).then(v=>document.getElementById('promise').textContent='Promise '+v);","text/javascript"),
            }[self.path]
            if self.path.endswith(".js"):self.server.module_received=getattr(self.server,"module_received",0)+1
            self.send_response(200);self.send_header("Content-Type",ctype);self.send_header("Content-Length",str(len(body)));self.send_header("Connection","close");self.end_headers();self.wfile.write(body);self.close_connection=True;return
        if MODE=="browse" and self.path=="/dynamic.js":
            body=b"let externalValue=7; document.getElementById('external').textContent='External script OK';"
            self.server.script_received=True
            self.send_response(200);self.send_header("Content-Type","application/javascript");self.send_header("Content-Length",str(len(body)));self.send_header("Connection","close");self.end_headers();self.wfile.write(body);self.close_connection=True;return
        if MODE=="browse" and self.path=="/dynamic.html":
            body=(b"<html><body><p id='demo'>Before</p><div id='empty'></div><p id='external'>Before external</p><script>/*"+b"x"*(513*1024)+b"*/</script><script>throw new Error('fixture');</script><script src='/dynamic.js'></script><script src='https://example.invalid/no.js'></script><script>/*"+b"x"*(40*1024)+b"*/let n=[1,2,3,4].reduce((a,b)=>a+b,0)+externalValue;document.addEventListener('DOMContentLoaded',()=>{document.getElementById('demo').textContent='Calculated '+n;document.getElementById('empty').textContent='Empty node updated';let p=document.createElement('p');p.textContent='Generated content';document.body.appendChild(p);});</script></body></html>")
            self.send_response(200);self.send_header("Content-Type","text/html");self.send_header("Content-Length",str(len(body)));self.send_header("Connection","close");self.end_headers();self.wfile.write(body);self.close_connection=True;return
        if MODE=="browse" and self.path=="/index.html":
            body=b"<html><head><title>Hi5 test</title><link rel='stylesheet' href='/style.css'></head><body><h1 style='color:#112233'>Hi5Browser qualification</h1><p id='demo' class='copy'>Before</p><img src='/photo.png' alt='PNG'><img src='/photo.jpg' alt='JPEG'><script>document.getElementById('demo').textContent = 'After';</script><a href='/get.exe'>Download EXE</a></body></html>"
            self.send_response(200);self.send_header("Content-Type","text/html");self.send_header("Content-Length",str(len(body)));self.send_header("Connection","close");self.end_headers();self.wfile.write(body);self.close_connection=True;return
        if MODE=="browse" and self.path in ("/style.css","/photo.png","/photo.jpg"):
            if self.path=="/style.css":body=b".copy { color: #245678; }";ctype="text/css"
            else:
                import io
                from PIL import Image
                f=io.BytesIO();Image.new("RGB",(80,48),(230,50,70)).save(f,format="PNG" if self.path.endswith("png") else "JPEG");body=f.getvalue();ctype="image/png" if self.path.endswith("png") else "image/jpeg"
            self.send_response(200);self.send_header("Content-Type",ctype);self.send_header("Content-Length",str(len(body)));self.send_header("Connection","close");self.end_headers();self.wfile.write(body);self.close_connection=True;return
        if MODE=="browse" and self.path=="/image.bmp":
            body=b"BM"+struct.pack("<IHHI",58,0,0,54)+struct.pack("<IiiHHIIiiII",40,1,1,1,24,0,4,0,0,0,0)+bytes([0,0,255,0])
            self.send_response(200);self.send_header("Content-Type","image/bmp");self.send_header("Content-Length",str(len(body)));self.send_header("Connection","close");self.end_headers();self.wfile.write(body);self.close_connection=True;return
        if MODE=="browse" and self.path=="/get.exe":
            self.send_response(302);self.send_header("Location","/hello.exe");self.send_header("Content-Length","0");self.send_header("Connection","close");self.end_headers();self.close_connection=True;return
        if self.path!="/hello.exe":self.send_error(404);return
        self.send_response(200);self.send_header("Content-Type","application/octet-stream")
        if MODE in ("length","large","sevenzip","msi","browse"):self.send_header("Content-Length",str(len(PAYLOAD)))
        elif MODE=="chunked":self.send_header("Transfer-Encoding","chunked")
        self.send_header("Connection","close");self.end_headers()
        if MODE=="chunked":
            for i in range(0,len(PAYLOAD),137):
                chunk=PAYLOAD[i:i+137]
                self.wfile.write(("%x\r\n"%len(chunk)).encode()+chunk+b"\r\n");self.wfile.flush()
            self.wfile.write(b"0\r\nX-Test: yes\r\n\r\n")
        else:self.wfile.write(PAYLOAD)
        self.close_connection=True
def main():
    code,var=firmware()
    with http.server.ThreadingHTTPServer(("127.0.0.1",0),Handler) as server,tempfile.TemporaryDirectory(prefix="hi5os-net-") as td:
        threading.Thread(target=server.serve_forever,daemon=True).start()
        tmp=pathlib.Path(td);data=tmp/"data.img";shutil.copyfile(ROOT/"dist/hi5os-data-v1.img",data)
        vars=tmp/"vars.fd";shutil.copyfile(var,vars);log=ROOT/"dist"/f"network-{MODE}.log";log.unlink(missing_ok=True)
        qmp=tmp/"qmp.sock";system=ROOT/"dist/hi5os-v0.3.4.img"
        # Close-delimited test uses real SLIRP host-loopback TCP, in CI network namespace.
        net="user,id=n0" if MODE in ("close","browse") else f"user,id=n0,restrict=on,guestfwd=tcp:10.0.2.200:8080-tcp:127.0.0.1:{server.server_port}"
        capture=ROOT/"dist"/f"network-{MODE}.pcap"
        cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","qemu64","-m","512","-smp","1",
        "-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}",
        "-drive",f"if=none,id=os,format=raw,readonly=on,file={system}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1",
        "-drive",f"if=none,id=data,format=raw,file={data}","-device","ide-hd,bus=ide.1,drive=data,bootindex=2",
        "-netdev",net,"-device","e1000,netdev=n0,mac=52:54:00:12:34:56",*([] if MODE in ("sevenzip","msi") else ["-object",f"filter-dump,id=cap,netdev=n0,file={capture}"]),
        "-vga","std","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
        first_log=None
        proc=subprocess.Popen(cmd)
        def wait(marker,seconds=30):
            end=time.monotonic()+seconds
            while time.monotonic()<end:
                b=log.read_bytes() if log.exists() else b""
                if marker in b:return b
                if proc.poll() is not None or b"H5K: FAIL " in b or b"H5K: PANIC" in b or b"H5K: HTTP_DOWNLOAD_FAILED" in b:raise RuntimeError(f"Guest failed: {b!r}")
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
                key("6");wait(b"GUI_BROWSER_OPEN",5)
                if MODE=="close":
                    key("ctrl-l")
                    for ch in f"http://10.0.2.2:{server.server_port}/hello.exe":
                        key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                if MODE=="browse":
                    key("ctrl-l")
                    for ch in f"http://10.0.2.2:{server.server_port}/index.html":key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                    key("ret");wait(b"BROWSER_PAGE_RENDERED",30);wait(b"BROWSER_LINKS=0x0000000000000001",5)
                    wait(b"BROWSER_STYLE_BLOCKS=0x0000000000000001",5)
                    key("ctrl-i");wait(b"BROWSER_STYLESHEETS_LOADED=0x0000000000000001",45);wait(b"BROWSER_IMAGES_LOADED=0x0000000000000002",45);wait(b"BROWSER_STYLE_BLOCKS=0x0000000000000002",5)
                    key("ctrl-shift-j");wait(b"BROWSER_LITERAL_UPDATES=0x0000000000000001",5)
                    call("screendump",{"filename":str(ROOT/"dist"/"browser-page-512m.ppm")})
                    key("ctrl-1");wait(b"BROWSER_LINK_OPENED",5);wait(b"HTTP_REDIRECT_FOLLOWED",30)
                else:key("ret")
                wait(b"DHCP_ACK_OK",20);wait(b"HTTP_DOWNLOAD_OK",240 if MODE in ("large","sevenzip","msi") else 20)
                wait(f"HTTP_DOWNLOAD_BYTES=0x{len(PAYLOAD):016x}".encode(),2)
                wait({"length":b"HTTP_LENGTH_OK","close":b"HTTP_CLOSE_OK","chunked":b"HTTP_CHUNKED_OK","large":b"HTTP_LENGTH_OK","sevenzip":b"HTTP_LENGTH_OK","msi":b"HTTP_LENGTH_OK","browse":b"HTTP_LENGTH_OK"}[MODE],2)
                if MODE=="close":wait(b"HTTP_FIN_BYTES=",2)
                if MODE=="msi":
                    key("esc");key("4");wait(b"GUI_TERMINAL_OPEN",5)
                    for ch in "msiinfo":key(ch)
                    key("ret");wait(b"SHELL_MSIINFO_OK",120)
                    call("screendump",{"filename":str(ROOT/"dist"/"msi-inspect-512m.ppm")});call("quit");proc.wait(timeout=10);return
                if MODE=="sevenzip":
                    key("esc");key("4");wait(b"GUI_TERMINAL_OPEN",5)
                    for ch in "peinfo":key(ch)
                    key("ret");wait(b"SHELL_PEINFO_OK",30)
                    wait(f"PE_FILE_CRC=0x{zlib.crc32(PAYLOAD):016x}".encode(),2)
                    call("screendump",{"filename":str(ROOT/"dist"/"7zip-imports-512m.ppm")})
                    for ch in "pereloc":key(ch)
                    key("ret");wait(b"SHELL_PERELOC_OK",60)
                    call("screendump",{"filename":str(ROOT/"dist"/"7zip-relocated-512m.ppm")})
                    for ch in "winfault":key(ch)
                    key("ret");wait(b"SHELL_WINFAULT_OK",30);wait(b"TRIAL_DLL_RESOLVER_OK",2)
                    for ch in "install7zip":key(ch)
                    key("ret");wait(b"SHELL_INSTALL7ZIP_OK",120)
                    # Real cold boot: keep -no-reboot to detect guest resets/triple faults.
                    call("quit");proc.wait(timeout=10)
                    if proc.returncode!=0:raise RuntimeError("First VM exited unsuccessfully")
                    first_log=log
                    log=ROOT/"dist"/"network-sevenzip-cold.log";log.unlink(missing_ok=True)
                    qmp2=tmp/"qmp-cold.sock";vars2=tmp/"vars-cold.fd";shutil.copyfile(var,vars2)
                    cmd2=cmd.copy()
                    cmd2[cmd2.index(f"if=pflash,format=raw,file={vars}")]=f"if=pflash,format=raw,file={vars2}"
                    cmd2[cmd2.index(f"file:{first_log}")]=f"file:{log}"
                    cmd2[cmd2.index(f"unix:{qmp},server=on,wait=off")]=f"unix:{qmp2},server=on,wait=off"
                    # No fixture forwarding on the second boot: executable must come from disk.
                    cmd2[cmd2.index(net)]="user,id=n0,restrict=on"
                    proc=subprocess.Popen(cmd2)
                    validate(wait(b"H5K: KERNEL_READY",90))
                    with socket.socket(socket.AF_UNIX) as sock2:
                        sock2.settimeout(5);sock2.connect(str(qmp2));stream=sock2.makefile("rwb");json.loads(stream.readline())
                        call("qmp_capabilities")
                        key("4");wait(b"GUI_TERMINAL_OPEN",5)
                        for ch in "7za -h":key({" ":"spc","-":"minus"}.get(ch,ch))
                        key("ret");wait(b"APP_7ZIP_LOADED",120);wait(b"TRIAL_ENTER_7ZIP",30);wait(b"SHELL_7ZIP_RETURNED",30)
                        assert b"HTTP_DOWNLOAD_OK" not in log.read_bytes(),"Unexpected second boot download"
                        call("screendump",{"filename":str(ROOT/"dist"/"7zip-trial-512m.ppm")});call("quit")
                    proc.wait(timeout=10)
                    if proc.returncode!=0:raise RuntimeError("Second VM exited unsuccessfully")
                    with data.open("rb") as fh:
                        fh.seek(8192*512);h=fh.read(512);body=fh.read(len(PAYLOAD))
                    assert h[:8]==b"H5APP001" and struct.unpack_from("<I",h,12)[0]==len(PAYLOAD)
                    assert struct.unpack_from("<I",h,508)[0]==zlib.crc32(h[:508])
                    assert body==PAYLOAD
                    print("PASS: exact original 7-Zip installed on disk and loaded after reboot; startup probe returned (help qualification separate)")
                    return
                if MODE=="browse":
                    assert log.read_bytes().count(b"HTTP_DOWNLOAD_OK")>=2 and log.read_bytes().count(b"NET_TCP_OK")>=3,"Missing independent page, redirect or download connection"
                key("f9");wait(b"DOWNLOAD_SAVED",90 if MODE=="large" else 10)
                wait(f"DOWNLOAD_SAVED_CRC=0x{zlib.crc32(PAYLOAD):016x}".encode(),2)
                key("f10");wait(b"GUI_DOWNLOAD_REOPENED",90 if MODE=="large" else 5)
                call("screendump",{"filename":str(ROOT/"dist"/f"browser-{MODE}-512m.ppm")})
                if MODE=="browse":
                    key("ctrl-l")
                    for ch in f"http://10.0.2.2:{server.server_port}/image.bmp":key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                    key("ret");wait(b"BROWSER_BITMAP_RENDERED",30)
                    call("screendump",{"filename":str(ROOT/"dist"/"browser-image-512m.ppm")})
                    key("ctrl-b");wait(b"BROWSER_HISTORY_NAVIGATED",30)
                    key("ctrl-t");wait(b"BROWSER_TAB_COUNT=0x0000000000000002",5)
                    key("ctrl-l")
                    for ch in f"http://10.0.2.2:{server.server_port}/form.html":key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                    key("ret");wait(b"WEB_CORE_FORM_READY",30);key("ctrl-g")
                    for ch in "hi5 test":key("spc" if ch==" " else ch)
                    call("screendump",{"filename":str(ROOT/"dist"/"browser-form-512m.ppm")})
                    prior=log.read_bytes().count(b"BROWSER_PAGE_RENDERED");key("ret");wait(b"WEB_CORE_FORM_SUBMITTED",5)
                    deadline=time.monotonic()+30
                    while not getattr(server,"form_received",False) and time.monotonic()<deadline:time.sleep(.1)
                    assert getattr(server,"form_received",False),"GET form submission did not reach server"
                    deadline=time.monotonic()+30
                    while log.read_bytes().count(b"BROWSER_PAGE_RENDERED")<=prior and time.monotonic()<deadline:time.sleep(.1)
                    assert log.read_bytes().count(b"BROWSER_PAGE_RENDERED")>prior,"Search response not rendered"
                    tail=log.read_bytes().rsplit(b"WEB_CORE_FORM_SUBMITTED",1)[-1]
                    assert b"BROWSER_LAYOUT_BLOCKS=0x0000000000000002" in tail,"Search result layout must contain two real blocks"
                    key("ctrl-s");wait(b"BROWSER_SETTINGS_OPEN",5)
                    call("screendump",{"filename":str(ROOT/"dist"/"browser-settings-512m.ppm")})
                    key("3");wait(b"BROWSER_DIAGNOSTICS=0x0000000000000001",5)
                    key("3");wait(b"BROWSER_DIAGNOSTICS=0x0000000000000000",5)
                    key("ctrl-s");wait(b"BROWSER_SETTINGS_CLOSED",5)
                    key("ctrl-tab");wait(b"BROWSER_TAB_ACTIVE=0x0000000000000000",30)
                    key("ctrl-tab");wait(b"BROWSER_TAB_CACHE_RESTORED",30)
                    key("ctrl-l")
                    for ch in f"http://10.0.2.2:{server.server_port}/dynamic.html":key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                    prior=log.read_bytes().count(b"BROWSER_PAGE_RENDERED");key("ret")
                    deadline=time.monotonic()+30
                    while log.read_bytes().count(b"BROWSER_PAGE_RENDERED")<=prior and time.monotonic()<deadline:time.sleep(.1)
                    assert log.read_bytes().count(b"BROWSER_PAGE_RENDERED")>prior,"Dynamic fixture did not render"
                    key("ctrl-j");wait(b"WEB_CORE_JS_EXECUTED=0x0000000000000002",90);wait(b"WEB_CORE_JS_DOM_UPDATES=0x0000000000000004",5)
                    wait(b"WEB_CORE_JS_EXTERNAL=0x0000000000000001",5);wait(b"WEB_CORE_JS_SKIPPED=0x0000000000000002",5);wait(b"WEB_CORE_JS_ERRORS=0x0000000000000001",5)
                    assert getattr(server,"script_received",False),"External script request missing"
                    call("screendump",{"filename":str(ROOT/"dist"/"browser-dynamic-512m.ppm")})
                    key("ctrl-l")
                    for ch in f"http://10.0.2.2:{server.server_port}/module.html":key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                    prior=log.read_bytes().count(b"BROWSER_PAGE_RENDERED");key("ret")
                    deadline=time.monotonic()+30
                    while log.read_bytes().count(b"BROWSER_PAGE_RENDERED")<=prior and time.monotonic()<deadline:time.sleep(.1)
                    assert log.read_bytes().count(b"BROWSER_PAGE_RENDERED")>prior,"Module fixture did not render"
                    prior=len(log.read_bytes());key("ctrl-j");deadline=time.monotonic()+90
                    while b"WEB_CORE_JS_MODULES=0x0000000000000002" not in log.read_bytes()[prior:] and time.monotonic()<deadline:time.sleep(.1)
                    tail=log.read_bytes()[prior:]
                    assert b"WEB_CORE_JS_MODULES=0x0000000000000002" in tail and b"WEB_CORE_JS_DOM_UPDATES=0x0000000000000002" in tail and b"WEB_CORE_JS_MODULE_ERRORS=0x0000000000000000" in tail,"Module import or Promise DOM update failed"
                    assert getattr(server,"module_received",0)==2,"Module resources not downloaded"
                    call("screendump",{"filename":str(ROOT/"dist"/"browser-module-512m.ppm")})
                    key("ctrl-w");wait(b"BROWSER_TAB_COUNT=0x0000000000000001",30)
                call("quit")
            proc.wait(timeout=10)
            with data.open("rb") as fh:
                fh.seek(64*512);h=fh.read(512);body=fh.read(len(PAYLOAD))
            assert h[:8]==b"H5DL0002" and struct.unpack_from("<I",h,12)[0]==len(PAYLOAD)
            assert body==PAYLOAD
            url=(f"http://10.0.2.2:{server.server_port}/hello.exe" if MODE in ("close","browse") else "http://10.0.2.200:8080/hello.exe").encode()
            assert struct.unpack_from("<I",h,20)[0]==len(url) and h[24:24+len(url)]==url
            assert struct.unpack_from("<I",h,508)[0]==zlib.crc32(h[:508])
            print(f"PASS: {MODE} HTTP downloaded binary and saved byte-exact fixture to dedicated data image")
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
            if first_log is not None:
                # Retain independent cold boot log and combined evidence for sevenzip.py.
                if log.exists():
                    with first_log.open("ab") as fh:fh.write(b"\n--- COLD BOOT ---\n"+log.read_bytes())
                log=first_log
            if log.exists():print(log.read_text(errors="replace"))
            server.shutdown()
if __name__=="__main__":main()
