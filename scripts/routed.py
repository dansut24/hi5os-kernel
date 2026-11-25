import pathlib,tempfile,shutil,subprocess,time,socket,json,threading,struct
from smoke import ROOT,firmware,validate
MAC=bytes.fromhex("525400123456");GW=bytes.fromhex("525400aabb01");DM=bytes.fromhex("525400aabb03")
IP=bytes([10,77,0,19]);G=bytes([10,77,0,1]);D=bytes([10,77,0,3]);R=bytes([192,0,2,77])
def ck(b):
    if len(b)%2:b+=b"\0"
    v=sum(struct.unpack("!%dH"%(len(b)//2),b))
    while v>>16:v=(v&65535)+(v>>16)
    return (~v)&65535
def frame(mac,src,dst,proto,payload):
    h=struct.pack("!BBHHHBBH4s4s",0x45,0,20+len(payload),1,0,64,proto,0,src,dst)
    return MAC+mac+b"\x08\x00"+h[:10]+struct.pack("!H",ck(h))+h[12:]+payload
def udp(mac,src,dst,sp,dp,b):return frame(mac,src,dst,17,struct.pack("!HHHH",sp,dp,8+len(b),0)+b)
def tcp(sp,dp,seq,ack,flags,b=b""):
    t=struct.pack("!HHIIBBHHH",sp,dp,seq,ack,0x50,flags,4096,0,0)+b
    c=ck(R+IP+struct.pack("!BBH",0,6,len(t))+t)
    return frame(GW,R,IP,6,t[:16]+struct.pack("!H",c)+t[18:])
def exact(s,n):
    b=b""
    while len(b)<n:
        x=s.recv(n-len(b))
        if not x:raise EOFError()
        b+=x
    return b
def peer(listener,seen,errors):
    try:
        s,_=listener.accept()
        with s:
            s.settimeout(60)
            def send(b):s.sendall(struct.pack("!I",len(b))+b)
            while True:
                n=struct.unpack("!I",exact(s,4))[0]
                if n>65536:raise ValueError("frame length")
                r=exact(s,n)
                if len(r)<42:continue
                if r[12:14]==b"\x08\x06":
                    target=r[38:42]
                    if target not in (G,D):continue
                    m=GW if target==G else DM
                    send(MAC+m+b"\x08\x06"+struct.pack("!HHBBH",1,0x800,6,4,2)+m+target+MAC+r[28:32]);seen.add("gateway-arp" if target==G else "dns-arp");continue
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
                        if kind==b"\x03":assert opts[50]==IP and opts[54]==G
                        a=bytearray(240);a[:3]=b"\x02\x01\x06";a[4:8]=b[4:8];a[16:20]=IP;a[28:34]=MAC;a[236:240]=b"\x63\x82\x53\x63"
                        a+=bytes([53,1,2 if kind==b"\x01" else 5,54,4])+G+bytes([1,4,255,255,255,0,3,4])+G+bytes([6,4])+D+bytes([51,4,0,0,14,16,255])
                        send(udp(GW,G,bytes([255]*4),67,68,a));seen.add("discover" if kind==b"\x01" else "request")
                    elif dp==53 and r[30:34]==D:
                        assert r[26:30]==IP and b[12:]==b"\x03lab\x04test\x00\x00\x01\x00\x01"
                        a=b[:2]+b"\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00"+b[12:]+b"\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04"+R
                        send(udp(DM,D,IP,53,sp,a));seen.add("dns")
                elif r[23]==6 and r[30:34]==R:
                    assert r[:6]==GW and r[26:30]==IP
                    sp,dp,seq,ack=struct.unpack_from("!HHII",r,off);assert dp==80
                    flags=r[off+13];b=r[off+(r[off+12]>>4)*4:14+struct.unpack_from("!H",r,16)[0]]
                    if flags&2:send(tcp(dp,sp,1000,seq+1,0x12));seen.add("routed-syn")
                    elif b:
                        assert b.startswith(b"GET / HTTP/1.1\r\nHost: lab.test\r\n")
                        body=b"Hi5 routed HTTP";a=b"HTTP/1.0 200 OK\r\nContent-Length: 15\r\n\r\n"+body
                        send(tcp(dp,sp,1001,seq+len(b),0x18,a));seen.add("routed-http")
    except EOFError:pass
    except Exception as e:errors.append(repr(e))
def main():
    code,var=firmware()
    with socket.socket() as listener,tempfile.TemporaryDirectory(prefix="hi5-route-") as td:
        listener.bind(("127.0.0.1",0));listener.listen(1);listener.settimeout(40)
        seen=set();errors=[];threading.Thread(target=peer,args=(listener,seen,errors),daemon=True).start()
        t=pathlib.Path(td);vars=t/"vars.fd";shutil.copyfile(var,vars);qmp=t/"qmp.sock";log=ROOT/"dist"/"routed-network.log";log.unlink(missing_ok=True)
        cmd=["qemu-system-x86_64","-machine","q35,accel=tcg","-cpu","qemu64","-m","512","-smp","1","-drive",f"if=pflash,format=raw,readonly=on,file={code}","-drive",f"if=pflash,format=raw,file={vars}","-drive",f"if=none,id=os,format=raw,readonly=on,file={ROOT/'dist/hi5os-v0.2.5.img'}","-device","qemu-xhci","-device","usb-storage,drive=os,bootindex=1","-netdev",f"socket,id=n0,connect=127.0.0.1:{listener.getsockname()[1]}","-device","e1000,netdev=n0,mac=52:54:00:12:34:56","-vga","std","-monitor","none","-no-reboot","-serial",f"file:{log}","-display","none","-qmp",f"unix:{qmp},server=on,wait=off"]
        proc=subprocess.Popen(cmd)
        def wait(marker,seconds=30):
            end=time.monotonic()+seconds
            while time.monotonic()<end:
                b=log.read_bytes() if log.exists() else b""
                if marker in b:return b
                if errors or proc.poll() is not None or b"H5K: FAIL " in b or b"H5K: PANIC" in b:raise RuntimeError(f"{errors} {b!r}")
                time.sleep(.1)
            raise RuntimeError(f"Missing {marker!r}: {b!r}; peer={seen} errors={errors}")
        try:
            validate(wait(b"H5K: KERNEL_READY",60))
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
                key("6");wait(b"GUI_BROWSER_OPEN",5);key("ctrl-l")
                for ch in "http://lab.test/":key({":":"shift-semicolon","/":"slash",".":"dot"}.get(ch,ch))
                key("ret");wait(b"DHCP_ACK_OK",20);wait(b"DNS_A_OK",10);wait(b"HTTP_DOWNLOAD_OK",10);wait(b"HTTP_DOWNLOAD_BYTES=0x000000000000000f",3)
                assert {"discover","request","dns-arp","dns","gateway-arp","routed-syn","routed-http"}<=seen
                call("screendump",{"filename":str(ROOT/"dist"/"routed-browser.ppm")});call("quit")
            proc.wait(timeout=10);print("PASS: DHCP DNS and gateway-routed HTTP on alternate subnet")
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
            if log.exists():print(log.read_text(errors="replace"))
if __name__=="__main__":main()
