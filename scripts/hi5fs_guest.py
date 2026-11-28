#!/usr/bin/env python3
"""Cold-boot AHCI qualification; independent byte-level checks and legacy preservation."""
import pathlib, tempfile, shutil, subprocess, time, socket, json, zlib, struct
from smoke import ROOT, firmware, validate
FIRST, END, DATA = 6466, 8192, 6500
def crc(b): return zlib.crc32(b)&0xffffffff
def u32(b,i): return struct.unpack_from("<I",b,i)[0]
def bank(raw,base):
    h=raw[base*512:(base+1)*512]
    if h[:8]!=b"H5FS0001" or crc(h[:508])!=u32(h,508):return None
    assert u32(h,8)==1 and u32(h,28)==DATA and u32(h,32)==END
    b=raw[(base+1)*512:(base+17)*512]
    if crc(b)!=u32(h,24):return None
    entries={}
    for i in range(u32(h,20)):
        e=b[i*128:(i+1)*128];id,parent=u32(e,0),u32(e,4)
        name=e[40:40+e[10]].decode("ascii")
        n,start=u32(e,12),u32(e,16);payload=raw[(DATA+start)*512:(DATA+start)*512+n]
        assert crc(payload)==u32(e,20)
        assert id not in entries
        entries[id]=(parent,name,e[8],payload)
    paths={}
    for id,(parent,name,kind,payload) in entries.items():
        names=[name];seen={id}
        while parent:
            assert parent not in seen;seen.add(parent)
            parent,n,_,_=entries[parent];names.append(n)
        paths["/"+"/".join(n for n in reversed(names) if n)]=(kind,payload)
    return u32(h,12),paths
def seed(data):
    raw=bytearray(data.read_bytes())
    note=bytearray(512);note[:8]=b"H5NOTE01";struct.pack_into("<III",note,8,1,1,11);note[20:31]=b"legacy note";struct.pack_into("<I",note,508,crc(note[:508]));raw[512:1024]=note
    word=b"H5DOC001"+struct.pack("<I",6)+bytes(4)+b"".join(bytes([c,0]) for c in b"legacy");word+=struct.pack("<I",crc(word))
    head=bytearray(512);head[:8]=b"H5OFF001";struct.pack_into("<III",head,8,1,len(word),crc(word));struct.pack_into("<I",head,508,crc(head[:508]))
    raw[4162*512:4163*512]=head;raw[4163*512:4163*512+len(word)]=word
    data.write_bytes(raw);return bytes(raw)
