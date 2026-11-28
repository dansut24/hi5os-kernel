#!/usr/bin/env python3
"""Isolated VM: fresh account, incorrect password, correct login, lock and cold restart."""
import shutil, subprocess, tempfile, time, socket, json, pathlib, hashlib, zlib
from smoke import ROOT, firmware, validate
def main():
    import sys
    absolute='--absolute' in sys.argv
    prefix='absolute-' if absolute else ''
    code,var=firmware()
    with tempfile.TemporaryDirectory(prefix="hi5os-account-") as td:
        tmp=pathlib.Path(td); data=tmp/"data.img"; shutil.copyfile(ROOT/"dist/hi5os-setup-data.img",data)
        for boot in (1,2):
            vars=tmp/f"vars{boot}.fd";shutil.copyfile(var,vars);qmp=tmp/f"qmp{boot}.sock";log=ROOT/"dist"/f"{prefix}account-{boot}.log";log.unlink(missing_ok=True)
            cmd=["qemu-system-x86_64","-machine","q35,accel=tcg,vmport=on" if absolute else "q35,accel=tcg","-cpu","max,+rdrand,vendor=Hi5AbsTestVM" if absolute else "max,+rdrand","-m","512","-smp","1","-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}","-drive",f"if=none,id=os,format=raw,readonly=on,file={ROOT/'dist/hi5os-v0.3.4.img'}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1","-drive",f"if=none,id=data,format=raw,file={data}","-device","ide-hd,bus=ide.1,drive=data,bootindex=2","-vga","std","-nic","none","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
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
                    if absolute:
                        wait(b"ABS_POINTER_READY",10)
                        assert any(v["absolute"] and v["current"] for v in call("query-mice")), "Absolute pointer not active"
                    def dims():
                        shot=tmp/"pointer.ppm";call("screendump",{"filename":str(shot)})
                        return tuple(map(int,shot.read_bytes().split(None,4)[1:3]))
                    def tap(x,y,setup=True):
                        width,height=dims();ox=(width-520)//2 if setup else 0;oy=(height-400)//2 if setup else 0
                        px=ox+x;py=oy+y
                        events=[{"type":"abs","data":{"axis":"x","value":round(px*32767/(width-1))}},{"type":"abs","data":{"axis":"y","value":round(py*32767/(height-1))}},{"type":"btn","data":{"down":True,"button":"left"}}]
                        call("input-send-event",{"events":events});time.sleep(.4)
                        call("input-send-event",{"events":[{"type":"btn","data":{"down":False,"button":"left"}}]});time.sleep(.4)
                    def submit():
                        if absolute:tap(260,332)
                        else:key("ret")
                    def key(k):call("human-monitor-command",{"command-line":"sendkey "+k});time.sleep(.23)
                    def typed(s):
                        for k in s:key(k)
                        submit()
                    # Public, synthetic CI-only credential; never a user password.
                    password="testpassword123"
                    if boot==1:
                        call("screendump",{"filename":str(ROOT/"dist"/f"{prefix}account-setup.ppm")})
                        if absolute:
                            tap(380,178);wait(b"SETUP_REGION_SELECTED=0x0000000000000001",5)
                        submit();typed("testuser");typed("hi5test");typed(password);typed(password)
                        if absolute:
                            tap(140,178);wait(b"SETUP_THEME_SELECTED=0x0000000000000000",5)
                        submit();wait(b"ACCOUNT_CREATED",180)
                    key("f4");assert b"GUI_TERMINAL_OPEN" not in log.read_bytes()
                    typed("incorrectpass");wait(b"ACCOUNT_LOGIN_REJECTED",180);time.sleep(4)
                    typed(password);wait(b"ACCOUNT_LOGIN_OK",180)
                    if absolute and boot==2:wait(b"APPEARANCE_RESTORED",5)
                    if absolute and boot==1:
                        width,height=dims();by=height-(54 if height>=700 else 0)-60
                        prior=len(log.read_bytes());tap(35,by+30,False);wait(b"GUI_MENU_TOGGLED",10,prior)
                        call("screendump",{"filename":str(ROOT/"dist"/"hi5shell-start.ppm")})
                        tap(100,by+30,False)
                        for c in "word":key(c)
                        prior=len(log.read_bytes());key("ret");wait(b"GUI_WORD_OPEN",10,prior)
                        call("screendump",{"filename":str(ROOT/"dist"/"hi5shell-search-word.ppm")})
                        key("esc")
                        key("f1");key("ctrl-1");wait(b"GRAPHICS_MODE_OK",10);key("esc")
                        width,height=dims();assert (width,height)==(640,480)
                        call("screendump",{"filename":str(ROOT/"dist"/"hi5shell-compact.ppm")})
                        prior=len(log.read_bytes());tap(width-100,85,False);wait(b"SHELL_NOTICE_OPEN",10,prior);key("esc")
                        key("f1");key("ctrl-3")
                        width,height=dims();assert (width,height)==(1024,768)
                        # Reflow resets the standard QEMU Settings window to these bounds.
                        by=height-54-60;wx=(width-560)//2+20;wy=(by-365)//2
                        def point(x,y):return [{"type":"abs","data":{"axis":"x","value":round(x*32767/(width-1))}},{"type":"abs","data":{"axis":"y","value":round(y*32767/(height-1))}}]
                        before=tmp/"before-drag.ppm";call("screendump",{"filename":str(before)})
                        prior=len(log.read_bytes());call("input-send-event",{"events":point(wx+100,wy+18)+[{"type":"btn","data":{"down":True,"button":"left"}}]});wait(b"COMPOSITOR_SURFACE_READY",10,prior)
                        for dx,dy in [(24,12),(48,24),(72,36)]:
                            call("input-send-event",{"events":point(wx+100+dx,wy+18+dy)});time.sleep(.2)
                        wait(b"COMPOSITOR_CACHED_FRAME",10,prior)
                        call("screendump",{"filename":str(ROOT/"dist"/"hi5compositor-drag.ppm")})
                        call("input-send-event",{"events":[{"type":"btn","data":{"down":False,"button":"left"}}]});wait(b"GUI_WINDOW_MOVED",10,prior);wait(b"COMPOSITOR_DRAG_FRAMES=",10,prior)
                        after=ROOT/"dist"/"hi5compositor-released.ppm";call("screendump",{"filename":str(after)})
                        def patch(path,x,y):
                            data=path.read_bytes().split(b"\n",3)[3];return b"".join(data[(j*width+x)*3:(j*width+x+24)*3] for j in range(y,y+20))
                        assert any(patch(before,wx+8,wy+5)==patch(after,wx+80+dx,wy+41+dy) for dx in range(-2,3) for dy in range(-2,3)),"Moved window icon differs"
                        assert patch(before,wx+8,wy+5)!=patch(after,wx+8,wy+5),"Old window left a visible ghost"
                        key("esc")
                        key("t");wait(b"APPEARANCE_SAVED",10)
                        call("screendump",{"filename":str(ROOT/"dist"/"hi5shell-responsive.ppm")})
                    key("f4");wait(b"GUI_TERMINAL_OPEN")
                    key("ctrl-shift-l");wait(b"ACCOUNT_LOCKED");call("screendump",{"filename":str(ROOT/"dist"/f"{prefix}account-lock-{boot}.ppm")})
                    prior=len(log.read_bytes());key("f1");assert b"GUI_SYSTEM_OPEN" not in log.read_bytes()[prior:]
                    typed(password);wait(b"ACCOUNT_LOGIN_OK",180,prior)
                    call("quit")
                proc.wait(timeout=10)
                with data.open("rb") as f:f.seek(12*512);b=f.read(512)
                assert b[:8]==b"H5ACC001" and b[16:24]==b"testuser" and b[48:55]==b"hi5test"
                assert zlib.crc32(b[:508])==int.from_bytes(b[508:],"little")
                assert b[96:128]==hashlib.pbkdf2_hmac("sha256",password.encode(),b[80:96],600000)
                assert password.encode() not in b
                if absolute:
                    with data.open("rb") as f:f.seek(17*512);pref=f.read(512)
                    assert pref[:8]==b"H5UI0001" and pref[12]==1 and zlib.crc32(pref[:508])==int.from_bytes(pref[508:],"little")
            finally:
                if proc.poll() is None:proc.terminate();proc.wait(timeout=10)
                if log.exists():print(log.read_text(errors="replace"))
    print("PASS: account setup, independent password verifier, cold sign-in, wrong password rejection and lock")
if __name__=="__main__":main()
