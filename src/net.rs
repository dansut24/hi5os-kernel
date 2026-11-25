//! Hi5Net: experimental polled E1000 and restricted single-connection HTTP/1.0.
//! DHCP, DNS A lookup and IPv4 routing. Bounded HTTP preview, no TLS.
use crate::{platform as p,serial};
use core::{ptr,sync::atomic::{fence,Ordering}};
const N:usize=16;
const B:usize=2048;
const ZERO:[u8;4]=[0;4];
pub const MAX_BODY:usize=4*1024*1024;
pub const MAX_RESPONSE:usize=MAX_BODY+128*1024;
#[repr(C,align(128))]struct Descs([u8;N*16]);
#[repr(C,align(4096))]struct Buffers([u8;N*B]);
static mut RX:Descs=Descs([0;N*16]);
static mut TX:Descs=Descs([0;N*16]);
static mut RB:Buffers=Buffers([0;N*B]);
static mut TB:Buffers=Buffers([0;N*B]);
fn rd(a:usize)->u32{unsafe{ptr::read_volatile(a as *const u32)}}
fn wr(a:usize,v:u32){unsafe{ptr::write_volatile(a as *mut u32,v)}}
fn pci(b:u32,o:u32)->u32{p::out32(0xcf8,b|o);p::input32(0xcfc)}
fn pci_wr(b:u32,o:u32,v:u32){p::out32(0xcf8,b|o);p::out32(0xcfc,v);}
fn be16(b:&[u8],i:usize)->u16{u16::from_be_bytes([b[i],b[i+1]])}
fn be32(b:&[u8],i:usize)->u32{u32::from_be_bytes([b[i],b[i+1],b[i+2],b[i+3]])}
fn put16(b:&mut[u8],i:usize,v:u16){b[i..i+2].copy_from_slice(&v.to_be_bytes());}
fn put32(b:&mut[u8],i:usize,v:u32){b[i..i+4].copy_from_slice(&v.to_be_bytes());}
fn sum(b:&[u8])->u32{let mut s=0;let mut i=0;while i+1<b.len(){s+=be16(b,i)as u32;i+=2;}if i<b.len(){s+=(b[i]as u32)<<8;}s}
fn fold(mut s:u32)->u16{while s>>16!=0{s=(s&65535)+(s>>16);}!(s as u16)}
fn tcp_sum(src:[u8;4],dst:[u8;4],b:&[u8])->u16{fold(sum(&src)+sum(&dst)+6+b.len()as u32+sum(b))}
fn before(deadline:u64)->bool{p::ticks()<deadline}
pub struct Nic{base:usize,mac:[u8;6],rx:usize,tx:usize,seq:u32,port:u16,ip:[u8;4],mask:[u8;4],gateway:[u8;4],dns:[u8;4],until:u64}
impl Nic{
    pub fn open()->Option<Self>{
        for d in 0..32u32{for f in 0..8u32{
            let b=0x80000000|(d<<11)|(f<<8);
            if pci(b,0)!=0x100e8086{continue;}
            let bar=pci(b,0x10);let base=(bar&!15)as usize;
            if bar&7!=0||base<0x8000_0000||base>0xfffe_0000{continue;}
            let command=pci(b,4);pci_wr(b,4,(command&0xffff)|6);
            wr(base+0xd8,u32::MAX);let _=rd(base+0xc0);
            wr(base,rd(base)|(1<<26));let mut reset=false;
            for _ in 0..2_000_000{if rd(base)&(1<<26)==0{reset=true;break;}}
            if !reset{return None;}
            wr(base+0xd8,u32::MAX);let _=rd(base+0xc0);
            let mut mac=[0;6];
            for i in 0..3{wr(base+0x14,1|((i as u32)<<8));let mut done=false;for _ in 0..100000{let v=rd(base+0x14);if v&16!=0{mac[i*2]=(v>>16)as u8;mac[i*2+1]=(v>>24)as u8;done=true;break;}}if !done{return None;}}
            if mac==[0;6]||mac==[255;6]||mac[0]&1!=0{return None;}
            wr(base+0x5400,u32::from_le_bytes([mac[0],mac[1],mac[2],mac[3]]));
            wr(base+0x5404,u16::from_le_bytes([mac[4],mac[5]])as u32|(1<<31));
            unsafe{
                let rx=ptr::addr_of_mut!(RX.0).cast::<u8>();let tx=ptr::addr_of_mut!(TX.0).cast::<u8>();
                let rb=ptr::addr_of_mut!(RB.0).cast::<u8>();let tb=ptr::addr_of_mut!(TB.0).cast::<u8>();
                if [rx,tx,rb,tb].iter().any(|v|*v as usize>0xffff_0000){return None;}
                ptr::write_bytes(rx,0,N*16);ptr::write_bytes(tx,0,N*16);
                for i in 0..N{ptr::write_unaligned(rx.add(i*16).cast::<u64>(),rb.add(i*B)as u64);ptr::write_unaligned(tx.add(i*16).cast::<u64>(),tb.add(i*B)as u64);tx.add(i*16+12).write(1);}
                fence(Ordering::SeqCst);
                wr(base+0x2800,rx as u32);wr(base+0x2804,0);wr(base+0x2808,(N*16)as u32);wr(base+0x2810,0);wr(base+0x2818,(N-1)as u32);
                wr(base+0x3800,tx as u32);wr(base+0x3804,0);wr(base+0x3808,(N*16)as u32);wr(base+0x3810,0);wr(base+0x3818,0);
            }
            wr(base+0x400,0);for i in 0..128{wr(base+0x5200+i*4,0);}
            wr(base,rd(base)|(1<<6)); // set link up
            wr(base+0x410,0x0060200a);
            wr(base+0x400,2|8|(0x10<<4)|(0x40<<12)); // TCTL
            wr(base+0x100,2|(1<<15)|(1<<26)); // RCTL: 2048 buffers, broadcast, strip CRC
            serial::line("H5K: E1000_READY");return Some(Self{base,mac,rx:0,tx:0,seq:0x48350000,port:49152,ip:ZERO,mask:ZERO,gateway:ZERO,dns:ZERO,until:0});
        }}None
    }
    fn send(&mut self,b:&[u8])->bool{
        if b.len()>1514{return false;}
        unsafe{
            let d=ptr::addr_of_mut!(TX.0).cast::<u8>().add(self.tx*16);
            let t=ptr::addr_of_mut!(TB.0).cast::<u8>().add(self.tx*B);
            let end=p::ticks()+100;
            while ptr::read_volatile(d.add(12))&1==0{if !before(end){return false;}p::wait_tick();}
            ptr::copy_nonoverlapping(b.as_ptr(),t,b.len());let len=b.len().max(60);if len>b.len(){ptr::write_bytes(t.add(b.len()),0,len-b.len());}
            ptr::write_unaligned(d.add(8).cast::<u16>(),len as u16);d.add(10).write(0);d.add(11).write(0x0b);ptr::write_volatile(d.add(12),0);
            fence(Ordering::SeqCst);self.tx=(self.tx+1)%N;wr(self.base+0x3818,self.tx as u32);
        }true
    }
    fn recv(&mut self,b:&mut[u8;B])->usize{
        unsafe{
            let d=ptr::addr_of_mut!(RX.0).cast::<u8>().add(self.rx*16);
            let status=ptr::read_volatile(d.add(12));if status&1==0{return 0;}
            fence(Ordering::SeqCst);
            let len=ptr::read_unaligned(d.add(8).cast::<u16>())as usize;
            let valid=status&2!=0&&ptr::read_volatile(d.add(13))==0&&len<=B&&len>=14;
            if valid{ptr::copy_nonoverlapping(ptr::addr_of!(RB.0).cast::<u8>().add(self.rx*B),b.as_mut_ptr(),len);}
            ptr::write_volatile(d.add(12),0);fence(Ordering::SeqCst);wr(self.base+0x2818,self.rx as u32);self.rx=(self.rx+1)%N;
            if valid{len}else{0}
        }
    }
    fn arp(&mut self,ip:[u8;4])->Option<[u8;6]>{
        let mut b=[0u8;42];b[..6].fill(255);b[6..12].copy_from_slice(&self.mac);put16(&mut b,12,0x806);
        put16(&mut b,14,1);put16(&mut b,16,0x800);b[18]=6;b[19]=4;put16(&mut b,20,1);b[22..28].copy_from_slice(&self.mac);b[28..32].copy_from_slice(&self.ip);b[38..42].copy_from_slice(&ip);
        let mut r=[0;B];
        for _ in 0..3{if !self.send(&b){return None;}let end=p::ticks()+100;
            while before(end){let n=self.recv(&mut r);if n>=42&&be16(&r,12)==0x806&&be16(&r,14)==1&&be16(&r,16)==0x800&&r[18]==6&&r[19]==4&&be16(&r,20)==2&&r[28..32]==ip&&r[38..42]==self.ip&&r[32..38]==self.mac&&r[6..12]==r[22..28]{let mut m=[0;6];m.copy_from_slice(&r[22..28]);if m[0]&1==0&&m!=[0;6]{return Some(m);}}if n==0{p::wait_tick();}}
        }None
    }
    fn answer_arp(&mut self,r:&[u8]){
        if r.len()<42||be16(r,12)!=0x806||be16(r,14)!=1||be16(r,16)!=0x800||r[18]!=6||r[19]!=4||be16(r,20)!=1||r[38..42]!=self.ip||r[6..12]!=r[22..28]{return;}
        let mut a=[0u8;42];a[..6].copy_from_slice(&r[22..28]);a[6..12].copy_from_slice(&self.mac);put16(&mut a,12,0x806);put16(&mut a,14,1);put16(&mut a,16,0x800);a[18]=6;a[19]=4;put16(&mut a,20,2);a[22..28].copy_from_slice(&self.mac);a[28..32].copy_from_slice(&self.ip);a[32..42].copy_from_slice(&r[22..32]);let _=self.send(&a);
    }
    fn packet(&mut self,mac:[u8;6],ip:[u8;4],port:u16,seq:u32,ack:u32,flags:u8,data:&[u8])->bool{
        if data.len()>1000{return false;}let mut b=[0u8;1054];let n=54+data.len();b[..6].copy_from_slice(&mac);b[6..12].copy_from_slice(&self.mac);put16(&mut b,12,0x800);
        b[14]=0x45;put16(&mut b,16,(40+data.len())as u16);put16(&mut b,20,0x4000);b[22]=64;b[23]=6;b[26..30].copy_from_slice(&self.ip);b[30..34].copy_from_slice(&ip);let cs=fold(sum(&b[14..34]));put16(&mut b,24,cs);
        put16(&mut b,34,self.port);put16(&mut b,36,port);put32(&mut b,38,seq);put32(&mut b,42,ack);b[46]=0x50;b[47]=flags;put16(&mut b,48,4096);b[54..n].copy_from_slice(data);
        let cs=tcp_sum(self.ip,ip,&b[34..n]);put16(&mut b,50,cs);self.send(&b[..n])
    }
    fn segment<'a>(&mut self,r:&'a[u8],ip:[u8;4],port:u16)->Option<(u32,u32,u8,&'a[u8])>{
        if r.len()<54||r[..6]!=self.mac||be16(r,12)!=0x800||r[14]>>4!=4||r[23]!=6||r[26..30]!=ip||r[30..34]!=self.ip||be16(r,20)&0x3fff!=0{return None;}
        let ih=(r[14]&15)as usize*4;let total=be16(r,16)as usize;
        if ih<20||total<ih+20||14+total>r.len()||fold(sum(&r[14..14+ih]))!=0{return None;}
        let t=&r[14+ih..14+total];let h=(t[12]>>4)as usize*4;
        if h<20||h>t.len()||be16(t,0)!=port||be16(t,2)!=self.port||tcp_sum(ip,self.ip,t)!=0{return None;}
        Some((be32(t,4),be32(t,8),t[13],&t[h..]))
    }
    pub fn config(&self)->([u8;4],[u8;4],[u8;4]){(self.ip,self.gateway,self.dns)}
    fn udp_send(&mut self,mac:[u8;6],src:[u8;4],dst:[u8;4],sp:u16,dp:u16,data:&[u8])->bool{
        if data.len()>1200{return false;}let mut b=[0;1242];let n=42+data.len();
        b[..6].copy_from_slice(&mac);b[6..12].copy_from_slice(&self.mac);put16(&mut b,12,0x800);b[14]=0x45;
        put16(&mut b,16,(28+data.len())as u16);b[22]=64;b[23]=17;b[26..30].copy_from_slice(&src);b[30..34].copy_from_slice(&dst);
        let c=fold(sum(&b[14..34]));put16(&mut b,24,c);put16(&mut b,34,sp);put16(&mut b,36,dp);put16(&mut b,38,(8+data.len())as u16);b[42..n].copy_from_slice(data);
        let c=fold(sum(&src)+sum(&dst)+17+(8+data.len())as u32+sum(&b[34..n]));put16(&mut b,40,if c==0{65535}else{c});self.send(&b[..n])
    }
    fn dhcp_send(&mut self,xid:u32,offer:Option<Lease>)->bool{
        let mut b=[0;300];b[0]=1;b[1]=1;b[2]=6;put32(&mut b,4,xid);put16(&mut b,10,0x8000);b[28..34].copy_from_slice(&self.mac);b[236..240].copy_from_slice(&[99,130,83,99]);
        let mut i=240;b[i..i+3].copy_from_slice(&[53,1,if offer.is_some(){3}else{1}]);i+=3;
        if let Some(v)=offer{b[i..i+2].copy_from_slice(&[50,4]);b[i+2..i+6].copy_from_slice(&v.ip);i+=6;b[i..i+2].copy_from_slice(&[54,4]);b[i+2..i+6].copy_from_slice(&v.server);i+=6;}
        b[i..i+6].copy_from_slice(&[55,4,1,3,6,51]);i+=6;b[i]=255;self.udp_send([255;6],ZERO,[255;4],68,67,&b)
    }
    fn dhcp_wait(&mut self,xid:u32,kind:u8,offer:Option<Lease>)->Option<Lease>{
        let mut r=[0;B];let end=p::ticks()+250;while before(end){let n=self.recv(&mut r);if n==0{p::wait_tick();continue;}
            if r[..6]!=self.mac&&r[..6]!=[255;6]{continue;}
            if let Some((_,_,67,68,b))=udp(&r[..n]){if let Some(v)=lease(b,xid,self.mac,kind){if offer.map_or(true,|a|a.ip==v.ip&&a.server==v.server){return Some(v);}}}
        }None
    }
    pub fn configure(&mut self)->Result<(),&'static str>{
        if self.ip!=ZERO&&before(self.until){return Ok(());}
        self.ip=ZERO;self.mask=ZERO;self.gateway=ZERO;self.dns=ZERO;
        let xid=0x48354448^(p::ticks()as u32).rotate_left(9)^u32::from_be_bytes([self.mac[2],self.mac[3],self.mac[4],self.mac[5]]);
        let mut offer=None;for _ in 0..2{if !self.dhcp_send(xid,None){return Err("DHCP send failed");}if let Some(v)=self.dhcp_wait(xid,2,None){offer=Some(v);break;}}
        let o=offer.ok_or("DHCP offer timeout")?;serial::line("H5K: DHCP_OFFER_OK");
        for _ in 0..2{if !self.dhcp_send(xid,Some(o)){return Err("DHCP request failed");}if let Some(v)=self.dhcp_wait(xid,5,Some(o)){
            self.ip=v.ip;self.mask=v.mask;self.gateway=v.gateway;self.dns=v.dns;self.until=p::ticks()+u64::from(v.seconds.clamp(1,86400))*80;
            serial::metric("H5K: DHCP_IPV4=",u32::from_be_bytes(self.ip)as u64);serial::metric("H5K: DHCP_GATEWAY=",u32::from_be_bytes(self.gateway)as u64);serial::metric("H5K: DHCP_DNS=",u32::from_be_bytes(self.dns)as u64);serial::line("H5K: DHCP_ACK_OK");return Ok(());
        }}Err("DHCP acknowledgement timeout")
    }
    fn hop(&self,ip:[u8;4])->Option<[u8;4]>{
        let m=u32::from_be_bytes(self.mask);let a=u32::from_be_bytes(self.ip);let b=u32::from_be_bytes(ip);
        if m==0||b==(a&m)||b==((a&m)|!m){return None;}
        if a&m==b&m{Some(ip)}else if valid(self.gateway){Some(self.gateway)}else{None}
    }
    fn resolve(&mut self,host:&[u8])->Result<[u8;4],&'static str>{
        let dns=self.dns;if !valid(dns){return Err("No DHCP DNS server");}
        let hop=self.hop(dns).ok_or("No DNS route")?;let mac=self.arp(hop).ok_or("DNS ARP timeout")?;
        let id=(p::ticks()as u16).wrapping_add(self.port);let sp=53000+id%1000;
        let mut q=[0;272];put16(&mut q,0,id);put16(&mut q,2,0x100);put16(&mut q,4,1);let mut end=12;
        for label in host.split(|b|*b==b'.'){if label.is_empty()||label.len()>63||end+label.len()+6>q.len(){return Err("Invalid DNS name");}q[end]=label.len()as u8;end+=1;q[end..end+label.len()].copy_from_slice(label);end+=label.len();}
        end+=1;put16(&mut q,end,1);put16(&mut q,end+2,1);end+=4;
        let mut r=[0;B];for _ in 0..2{if !self.udp_send(mac,self.ip,dns,sp,53,&q[..end]){return Err("DNS send failed");}let until=p::ticks()+250;
            while before(until){let n=self.recv(&mut r);if n==0{p::wait_tick();continue;}self.answer_arp(&r[..n]);if r[..6]!=self.mac{continue;}
                if let Some((s,d,53,dp,b))=udp(&r[..n]){if s==dns&&d==self.ip&&dp==sp{if let Some(ip)=dns_answer(b,&q[..end]){serial::metric("H5K: DNS_IPV4=",u32::from_be_bytes(ip)as u64);serial::line("H5K: DNS_A_OK");return Ok(ip);}}}
            }
        }Err("DNS A lookup failed")
    }
    pub fn connect(&mut self,host:&[u8],port:u16)->Result<Tcp<'_>,&'static str>{
        self.configure()?;
        let ip=match numeric(host){Some(ip)=>ip,None=>self.resolve(host)?};
        if !valid(ip)||ip==self.ip{return Err("Unsupported destination");}
        let hop=self.hop(ip).ok_or("No route to destination")?;
        self.port=if self.port==65535{49152}else{self.port+1};self.seq=self.seq.wrapping_add(0x10001);
        let mac=self.arp(hop).ok_or("ARP timeout")?;serial::line("H5K: NET_ARP_OK");
        let mut rx=[0;B];let mut remote=0;let mut connected=false;
        for _ in 0..2{if !self.packet(mac,ip,port,self.seq,0,2,&[]){return Err("TX timeout");}
            let end=p::ticks()+200;
            while before(end){let n=self.recv(&mut rx);if n==0{p::wait_tick();continue;}self.answer_arp(&rx[..n]);if let Some((s,a,f,_))=self.segment(&rx[..n],ip,port){if f&4!=0&&f&16!=0&&a==self.seq.wrapping_add(1){return Err("TCP reset");}if f&0x12==0x12&&a==self.seq.wrapping_add(1){remote=s.wrapping_add(1);connected=true;break;}}}if connected{break;}
        }
        if !connected{return Err("TCP connect timeout");}
        self.seq=self.seq.wrapping_add(1);if !self.packet(mac,ip,port,self.seq,remote,0x10,&[]){return Err("ACK failed");}
        serial::line("H5K: NET_TCP_OK");
        Ok(Tcp{nic:self,mac,ip,port,remote,closed:false})
    }
    pub fn get(&mut self,url:&[u8],response:&mut[u8;MAX_RESPONSE])->Result<usize,&'static str>{
        let (host,port,path)=parse_url(url).ok_or("Use http://host:port/path")?;
        let mut req=[0u8;256];let mut k=0;
        for part in [b"GET ".as_slice(),path,b" HTTP/1.1\r\nHost: ".as_slice(),&url[7..url.iter().enumerate().skip(7).find(|(_,b)|**b==b'/').map(|(i,_)|i).unwrap_or(url.len())],b"\r\nConnection: close\r\n\r\n".as_slice()]{if k+part.len()>req.len(){return Err("URL too long");}req[k..k+part.len()].copy_from_slice(part);k+=part.len();}
        let mut tcp=self.connect(host,port)?;tcp.write_request(&req[..k])?;
        let overall=p::ticks()+24000;let mut used=0;let mut next_log=65536;
        let mut buf=[0u8;B];
        loop{
            let (n,fin)=tcp.read(&mut buf,overall)?;
            if used+n>response.len(){return Err("Response exceeds preview limit");}
            response[used..used+n].copy_from_slice(&buf[..n]);used+=n;
            if used>=next_log{serial::metric("H5K: HTTP_WIRE_PROGRESS=",used as u64);next_log+=65536;}
            if fin{serial::metric("H5K: HTTP_FIN_BYTES=",used as u64);}
            if let Some((len,mode))=crate::http::complete(response,used,fin)?{
                serial::line(match mode{0=>"H5K: HTTP_LENGTH_OK",1=>"H5K: HTTP_CLOSE_OK",_=>"H5K: HTTP_CHUNKED_OK"});
                serial::metric("H5K: HTTP_DOWNLOAD_BYTES=",len as u64);serial::line("H5K: HTTP_DOWNLOAD_OK");return Ok(len);
            }
            if fin{return Err("Incomplete HTTP body");}
        }
    }
}
/// Bounded, single-outstanding-write transport. Not yet a general TCP socket:
/// no congestion control, write retransmission, partial ACKs or reassembly.
pub struct Tcp<'a>{nic:&'a mut Nic,mac:[u8;6],ip:[u8;4],port:u16,remote:u32,closed:bool}
impl Tcp<'_>{
    pub fn write_request(&mut self,data:&[u8])->Result<(),&'static str>{
        if self.closed||data.is_empty()||data.len()>1000{return Err("TCP write bounds");}
        if !self.nic.packet(self.mac,self.ip,self.port,self.nic.seq,self.remote,0x18,data){return Err("Request failed");}
        self.nic.seq=self.nic.seq.wrapping_add(data.len()as u32);Ok(())
    }
    pub fn read(&mut self,out:&mut[u8;B],overall:u64)->Result<(usize,bool),&'static str>{
        if self.closed{return Ok((0,true));}
        let end=p::ticks()+1000;let mut rx=[0;B];
        while before(end)&&before(overall){
            let n=self.nic.recv(&mut rx);if n==0{p::wait_tick();continue;}self.nic.answer_arp(&rx[..n]);
            if let Some((s,a,f,data))=self.nic.segment(&rx[..n],self.ip,self.port){
                if f&0x10==0||a!=self.nic.seq{continue;}if f&4!=0{return Err("TCP reset");}
                if s!=self.remote{let _=self.nic.packet(self.mac,self.ip,self.port,self.nic.seq,self.remote,0x10,&[]);continue;}
                let fin=f&1!=0;if data.is_empty()&&!fin{continue;}
                out[..data.len()].copy_from_slice(data);self.remote=self.remote.wrapping_add(data.len()as u32);
                if fin{self.remote=self.remote.wrapping_add(1);}
                if !self.nic.packet(self.mac,self.ip,self.port,self.nic.seq,self.remote,0x10,&[]){return Err("TCP ACK failed");}
                return Ok((data.len(),fin));
            }
        }Err("TCP read timeout")
    }
}
impl Drop for Tcp<'_>{fn drop(&mut self){if !self.closed{let _=self.nic.packet(self.mac,self.ip,self.port,self.nic.seq,self.remote,0x14,&[]);self.closed=true;}}}

