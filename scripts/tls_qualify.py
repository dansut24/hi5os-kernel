#!/usr/bin/env python3
"""Actual guest TLS over a deterministic virtual Ethernet peer. Test CA never shipped."""
import pathlib,tempfile,shutil,subprocess,time,socket,json,threading,struct,ssl,os
from routed import MAC,GW,DM,IP,G,D,R,udp,tcp,exact
from smoke import ROOT,firmware,validate

def peer(listener,seen,errors,context,host,success):
    try:
        s,_=listener.accept()
        with s:
            s.settimeout(120)
            def send(b):s.sendall(struct.pack("!I",len(b))+b)
            cin=ssl.MemoryBIO();cout=ssl.MemoryBIO();tls=context.wrap_bio(cin,cout,server_side=True)
            server_seq=1001;client_seq=None;established=False;replied=False;req=b""
            def drain(dp,sp):
                nonlocal server_seq
                while cout.pending:
                    b=cout.read(1000);send(tcp(dp,sp,server_seq,client_seq,0x18,b));server_seq+=len(b)
            while True:
                n=struct.unpack("!I",exact(s,4))[0]
                if n>65536:raise ValueError("frame length")
                r=exact(s,n)
                if len(r)<42:continue
                if r[12:14]==b"\x08\x06":
                    target=r[38:42]
                    if target not in (G,D):continue
                    m=GW if target==G else DM
                    send(MAC+m+b"\x08\x06"+struct.pack("!HHBBH",1,0x800,6,4,2)+m+target+MAC+r[28:32]);continue
                if r[12:14]!=b"\x08\x00":continue
                off=14+(r[14]&15)*4
                if r[23]==17:
                    sp,dp,ln,_=struct.unpack_from("!HHHH",r,off);b=r[off+8:off+ln]
                    if (sp,dp)==(68,67) and len(b)>=240:
                        i=240;opts={}
                        while i<len(b):
                            k=b[i];i+=1
                            if k==255:break
                            if k==0:continue
                            l=b[i];i+=1;opts[k]=b[i:i+l];i+=l
                        kind=opts.get(53)
                        if kind not in (b"\x01",b"\x03"):continue
                        a=bytearray(240);a[:3]=b"\x02\x01\x06";a[4:8]=b[4:8];a[16:20]=IP;a[28:34]=MAC;a[236:240]=b"\x63\x82\x53\x63"
                        a+=bytes([53,1,2 if kind==b"\x01" else 5,54,4])+G+bytes([1,4,255,255,255,0,3,4])+G+bytes([6,4])+D+bytes([51,4,0,0,14,16,255])
                        send(udp(GW,G,bytes([255]*4),67,68,a))
                    elif dp==53 and r[30:34]==D:
                        question=b"".join(bytes([len(x)])+x.encode() for x in host.split("."))+b"\x00\x00\x01\x00\x01"
                        assert b[12:]==question,(b[12:],question)
                        a=b[:2]+b"\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00"+b[12:]+b"\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04"+R
                        send(udp(DM,D,IP,53,sp,a));seen.add("dns")
                elif r[23]==6 and r[30:34]==R:
                    sp,dp,seq,ack=struct.unpack_from("!HHII",r,off);assert dp==443
                    flags=r[off+13];b=r[off+(r[off+12]>>4)*4:14+struct.unpack_from("!H",r,16)[0]]
                    if flags&4:return
                    if flags&2:
                        client_seq=seq+1;send(tcp(dp,sp,1000,client_seq,0x12));seen.add("syn");continue
                    if flags&1:
                        client_seq=seq+1;send(tcp(dp,sp,server_seq,client_seq,0x11));return
                    if not b:continue
                    assert seq==client_seq,(seq,client_seq);client_seq+=len(b);send(tcp(dp,sp,server_seq,client_seq,0x10))
                    cin.write(b)
                    try:
                        if not established:
                            tls.do_handshake();established=True;seen.add("handshake")
                        if not replied:
                            req+=tls.read(4096)
                            if b"\r\n\r\n" in req:
                                assert req.startswith(b"GET / HTTP/1.1\r\nHost: "+host.encode()+b"\r\n");seen.add("http")
                                body=b"<html><body>Native HTTPS verified</body></html>"
                                tls.write(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: "+str(len(body)).encode()+b"\r\n\r\n"+body);replied=True
                                try:tls.unwrap()
                                except ssl.SSLWantReadError:pass
                        elif replied:
                            try:tls.unwrap()
                            except ssl.SSLWantReadError:pass
                    except ssl.SSLWantReadError:pass
                    except ssl.SSLError:
                        seen.add("tls-alert")
                        if success:raise
                    drain(dp,sp)
    except (EOFError,ConnectionResetError,BrokenPipeError):pass
    except Exception as e:errors.append(repr(e))

def qualify(image,context,host,success,tmp):
    code,var=firmware();tag=host.replace(".","-")+("-trusted" if success else "-reject");vars=tmp/f"{tag}.fd";shutil.copyfile(var,vars)
    qmp=tmp/f"{tag}.sock";log=ROOT/"dist"/f"tls-{tag}.log";log.unlink(missing_ok=True)
    with socket.socket() as listener:
        listener.bind(("127.0.0.1",0));listener.listen(1);listener.settimeout(60)
        seen=set();errors=[];threading.Thread(target=peer,args=(listener,seen,errors,context,host,success),daemon=True).start()
        cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","max,+rdrand","-m","512","-smp","1","-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}","-drive",f"if=none,id=os,format=raw,readonly=on,file={image}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1","-netdev",f"socket,id=n0,connect=127.0.0.1:{listener.getsockname()[1]}","-device","e1000,netdev=n0,mac=52:54:00:12:34:56","-vga","std","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
        proc=subprocess.Popen(cmd)
        def wait(mark,seconds=60):
            end=time.monotonic()+seconds
            while time.monotonic()<end:
                b=log.read_bytes() if log.exists() else b""
                if errors or proc.poll() is not None or b"H5K: FAIL " in b or b"H5K: PANIC" in b:raise RuntimeError(f"{errors} {b!r}")
                if mark in b:return b
                time.sleep(.1)
            raise RuntimeError(f"Missing {mark!r}; peer={seen} errors={errors}: {b!r}")
        try:
            validate(wait(b"H5K: KERNEL_READY",90))
            with socket.socket(socket.AF_UNIX) as s:
                s.settimeout(5);s.connect(str(qmp));f=s.makefile("rwb");json.loads(f.readline())
                def call(name,args=None):
                    f.write((json.dumps({"execute":name,**({"arguments":args} if args else {})})+"\n").encode());f.flush()
                    while True:
                        v=json.loads(f.readline())
                        if "error" in v:raise RuntimeError(v["error"])
                        if "return" in v:return v["return"]
                call("qmp_capabilities")
                def key(k):call("human-monitor-command",{"command-line":"sendkey "+k});time.sleep(.23)
                key("6");wait(b"GUI_BROWSER_OPEN",5);key("ctrl-l")
                for ch in f"https://{host}/":key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                key("ret")
                if success:
                    wait(b"TLS_VERIFIED",40);wait(b"BROWSER_PAGE_RENDERED",30);assert "http" in seen
                else:
                    wait(b"HTTP_DOWNLOAD_FAILED",40);b=log.read_bytes();assert b"TLS_ERROR=" in b and b"TLS_VERIFIED" not in b and "http" not in seen
                call("screendump",{"filename":str(ROOT/"dist"/f"tls-{tag}.ppm")});call("quit")
            proc.wait(timeout=10)
            print(f"PASS: {host} expected {'trusted HTTPS' if success else 'certificate rejection'}")
        finally:
            if proc.poll() is None:proc.terminate();proc.wait(timeout=10)
            if log.exists():print(log.read_text(errors="replace"))

