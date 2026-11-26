#!/usr/bin/env python3
"""UTM-equivalent offline package installation and actual trial launch."""
import pathlib,tempfile,shutil,subprocess,time,socket,json,struct,zlib,hashlib,sys
CPU=next((a for a in sys.argv[1:] if not a.startswith("--")),"qemu64")
suffix="-extended" if "--extended" in sys.argv else ""
assert CPU in ("qemu64","max")
from smoke import ROOT,firmware,validate
code,var=firmware()
with tempfile.TemporaryDirectory(prefix="hi5os-offline-") as td:
    tmp=pathlib.Path(td);data=tmp/"data.img";shutil.copyfile(ROOT/"dist/hi5os-data-v1.img",data)
    log=ROOT/"dist"/f"7zip-archive-{CPU}{suffix}.log";log.unlink(missing_ok=True)
    qmp=tmp/"qmp.sock";vars=tmp/"vars.fd";shutil.copyfile(var,vars)
    cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu",CPU,"-m","512","-smp","1",
         "-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}",
         "-drive",f"if=none,id=os,format=raw,readonly=on,file={ROOT/'dist/hi5os-v0.3.4.img'}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1",
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
            wait(b"TRIAL_DLL_RESOLVER_OK",2);wait(b"TRIAL_CPU_FEATURES_OK",2);wait(b"TRIAL_CRITICAL_SECTION_OK",2);wait(b"TRIAL_VERSION_INFO_OK",2);wait(b"TRIAL_RESTRICTED_TOKEN_OK",2)
            assert b"SEVENZIP_HELP_OK" in log.read_bytes()
            assert b"TRIAL_HEAP_REUSE_OK" in log.read_bytes()
            assert b"TRIAL_NLS_CONVERSION_OK" in log.read_bytes()
            assert b"TRIAL_FILE_ABI_OK" in log.read_bytes()
            assert b"TRIAL_ARCHIVE_IO_TIME_OK" in log.read_bytes()
            assert b"TRIAL_USER_TEXT_OK" in log.read_bytes()
            def command(s,marker=b"SHELL_7ZIP_RETURNED",extra=None):
                prior=len(log.read_bytes())
                for ch in s:key({" ":"spc","-":"minus",".":"dot","=":"equal","/":"slash"}.get(ch,ch))
                key("ret");end=time.monotonic()+120
                while time.monotonic()<end:
                    tail=log.read_bytes()[prior:]
                    if marker in tail or (extra is not None and extra in tail) or b"SHELL_WORKSPACE_FAILED" in tail or b"SHELL_7ZIP_BLOCKED" in tail:return tail
                    if proc.poll() is not None or b"H5K: PANIC" in tail:raise RuntimeError(tail)
                    time.sleep(.1)
                raise RuntimeError(tail)
            results={}
            assert b"SHELL_ARCSEED_OK" in command("arcseed",b"SHELL_ARCSEED_OK")
            operations=["7za i","7za a -mx=1 -mmt=1 test.zip notes.txt","7za a -mx=1 -mmt=1 test.7z notes.txt","7za l test.zip","7za l test.7z","7za t test.zip","7za t test.7z","7za x -y -ooutzip test.zip","7za x -y -oout7z test.7z"]
            for op in operations:
                tail=command(op)
                ok=b"TRIAL_RESULT=0x0000000000000001" in tail and b"TRIAL_EXIT=0x0000000000000000" in tail and b"SHELL_WORKSPACE_FAILED" not in tail
                if op=="7za i":ok=ok and b"SEVENZIP_FORMATS_OK" in tail and b"Codecs:" in tail and b"Hashers:" in tail
                elif op.startswith("7za l "):ok=ok and b"notes.txt" in tail and b"1 files" in tail
                elif op.startswith(("7za a ","7za t ","7za x ")):ok=ok and b"Everything is Ok" in tail
                results[op]=ok
                print("ARCHIVE OP:",op,"PASS" if ok else "NOT QUALIFIED",flush=True)
            assert results["7za i"],"Regression: genuine supported-format output no longer qualified"
            results["arcverify"]=b"SHELL_ARCHIVE_VERIFY_OK" in command("arcverify",b"SHELL_ARCHIVE_VERIFY_OK")
            if "--extended" in sys.argv:
                assert b"SHELL_ARCSEED_OK" in command("arcbig",b"SHELL_ARCSEED_OK")
                for op in ["7za a -mx=1 -mmt=1 multi.zip batch","7za a -mx=1 -mmt=1 multi.7z batch","7za t multi.zip","7za t multi.7z"]:
                    tail=command(op);results[op]=b"TRIAL_RESULT=0x0000000000000001" in tail and b"TRIAL_EXIT=0x0000000000000000" in tail and b"Everything is Ok" in tail
                    print("EXTENDED OP:",op,results[op],flush=True)
                assert b"SHELL_ARCSEED_OK" in command("arcupdate",b"SHELL_ARCSEED_OK")
                for op in ["7za u -mx=1 -mmt=1 multi.zip batch/small.txt","7za u -mx=1 -mmt=1 multi.7z batch/small.txt","7za d multi.zip batch/large.bin","7za d multi.7z batch/large.bin"]:
                    tail=command(op);results[op]=b"TRIAL_RESULT=0x0000000000000001" in tail and b"TRIAL_EXIT=0x0000000000000000" in tail and b"Everything is Ok" in tail
                    print("EXTENDED OP:",op,results[op],flush=True)
                key("ctrl-f6");wait(b"GUI_WINDEMO_OPEN",30);results["compiled PE GUI/DLL"]=b"GUI_PE_DLL_EXECUTED" in log.read_bytes()
                assert results["compiled PE GUI/DLL"],"Regression: compiled graphical PE/DLL fixture failed"
                call("screendump",{"filename":str(ROOT/"dist"/"wingui-demo.ppm")})
                key("f4");tail=command("7zfm",b"SHELL_ORIGINAL_GUI_PROBED",extra=b"SHELL_ORIGINAL_GUI_BLOCKED")
                results["original File Manager runtime reached"]=b"ORIGINAL_GUI_RUNTIME_REACHED" in tail
                print("ORIGINAL GUI PROBE:",results["original File Manager runtime reached"],flush=True)
                # Exercise the actual display device before the mandatory GUI launch.
                key("f1");prior=len(log.read_bytes());key("ctrl-6")
                deadline=time.monotonic()+10
                while b"GRAPHICS_MODE_OK" not in log.read_bytes()[prior:] and time.monotonic()<deadline:time.sleep(.1)
                assert b"GRAPHICS_MODE_OK" in log.read_bytes()[prior:], "Full HD display mode unavailable"
                key("f4")
                # Explicit live test, independent of the diagnostic startup result.
                tail=command("7zlive",b"GUI_WINLIVE_OPEN",extra=b"SHELL_ORIGINAL_GUI_LIVE_BLOCKED")
                ready=b"ORIGINAL_GUI_LIVE_WAIT" in tail and b"GUI_WINLIVE_OPEN" in tail
                results["original live window"]=ready
                if ready:
                    screenshot=ROOT/"dist"/"7zip-original-live.ppm"
                    call("screendump",{"filename":str(screenshot)})
                    assert screenshot.read_bytes().split(None,4)[:3]==[b"P6",b"1920",b"1080"],"Live GUI screenshot is not Full HD"
                    time.sleep(5)
                    current=log.read_bytes().rsplit(b"ORIGINAL_GUI_LIVE_WAIT",1)[-1]
                    ready=proc.poll() is None and screenshot.is_file() and screenshot.stat().st_size>1024 and not any(m in current for m in (b"ORIGINAL_GUI_LIVE_END",b"ORIGINAL_GUI_SESSION_DISCARDED",b"SHELL_ORIGINAL_GUI_LIVE_BLOCKED",b"H5K: PANIC",b"H5K: FAIL "))
                    if ready:
                        call("screendump",{"filename":str(ROOT/"dist"/"7zip-original-live-persistent.ppm")})
                    # Separate interaction probe: actual guest key events and resulting list evidence.
                    if ready:
                        prior=len(log.read_bytes());key("down");key("ret");time.sleep(1)
                        tail=log.read_bytes()[prior:]
                        selected=b"GUI_LIST_SELECTED" in tail
                        navigated=b"GUI_LIST_ROW|C:" in tail and b"ORIGINAL_GUI_LIVE_END" not in tail
                        results["original list selection"]=selected
                        results["original Computer navigation (diagnostic)"]=navigated
                        print("ORIGINAL GUI INPUT:",selected,"NAVIGATION:",navigated,flush=True)
                        call("screendump",{"filename":str(ROOT/"dist"/"7zip-original-interaction.ppm")})
                    key("ctrl-esc");key("f4")
                results["original live persistence"]=ready
                print("ORIGINAL GUI LIVE:",ready,flush=True)
                key("f12");wait(b"GUI_STORE_OPEN",30);key("u");wait(b"STORE_UNINSTALL_OK",30);key("ret")
                results["portable uninstall blocks execution"]=b"SHELL_7ZIP_BLOCKED" in command("7za -h")
                key("f12");key("i");wait(b"STORE_INSTALL_OK",120);key("ret")
                results["portable reinstall"]=b"SEVENZIP_HELP_OK" in command("7za -h")
            call("screendump",{"filename":str(ROOT/"dist"/f"7zip-archive-{CPU}{suffix}-512m.ppm")});call("quit")
        proc.wait(timeout=10)
        assert proc.returncode==0
        with data.open("rb") as fh:
            fh.seek(8192*512);h=fh.read(512);body=fh.read(1328640)
        assert h[:8]==b"H5APP001" and struct.unpack_from("<I",h,12)[0]==len(body)
        assert hashlib.sha256(body).hexdigest()=="dd3757f31c97e33776f2590317d4fb12f2da075db3dbb1e3f94d0f2a01d84ffa"
        assert b"HTTP_DOWNLOAD_OK" not in log.read_bytes()
        # Independently parse the latest committed workspace snapshot, validate CRCs,
        # use host unzip and 7z (not guest output text) to verify contents.
        import zipfile,io
        candidates=[]
        with data.open("rb") as fh:
            for base in (16384,18432,20480,26624):
                fh.seek(base*512);hdr=fh.read(512)
                if hdr[:8]!=b"H5WORK01" or zlib.crc32(hdr[:508])!=struct.unpack_from("<I",hdr,508)[0]:continue
                gen,n,crc=struct.unpack_from("<III",hdr,8);body=fh.read(n)
                if zlib.crc32(body)==crc:candidates.append((gen,body))
        files={}
        if candidates:
            _,blob=max(candidates);assert blob[:8]==b"H5WFS001";at=12
            for _ in range(struct.unpack_from("<I",blob,8)[0]):
                ln,attr,n=struct.unpack_from("<III",blob,at);at+=36;name=blob[at:at+ln].decode("ascii");at+=ln
                body=blob[at:at+n];at+=n
                if attr!=16:files[name]=body
            assert at==len(blob)
        sample=b"Hi5OS genuine Windows archive qualification\r\nSecond line: ZIP and 7z round trip.\r\n"
        results["source bytes"]=files.get("notes.txt")==sample
        try:
            with zipfile.ZipFile(io.BytesIO(files["test.zip"])) as z:results["host ZIP verify"]=z.testzip() is None and z.read("notes.txt")==sample
        except (KeyError,zipfile.BadZipFile):results["host ZIP verify"]=False
        try:
            arc=tmp/"test.7z";arc.write_bytes(files["test.7z"])
            p=subprocess.run(["7z","x","-so",str(arc),"notes.txt"],capture_output=True,timeout=30)
            results["host 7z verify"]=p.returncode==0 and p.stdout==sample
        except (KeyError,subprocess.TimeoutExpired):results["host 7z verify"]=False
        results["extracted ZIP bytes"]=files.get("outzip\\notes.txt")==sample
        results["extracted 7z bytes"]=files.get("out7z\\notes.txt")==sample
        if "--extended" in sys.argv:
            original=bytes((i*17+j*31+(j>>8))&255 for i in range(48) for j in range(4096))
            results["large persisted source"]=files.get("batch\\large.bin")==original
            expected=b"Hi5OS updated content\r\n"
            try:
                with zipfile.ZipFile(io.BytesIO(files["multi.zip"])) as z:
                    results["updated ZIP deletion"]=z.testzip() is None and z.read("batch/small.txt")==expected and "batch/large.bin" not in z.namelist()
            except (KeyError,zipfile.BadZipFile):results["updated ZIP deletion"]=False
            try:
                arc=tmp/"multi.7z";arc.write_bytes(files["multi.7z"])
                p=subprocess.run(["7z","x","-so",str(arc),"batch/small.txt"],capture_output=True,timeout=30)
                listing=subprocess.run(["7z","l","-slt",str(arc)],capture_output=True,timeout=30)
                results["updated 7z deletion"]=p.returncode==0 and p.stdout==expected and listing.returncode==0 and b"Path = batch/large.bin" not in listing.stdout and b"Path = batch\\large.bin" not in listing.stdout
            except (KeyError,subprocess.TimeoutExpired):results["updated 7z deletion"]=False
        (ROOT/"dist"/("7zip-archive"+suffix+"-results.json")).write_text(json.dumps(results,indent=2)+"\n")
        print("Archive qualification:",json.dumps(results))
        if not all(results.values()):print("::warning::Archive functionality NOT qualified; see operation results and actual API trace")
        # Strict release mode is separate from the diagnostic candidate.
        if "--require" in sys.argv:assert all(results.values()),results
        if "--require-live" in sys.argv:assert results.get("original live window") and results.get("original live persistence"),"Original 7-Zip did not reach and maintain visible message wait"
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:proc.wait(timeout=5)
            except subprocess.TimeoutExpired:proc.kill();proc.wait()
        if log.exists():print(log.read_text(errors="replace"))
