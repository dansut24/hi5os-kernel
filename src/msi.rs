//! Native bounded read-only CFB/MSI stream inventory. No installer execution.
use crate::serial;
const FREE:u32=0xffffffff;const END:u32=0xfffffffe;const FAT:u32=0xfffffffd;
fn u16at(b:&[u8],o:usize)->Option<u16>{Some(u16::from_le_bytes(b.get(o..o.checked_add(2)?)?.try_into().ok()?))}
fn u32at(b:&[u8],o:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(o..o.checked_add(4)?)?.try_into().ok()?))}
fn u64at(b:&[u8],o:usize)->Option<u64>{Some(u64::from_le_bytes(b.get(o..o.checked_add(8)?)?.try_into().ok()?))}
struct Cfb<'a>{b:&'a[u8],sector:usize,n:usize,fats:usize,dir:u32,mini:u32,mini_count:usize,root:u32,root_size:usize}
impl<'a>Cfb<'a>{
    fn new(b:&'a[u8])->Option<Self>{
        if b.get(..8)?!=b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"||u16at(b,28)?!=0xfffe||u16at(b,32)?!=6||u32at(b,56)?!=4096{return None;}
        let major=u16at(b,26)?;let shift=u16at(b,30)?;if !matches!((major,shift),(3,9)|(4,12)){return None;}
        let sector=1usize<<shift;if b.len()<sector*2||b.len()%sector!=0{return None;}
        let n=b.len()/sector-1;let fats=u32at(b,44)?as usize;
        // Current MSI packages fit the header DIFAT. Extended DIFAT is deliberately rejected.
        if fats==0||fats>109||u32at(b,72)?!=0||u32at(b,68)?!=END||n>65536||fats*sector/4<n{return None;}
        let mut c=Self{b,sector,n,fats,dir:u32at(b,48)?,mini:u32at(b,60)?,mini_count:u32at(b,64)?as usize,root:END,root_size:0};
        for i in 0..fats{let id=u32at(b,76+4*i)?;if id as usize>=n||c.fat(id)!=Some(FAT){return None;}}
        let root=c.sector(c.dir)?.get(..128)?;
        if root[66]!=5{return None;}c.root=u32at(root,116)?;c.root_size=usize::try_from(u64at(root,120)?).ok()?;
        if c.root_size>c.n*c.sector||c.mini_count>c.n{return None;}Some(c)
    }
    fn sector(&self,id:u32)->Option<&'a[u8]>{let i=id as usize;if i>=self.n{return None;}let off=(i+1)*self.sector;self.b.get(off..off+self.sector)}
    fn fat(&self,id:u32)->Option<u32>{let i=id as usize;let per=self.sector/4;if i>=self.n||i/per>=self.fats{return None;}let s=u32at(self.b,76+4*(i/per))?;u32at(self.sector(s)?,4*(i%per))}
    fn nth(&self,mut id:u32,steps:usize)->Option<u32>{if steps>=self.n{return None;}for _ in 0..steps{id=self.fat(id)?;}self.sector(id)?;Some(id)}
    fn mini_next(&self,id:u32)->Option<u32>{let i=id as usize;let per=self.sector/4;if i/per>=self.mini_count{return None;}let s=self.nth(self.mini,i/per)?;u32at(self.sector(s)?,4*(i%per))}
    fn stream_crc(&self,mut id:u32,size:usize)->Option<u32>{
        if size==0{return if id==END||id==FREE{Some(0)}else{None};}
        if size>self.b.len(){return None;}let small=size<4096;let unit=if small{64}else{self.sector};let mut left=size;let mut crc=0xffffffffu32;
        while left!=0{
            let raw=if small{let off=(id as usize).checked_mul(64)?;if off.checked_add(64)?>self.root_size{return None;}let sid=self.nth(self.root,off/self.sector)?;let p=off%self.sector;self.sector(sid)?.get(p..p+64)?}else{self.sector(id)?};
            let take=left.min(unit);for v in &raw[..take]{crc^=*v as u32;for _ in 0..8{crc=(crc>>1)^(0xedb88320&0u32.wrapping_sub(crc&1));}}
            left-=take;id=if small{self.mini_next(id)?}else{self.fat(id)?};
        }
        if id!=END{return None;}Some(!crc)
    }
}
#[derive(Clone,Copy)]pub struct Report{pub streams:u32,pub bytes:u64}
pub fn inspect(b:&[u8])->Option<Report>{
    let c=Cfb::new(b)?;let mut id=c.dir;let mut steps=0;let mut report=Report{streams:0,bytes:0};
    serial::line("H5K: MSI_CFB_BEGIN");
    while id!=END{
        if steps>=c.n{return None;}steps+=1;let block=c.sector(id)?;
        for e in block.chunks_exact(128){
            let ty=e[66];if ty==0{continue;}if !matches!(ty,1|2|5){return None;}
            let n=u16at(e,64)?as usize;if n<2||n>64||n%2!=0||u16at(e,n-2)?!=0{return None;}
            if ty!=2{continue;}
            let size=usize::try_from(u64at(e,120)?).ok()?;let start=u32at(e,116)?;let crc=c.stream_crc(start,size)?;
            // Match host by raw UTF-16 name code units, including encoded MSI stream names.
            serial::write("H5K: MSI_STREAM|");let mut hex=[0u8;124];let mut used=0;
            for i in (0..n-2).step_by(2){let v=u16at(e,i)?;for j in 0..4{hex[used]=b"0123456789abcdef"[((v>>(12-j*4))&15)as usize];used+=1;}}
            serial::line(core::str::from_utf8(&hex[..used]).ok()?);
            serial::metric("H5K: MSI_STREAM_SIZE=",size as u64);serial::metric("H5K: MSI_STREAM_CRC=",crc as u64);
            report.streams=report.streams.checked_add(1)?;report.bytes=report.bytes.checked_add(size as u64)?;
        }id=c.fat(id)?;
    }
    if report.streams==0{return None;}serial::metric("H5K: MSI_STREAM_COUNT=",report.streams as u64);serial::line("H5K: MSI_CFB_OK");Some(report)
}
pub fn self_test()->bool{
    let mut b=[0u8;2048];b[..8].copy_from_slice(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1");
    b[26..28].copy_from_slice(&3u16.to_le_bytes());b[28..30].copy_from_slice(&0xfffeu16.to_le_bytes());b[30..32].copy_from_slice(&9u16.to_le_bytes());b[32..34].copy_from_slice(&6u16.to_le_bytes());
    for (o,v) in [(44,1),(48,1),(56,4096),(60,END),(64,0),(68,END),(72,0),(76,0)]{b[o..o+4].copy_from_slice(&v.to_le_bytes());}
    b[512..1024].fill(0xff);b[512..516].copy_from_slice(&FAT.to_le_bytes());b[516..520].copy_from_slice(&END.to_le_bytes());
    b[1024+66]=5;b[1024+116..1024+120].copy_from_slice(&END.to_le_bytes());
    if Cfb::new(&b).is_none()||Cfb::new(&b[..700]).is_some(){return false;}
    b[76..80].copy_from_slice(&99u32.to_le_bytes());Cfb::new(&b).is_none()
}