def main():
    code,var=firmware()
    with tempfile.TemporaryDirectory(prefix="hi5fs-guest-") as td:
        tmp=pathlib.Path(td);data=tmp/"data.img"
        shutil.copyfile(ROOT/"dist/hi5os-data-v1.img",data);original=seed(data)
        for boot in (1,2,3):
            vars=tmp/f"vars{boot}.fd";shutil.copyfile(var,vars)
            log=ROOT/"dist"/f"hi5fs-{boot}.log";log.unlink(missing_ok=True)
            qmp=tmp/f"qmp{boot}.sock"
            cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","qemu64","-m","512","-smp","1",
                "-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}",
                "-drive",f"if=none,id=os,format=raw,readonly=on,file={ROOT/'dist/hi5os-v0.3.4.img'}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1",
                "-drive",f"if=none,id=data,format=raw,file={data}","-device","ide-hd,bus=ide.1,drive=data,bootindex=2",
                "-vga","std","-nic","none","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
            proc=subprocess.Popen(cmd)
            def wait(marker,seconds=90,prior=0):
                end=time.monotonic()+seconds
                while time.monotonic()<end:
                    b=log.read_bytes() if log.exists() else b""
                    if marker in b[prior:]:return b
                    if proc.poll() is not None or b"H5K: FAIL " in b or b"H5K: PANIC" in b:raise RuntimeError(f"Guest failed: {b!r}")
                    time.sleep(.1)
                raise RuntimeError(f"Missing {marker!r}: {b!r}")
            try:
                validate(wait(b"H5K: KERNEL_READY"))
                with socket.socket(socket.AF_UNIX) as sock:
                    sock.settimeout(15);sock.connect(str(qmp));stream=sock.makefile("rwb");json.loads(stream.readline())
                    def call(name,args=None):
                        stream.write((json.dumps({"execute":name,**({"arguments":args} if args else {})})+"\n").encode());stream.flush()
                        while True:
                            r=json.loads(stream.readline())
                            if "error" in r:raise RuntimeError(r["error"])
                            if "return" in r:return r["return"]
                    call("qmp_capabilities")
                    def key(k):call("human-monitor-command",{"command-line":"sendkey "+k});time.sleep(.23)
                    def typed(t):
                        for c in t:key({" ":"spc",".":"dot","/":"slash",";":"semicolon","-":"minus"}.get(c,c))
                    def command(t,marker=None):
                        prior=len(log.read_bytes());typed(t);key("ret")
                        if marker:wait(marker,90,prior)
                    key("f2");wait(b"GUI_FILES_OPEN");key("ctrl-h");wait(b"HI5FS_EXPLORER")
                    if boot==1:
                        command("init",b"HI5FS_MOUNTED")
                        command("cd documents",b"HI5FS_DIRECTORY_OPEN")
                        command("mkdir work",b"HI5FS_TRANSACTION_OK")
                        command("cd work",b"HI5FS_DIRECTORY_OPEN")
                        command("write test.txt; hello world",b"HI5FS_TRANSACTION_OK")
                        command("copy test.txt; clone.txt",b"HI5FS_TRANSACTION_OK")
                        command("rename clone.txt; renamed.txt",b"HI5FS_TRANSACTION_OK")
                        command("move renamed.txt; ../moved.txt",b"HI5FS_TRANSACTION_OK")
                        command("delete test.txt");command("confirm",b"HI5FS_TRANSACTION_OK")
                        command("new draft.h5doc",b"HI5FS_TRANSACTION_OK")
                        command("open draft.h5doc",b"HI5FS_WORD_OPEN")
                        typed("hello");prior=len(log.read_bytes());key("ctrl-s");wait(b"HI5FS_WORD_SAVED",90,prior)
                        prior=len(log.read_bytes());key("ctrl-e");wait(b"HI5FS_DOCX_SAVED",90,prior)
                        key("ctrl-shift-s");wait(b"HI5FS_EXPLORER",90,prior)
                        command("saveword copy.h5doc",b"HI5FS_WORD_SAVED")
                        command("import",b"HI5FS_LEGACY_IMPORTED")
                        command("mkdir verified",b"HI5FS_TRANSACTION_OK")
                        command("check",b"HI5FS_CHECK_OK")
                        call("screendump",{"filename":str(ROOT/"dist"/"hi5fs-explorer.ppm")})
                    else:
                        wait(b"HI5FS_RECOVERED" if boot==3 else b"HI5FS_MOUNTED")
                        command("cd documents/work",b"HI5FS_DIRECTORY_OPEN")
                        command("check",b"HI5FS_CHECK_OK")
                        command("open draft.h5doc",b"HI5FS_WORD_OPEN")
                        call("screendump",{"filename":str(ROOT/"dist"/f"hi5fs-word-{boot}.ppm")})
                    call("quit")
                proc.wait(timeout=10)
                raw=data.read_bytes()
                assert raw[:FIRST*512]==original[:FIRST*512] and raw[END*512:]==original[END*512:],"Legacy sectors changed"
                valid=[(base,v) for base in (FIRST,FIRST+17) if (v:=bank(raw,base)) is not None]
                base,(gen,paths)=max(valid,key=lambda v:v[1][0])
                root="/Users/Local/Documents"
                assert paths[root+"/moved.txt"]==(2,b"hello world")
                assert root+"/Work/test.txt" not in paths and root+"/work/test.txt" not in paths
                assert paths[root+"/work/draft.h5doc"][1][16:-4:2]==b"hello"
                import io,zipfile
                with zipfile.ZipFile(io.BytesIO(paths[root+"/work/draft.docx"][1])) as z:
                    assert z.testzip() is None and b"hello" in z.read("word/document.xml")
                assert paths[root+"/Legacy/notes.txt"]==(2,b"legacy note")
                assert paths[root+"/Legacy/Document 1.h5doc"][1][16:-4:2]==b"legacy"
                if boot==2:
                    with data.open("r+b") as fh:fh.seek(base*512);v=fh.read(1);fh.seek(base*512);fh.write(bytes([v[0]^1]))
            finally:
                if proc.poll() is None:proc.terminate();proc.wait(timeout=10)
                if log.exists():print(log.read_text(errors="replace"))
    print("PASS: Hi5FS AHCI native operations, Word/DOCX, legacy migration, cold mount, corrupt-generation fallback and untouched legacy sectors")
if __name__=="__main__":main()