def main():
    with tempfile.TemporaryDirectory(prefix="hi5-tls-fixture-") as td:
        t=pathlib.Path(td);ca=t/"ca.pem";key=t/"ca.key";cert=t/"server.pem";serverkey=t/"server.key";csr=t/"server.csr";ext=t/"ext"
        def run(*a):subprocess.run(a,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        run("openssl","req","-x509","-newkey","rsa:2048","-nodes","-keyout",str(key),"-out",str(ca),"-days","2","-subj","/CN=Hi5OS CI test only","-addext","basicConstraints=critical,CA:TRUE","-addext","keyUsage=critical,keyCertSign,cRLSign")
        run("openssl","req","-new","-newkey","rsa:2048","-nodes","-keyout",str(serverkey),"-out",str(csr),"-subj","/CN=tls.test")
        ext.write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:tls.test\n")
        run("openssl","x509","-req","-in",str(csr),"-CA",str(ca),"-CAkey",str(key),"-CAcreateserial","-out",str(cert),"-days","2","-extfile",str(ext))
        subprocess.run(["/usr/bin/python3",str(ROOT/"scripts/prepare_tls.py"),str(ca)],check=True)
        env=os.environ.copy();env["RUSTFLAGS"]="-Lnative="+str(ROOT/"target/hi5-tls/test");env["CARGO_TARGET_DIR"]=str(t/"cargo")
        subprocess.run(["cargo","build","--release","--locked","--target","x86_64-unknown-uefi"],cwd=ROOT,env=env,check=True)
        image=t/"test.img";shutil.copyfile(ROOT/"dist/hi5os-v0.3.4.img",image)
        subprocess.run(["mcopy","-o","-i",str(image),str(t/"cargo/x86_64-unknown-uefi/release/hi5kernel.efi"),"::/EFI/BOOT/BOOTX64.EFI"],check=True)
        ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);ctx.minimum_version=ctx.maximum_version=ssl.TLSVersion.TLSv1_2;ctx.load_cert_chain(cert,serverkey)
        qualify(image,ctx,"tls.test",True,t);qualify(image,ctx,"wrong.test",False,t)
        # Same server with shipping Mozilla-root image: private test CA must fail.
        qualify(ROOT/"dist/hi5os-v0.3.4.img",ctx,"tls.test",False,t)
if __name__=="__main__":main()
