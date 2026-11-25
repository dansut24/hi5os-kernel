//! Hi5Net: experimental polled E1000 and restricted single-connection HTTP/1.0.
//! QEMU user network only: static 10.0.2.15, numeric 10.0.2.x destinations.
//! No DHCP, DNS, TLS, public web, routing, or general TCP sockets.
use crate::{platform as p,serial};
use core::{ptr,sync::atomic::{fence,Ordering}};
const N:usize=16;
const B:usize=2048;
const IP:[u8;4]=[10,0,2,15];
pub const MAX_BODY:usize=4096;
pub const MAX_RESPONSE:usize=6144;
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
pub struct Nic{base:usize,mac:[u8;6],rx:usize,tx:usize,seq:u32,port:u16}
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
            serial::line("H5K: E1000_READY");return Some(Self{base,mac,rx:0,tx:0,seq:0x48350000,port:49152});
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
        put16(&mut b,14,1);put16(&mut b,16,0x800);b[18]=6;b[19]=4;put16(&mut b,20,1);b[22..28].copy_from_slice(&self.mac);b[28..32].copy_from_slice(&IP);b[38..42].copy_from_slice(&ip);
        let mut r=[0;B];
        for _ in 0..3{if !self.send(&b){return None;}let end=p::ticks()+100;
            while before(end){let n=self.recv(&mut r);if n>=42&&be16(&r,12)==0x806&&be16(&r,14)==1&&be16(&r,16)==0x800&&r[18]==6&&r[19]==4&&be16(&r,20)==2&&r[28..32]==ip&&r[38..42]==IP&&r[32..38]==self.mac&&r[6..12]==r[22..28]{let mut m=[0;6];m.copy_from_slice(&r[22..28]);if m[0]&1==0&&m!=[0;6]{return Some(m);}}if n==0{p::wait_tick();}}
        }None
    }
    fn answer_arp(&mut self,r:&[u8]){
        if r.len()<42||be16(r,12)!=0x806||be16(r,14)!=1||be16(r,16)!=0x800||r[18]!=6||r[19]!=4||be16(r,20)!=1||r[38..42]!=IP||r[6..12]!=r[22..28]{return;}
        let mut a=[0u8;42];a[..6].copy_from_slice(&r[22..28]);a[6..12].copy_from_slice(&self.mac);put16(&mut a,12,0x806);put16(&mut a,14,1);put16(&mut a,16,0x800);a[18]=6;a[19]=4;put16(&mut a,20,2);a[22..28].copy_from_slice(&self.mac);a[28..32].copy_from_slice(&IP);a[32..42].copy_from_slice(&r[22..32]);let _=self.send(&a);
    }
    fn packet(&mut self,mac:[u8;6],ip:[u8;4],port:u16,seq:u32,ack:u32,flags:u8,data:&[u8])->bool{
        if data.len()>1000{return false;}let mut b=[0u8;1054];let n=54+data.len();b[..6].copy_from_slice(&mac);b[6..12].copy_from_slice(&self.mac);put16(&mut b,12,0x800);
        b[14]=0x45;put16(&mut b,16,(40+data.len())as u16);put16(&mut b,20,0x4000);b[22]=64;b[23]=6;b[26..30].copy_from_slice(&IP);b[30..34].copy_from_slice(&ip);let cs=fold(sum(&b[14..34]));put16(&mut b,24,cs);
        put16(&mut b,34,self.port);put16(&mut b,36,port);put32(&mut b,38,seq);put32(&mut b,42,ack);b[46]=0x50;b[47]=flags;put16(&mut b,48,4096);b[54..n].copy_from_slice(data);
        let cs=tcp_sum(IP,ip,&b[34..n]);put16(&mut b,50,cs);self.send(&b[..n])
    }
    fn segment<'a>(&mut self,r:&'a[u8],ip:[u8;4],port:u16)->Option<(u32,u32,u8,&'a[u8])>{
        if r.len()<54||r[..6]!=self.mac||be16(r,12)!=0x800||r[14]>>4!=4||r[23]!=6||r[26..30]!=ip||r[30..34]!=IP||be16(r,20)&0x3fff!=0{return None;}
        let ih=(r[14]&15)as usize*4;let total=be16(r,16)as usize;
        if ih<20||total<ih+20||14+total>r.len()||fold(sum(&r[14..14+ih]))!=0{return None;}
        let t=&r[14+ih..14+total];let h=(t[12]>>4)as usize*4;
        if h<20||h>t.len()||be16(t,0)!=port||be16(t,2)!=self.port||tcp_sum(ip,IP,t)!=0{return None;}
        Some((be32(t,4),be32(t,8),t[13],&t[h..]))
    }
    pub fn get(&mut self,url:&[u8],response:&mut[u8;MAX_RESPONSE])->Result<usize,&'static str>{
        let (ip,port,path)=parse_url(url).ok_or("Use http://10.0.2.x:port/path")?;
        self.port=if self.port==65535{49152}else{self.port+1};self.seq=self.seq.wrapping_add(0x10001);
        let mac=self.arp(ip).ok_or("ARP timeout")?;serial::line("H5K: NET_ARP_OK");
        let mut rx=[0;B];let mut remote=0;let mut connected=false;
        for _ in 0..2{if !self.packet(mac,ip,port,self.seq,0,2,&[]){return Err("TX timeout");}
            let end=p::ticks()+200;
            while before(end){let n=self.recv(&mut rx);if n==0{p::wait_tick();continue;}self.answer_arp(&rx[..n]);if let Some((s,a,f,_))=self.segment(&rx[..n],ip,port){if f&4!=0{return Err("TCP reset");}if f&0x12==0x12&&a==self.seq.wrapping_add(1){remote=s.wrapping_add(1);connected=true;break;}}}if connected{break;}
        }
        if !connected{return Err("TCP connect timeout");}
        self.seq=self.seq.wrapping_add(1);if !self.packet(mac,ip,port,self.seq,remote,0x10,&[]){return Err("ACK failed");}
        serial::line("H5K: NET_TCP_OK");
        let mut req=[0u8;256];let mut k=0;
        for part in [b"GET ".as_slice(),path,b" HTTP/1.0\r\nHost: ".as_slice(),&url[7..url.iter().enumerate().skip(7).find(|(_,b)|**b==b'/').map(|(i,_)|i).unwrap_or(url.len())],b"\r\nConnection: close\r\n\r\n".as_slice()]{if k+part.len()>req.len(){return Err("URL too long");}req[k..k+part.len()].copy_from_slice(part);k+=part.len();}
        if !self.packet(mac,ip,port,self.seq,remote,0x18,&req[..k]){return Err("Request failed");}self.seq=self.seq.wrapping_add(k as u32);
        let end=p::ticks()+500;let mut used=0;
        while before(end){
            let n=self.recv(&mut rx);if n==0{p::wait_tick();continue;}self.answer_arp(&rx[..n]);
            if let Some((s,a,f,data))=self.segment(&rx[..n],ip,port){
                if f&4!=0{return Err("TCP reset");}if f&0x10==0||a!=self.seq{continue;}
                if s!=remote{let _=self.packet(mac,ip,port,self.seq,remote,0x10,&[]);continue;}
                if used+data.len()>response.len(){return Err("Response exceeds preview limit");}
                response[used..used+data.len()].copy_from_slice(data);used+=data.len();remote=remote.wrapping_add(data.len()as u32);
                if f&1!=0{remote=remote.wrapping_add(1);}
                if !data.is_empty()||f&1!=0{let _=self.packet(mac,ip,port,self.seq,remote,0x10,&[]);}
                if let Some((start,len))=http_body(&response[..used])?{if used>=start+len{
                    if used!=start+len{return Err("Unexpected HTTP trailing bytes");}
                    let _=self.packet(mac,ip,port,self.seq,remote,0x14,&[]);
                    response.copy_within(start..start+len,0);serial::metric("H5K: HTTP_DOWNLOAD_BYTES=",len as u64);serial::line("H5K: HTTP_DOWNLOAD_OK");return Ok(len);
                }}
                if f&1!=0{return Err("Incomplete HTTP body");}
            }
        }Err("HTTP timeout")
    }
}
fn parse_url(url:&[u8])->Option<([u8;4],u16,&[u8])>{
    let rest=url.strip_prefix(b"http://")?;let end=rest.iter().position(|b|*b==b'/').unwrap_or(rest.len());
    let host=&rest[..end];let path=if end==rest.len(){b"/".as_slice()}else{&rest[end..]};
    if path.len()>120||!path.iter().all(|b|(33..127).contains(b)&&*b!=b'#'){return None;}
    let mut parts=host.split(|b|*b==b':');let addr=parts.next()?;let port=match parts.next(){Some(v)=>decimal(v,65535)? as u16,None=>80};if port==0||parts.next().is_some(){return None;}
    let mut ip=[0u8;4];let mut oct=addr.split(|b|*b==b'.');for i in 0..4{ip[i]=decimal(oct.next()?,255)? as u8;}if oct.next().is_some()||ip[..3]!=[10,0,2]||matches!(ip[3],0|15|255){return None;}
    Some((ip,port,path))
}
fn decimal(b:&[u8],max:usize)->Option<usize>{if b.is_empty(){return None;}let mut n=0usize;for x in b{if !x.is_ascii_digit(){return None;}n=n.checked_mul(10)?.checked_add((x-b'0')as usize)?;if n>max{return None;}}Some(n)}
fn http_body(b:&[u8])->Result<Option<(usize,usize)>,&'static str>{
    let Some(i)=b.windows(4).position(|w|w==b"\r\n\r\n")else{if b.len()>2048{return Err("HTTP header too large");}return Ok(None);};
    if i>2048{return Err("HTTP header too large");}
    let mut lines=b[..i].split(|x|*x==10);let status=lines.next().ok_or("Missing HTTP status")?;
    if !(status.starts_with(b"HTTP/1.0 200 ")||status.starts_with(b"HTTP/1.1 200 ")){return Err("HTTP status not 200");}
    let mut len=None;for line in lines{let line=line.strip_suffix(b"\r").unwrap_or(line);let Some(col)=line.iter().position(|b|*b==b':')else{return Err("Invalid HTTP header");};let name=&line[..col];let value=&line[col+1..];let value=value.strip_prefix(b" ").unwrap_or(value);
        if name.eq_ignore_ascii_case(b"transfer-encoding"){return Err("Chunked HTTP not supported");}
        if name.eq_ignore_ascii_case(b"content-length"){if len.is_some(){return Err("Duplicate length");}len=Some(decimal(value,MAX_BODY).ok_or("File exceeds 4096 bytes")?);}
    }Ok(Some((i+4,len.ok_or("Content-Length required")?)))
}
pub fn self_test()->bool{
    parse_url(b"http://10.0.2.100:8080/a")==Some(([10,0,2,100],8080,b"/a".as_slice()))&&parse_url(b"https://10.0.2.100/a").is_none()&&parse_url(b"http://8.8.8.8/a").is_none()&&
    http_body(b"HTTP/1.0 200 OK\r\nContent-Length: 3\r\n\r\nabc")==Ok(Some((b"HTTP/1.0 200 OK\r\nContent-Length: 3\r\n\r\n".len(),3)))&&http_body(b"HTTP/1.0 200 OK\r\nContent-Length: 99999\r\n\r\n").is_err()
}