#[derive(Clone,Copy)]struct Lease{ip:[u8;4],mask:[u8;4],gateway:[u8;4],dns:[u8;4],server:[u8;4],seconds:u32}
fn valid(ip:[u8;4])->bool{ip[0]!=0&&ip[0]!=127&&ip[0]<224}
fn numeric(s:&[u8])->Option<[u8;4]>{let mut it=s.split(|b|*b==b'.');let mut ip=[0;4];for x in &mut ip{*x=decimal(it.next()?,255)? as u8;}if it.next().is_some(){return None;}Some(ip)}
fn parse_url(url:&[u8])->Option<(&[u8],u16,&[u8])>{
    let r=url.strip_prefix(b"http://")?;let i=r.iter().position(|b|*b==b'/').unwrap_or(r.len());let path=if i==r.len(){b"/".as_slice()}else{&r[i..]};
    if path.len()>120||!path.iter().all(|b|(33..127).contains(b)&&*b!=b'#'){return None;}
    let mut s=r[..i].split(|b|*b==b':');let h=s.next()?;let p=match s.next(){Some(v)=>decimal(v,65535)? as u16,None=>80};
    if p==0||s.next().is_some()||h.len()>253||!h.split(|b|*b==b'.').all(|l|!l.is_empty()&&l.len()<=63&&l[0]!=b'-'&&l[l.len()-1]!=b'-'&&l.iter().all(|b|b.is_ascii_alphanumeric()||*b==b'-')){return None;}
    if let Some(ip)=numeric(h){if !valid(ip){return None;}}Some((h,p,path))
}
fn udp(r:&[u8])->Option<([u8;4],[u8;4],u16,u16,&[u8])>{
    if r.len()<42||be16(r,12)!=0x800||r[14]>>4!=4||r[23]!=17||be16(r,20)&0x3fff!=0{return None;}
    let ih=(r[14]&15)as usize*4;let t=be16(r,16)as usize;if ih<20||t<ih+8||14+t>r.len()||fold(sum(&r[14..14+ih]))!=0{return None;}
    let u=&r[14+ih..14+t];let len=be16(u,4)as usize;if len!=u.len()||len<8{return None;}
    let s:[u8;4]=r[26..30].try_into().ok()?;let d:[u8;4]=r[30..34].try_into().ok()?;
    if be16(u,6)!=0&&fold(sum(&s)+sum(&d)+17+len as u32+sum(u))!=0{return None;}
    Some((s,d,be16(u,0),be16(u,2),&u[8..]))
}
fn lease(b:&[u8],xid:u32,mac:[u8;6],kind:u8)->Option<Lease>{
    if b.len()<240||b[..3]!=[2,1,6]||be32(b,4)!=xid||b[28..34]!=mac||b[236..240]!=[99,130,83,99]{return None;}
    let mut v=Lease{ip:b[16..20].try_into().ok()?,mask:ZERO,gateway:ZERO,dns:ZERO,server:ZERO,seconds:3600};
    let mut i=240;let mut msg=0;let mut end=false;
    while i<b.len(){let k=b[i];i+=1;if k==0{continue;}if k==255{end=true;break;}let n=*b.get(i)? as usize;i+=1;let s=b.get(i..i.checked_add(n)?)?;i+=n;
        match(k,n){(53,1)=>msg=s[0],(1,4)=>v.mask=s.try_into().ok()?,(3,n) if n>=4&&n%4==0=>v.gateway=s[..4].try_into().ok()?,(6,n) if n>=4&&n%4==0=>v.dns=s[..4].try_into().ok()?,(54,4)=>v.server=s.try_into().ok()?,(51,4)=>v.seconds=be32(s,0),_=>{}}
    }
    let m=u32::from_be_bytes(v.mask);let inv=!m;let ip=u32::from_be_bytes(v.ip);
    if !end||msg!=kind||!valid(v.ip)||!valid(v.server)||m==0||inv==0||inv&(inv.wrapping_add(1))!=0||ip&inv==0||ip&inv==inv{return None;}
    if v.gateway!=ZERO&&(!valid(v.gateway)||u32::from_be_bytes(v.gateway)&m!=ip&m){return None;}Some(v)
}
fn name(b:&[u8],start:usize,out:&mut[u8;253])->Option<(usize,usize)>{
    let mut p=start;let mut next=None;let mut n=0;let mut jumps=0;
    loop{let k=*b.get(p)?;p+=1;if k==0{return Some((n,next.unwrap_or(p)));}
        if k&0xc0==0xc0{let to=(((k&63)as usize)<<8)|*b.get(p)? as usize;if to>=p-1||jumps>=16{return None;}if next.is_none(){next=Some(p+1);}p=to;jumps+=1;continue;}
        if k>63{return None;}let part=b.get(p..p.checked_add(k as usize)?)?;p+=k as usize;
        if n>0{*out.get_mut(n)?=b'.';n+=1;}for ch in part{*out.get_mut(n)?=ch.to_ascii_lowercase();n+=1;}
    }
}
fn dns_answer(b:&[u8],q:&[u8])->Option<[u8;4]>{
    if b.len()<12||q.len()<12||be16(b,0)!=be16(q,0)||be16(b,2)&0xfa0f!=0x8000||be16(b,4)!=1{return None;}
    let mut target=[0;253];let (mut tl,qe)=name(q,12,&mut target)?;let mut owner=[0;253];let (ol,p)=name(b,12,&mut owner)?;
    if target[..tl]!=owner[..ol]||b.get(p..p+4)!=q.get(qe..qe+4){return None;}let count=be16(b,6)as usize;if count>32{return None;}
    for _ in 0..8{let mut at=p+4;let mut alias=None;
        for _ in 0..count{let (ol,e)=name(b,at,&mut owner)?;let h=b.get(e..e+10)?;let typ=be16(h,0);let class=be16(h,2);let len=be16(h,8)as usize;let off=e+10;let val=b.get(off..off.checked_add(len)?)?;at=off+len;
            if class!=1||target[..tl]!=owner[..ol]{continue;}
            if typ==1&&len==4{let ip:[u8;4]=val.try_into().ok()?;if valid(ip){return Some(ip);}}
            if typ==5{let mut a=[0;253];let (al,finish)=name(b,off,&mut a)?;if finish!=off+len{return None;}alias=Some((a,al));}
        }let(a,al)=alias?;target=a;tl=al;
    }None
}
fn decimal(b:&[u8],max:usize)->Option<usize>{if b.is_empty(){return None;}let mut n=0usize;for x in b{if !x.is_ascii_digit(){return None;}n=n.checked_mul(10)?.checked_add((x-b'0')as usize)?;if n>max{return None;}}Some(n)}
pub fn self_test()->bool{
    if parse_url(b"http://10.0.2.100:8080/a")!=Some((b"10.0.2.100".as_slice(),8080,b"/a".as_slice()))||parse_url(b"https://example.com/").is_some()||parse_url(b"http://127.0.0.1/").is_some()||parse_url(b"http://example.com/").is_none(){return false;}
    if !crate::http::self_test(){return false;}
    let q=b"\x12\x34\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x03lab\x04test\x00\x00\x01\x00\x01";
    let mut r=[0;64];r[..q.len()].copy_from_slice(q);r[2]=0x81;r[3]=0x80;r[7]=1;let o=q.len();let ans=[0xc0,12,0,1,0,1,0,0,0,60,0,4,192,0,2,77];r[o..o+ans.len()].copy_from_slice(&ans);
    dns_answer(&r[..o+ans.len()],q)==Some([192,0,2,77])&&dns_answer(&r[..o+5],q).is_none()
}
