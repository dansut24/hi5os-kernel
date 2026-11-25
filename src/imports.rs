//! Bounded, read-only PE64 import inventory. No code execution or arbitrary DLL loading.
use crate::{compat,serial};
fn u16at(b:&[u8],p:usize)->Option<u16>{Some(u16::from_le_bytes(b.get(p..p.checked_add(2)?)?.try_into().ok()?))}
fn u32at(b:&[u8],p:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(p..p.checked_add(4)?)?.try_into().ok()?))}
fn u64at(b:&[u8],p:usize)->Option<u64>{Some(u64::from_le_bytes(b.get(p..p.checked_add(8)?)?.try_into().ok()?))}
#[derive(Clone,Copy,Default,Debug,PartialEq,Eq)]
pub struct Report{pub dlls:u32,pub functions:u32,pub fixture_apis:u32,pub delayed:bool,pub reloc:bool}
pub enum Symbol<'a>{Name(&'a str),Ordinal(u16)}
struct Pe<'a>{b:&'a[u8],opt:usize,sections:usize,count:usize,headers:usize,size:usize,dirs:usize}
impl<'a> Pe<'a>{
    fn new(b:&'a[u8])->Option<Self>{
        if compat::classify(b)!=compat::Kind::Pe64{return None;}
        let pe=u32at(b,60)? as usize;let opt=pe.checked_add(24)?;let opts=u16at(b,pe+20)? as usize;
        let dirs=u32at(b,opt+108)? as usize;if dirs>16||112+dirs*8>opts{return None;}
        Some(Self{b,opt,sections:opt+opts,count:u16at(b,pe+6)? as usize,headers:u32at(b,opt+60)? as usize,size:u32at(b,opt+56)? as usize,dirs})
    }
    fn dir(&self,i:usize)->Option<(u32,u32)>{
        if i>=self.dirs{return Some((0,0));}
        let r=u32at(self.b,self.opt+112+i*8)?;let n=u32at(self.b,self.opt+116+i*8)?;
        if (r==0)!=(n==0)||r.checked_add(n)? as usize>self.size{return None;}Some((r,n))
    }
    // Map only file-backed ranges; reject ambiguous section aliases and out-of-bounds values.
    fn tail(&self,r:u32)->Option<&'a[u8]>{
        if r as usize>=self.size{return None;}
        let mut result=if (r as usize)<self.headers{Some(self.b.get(r as usize..self.headers)?)}else{None};
        for i in 0..self.count{
            let s=self.sections+40*i;let va=u32at(self.b,s+12)?;let len=u32at(self.b,s+16)?;let raw=u32at(self.b,s+20)?;
            if r>=va&&r-va<len{
                if result.is_some(){return None;}
                let from=raw.checked_add(r-va)? as usize;let to=raw.checked_add(len)? as usize;
                result=Some(self.b.get(from..to)?);
            }
        }result
    }
    fn bytes(&self,r:u32,n:usize)->Option<&'a[u8]>{self.tail(r)?.get(..n)}
    fn name(&self,r:u32)->Option<&'a str>{
        let t=self.tail(r)?;let n=t.iter().take(256).position(|c|*c==0)?;
        if n==0||!t[..n].iter().all(|c|(33..127).contains(c)&&*c!=b'|'){return None;}
        core::str::from_utf8(&t[..n]).ok()
    }
}
pub fn inspect(b:&[u8],mut emit:impl FnMut(&str,Symbol<'_>))->Result<Report,&'static str>{
    let p=Pe::new(b).ok_or("Invalid or unsupported PE64")?;
    let (r,n)=p.dir(1).ok_or("Invalid import directory")?;
    let mut report=Report{delayed:p.dir(13).ok_or("Invalid delay imports")?.0!=0,reloc:p.dir(5).ok_or("Invalid relocation directory")?.0!=0,..Report::default()};
    if r==0{return Ok(report);}
    if n<20||n>128*20+20{return Err("Import directory limit");}
    let descriptors=p.bytes(r,n as usize).ok_or("Import directory outside file")?;
    for d in descriptors.chunks_exact(20){
        if d.iter().all(|x|*x==0){return Ok(report);}
        if report.dlls>=128{return Err("Too many DLL imports");}
        let original=u32at(d,0).ok_or("ILT")?;let dll=p.name(u32at(d,12).ok_or("DLL RVA")?).ok_or("Invalid DLL name")?;
        let iat=u32at(d,16).ok_or("IAT")?;if iat==0{return Err("Missing IAT");}
        let lookup=if original!=0{original}else{iat};if lookup==0{return Err("Missing thunk table");}
        report.dlls+=1;let mut ended=false;
        for i in 0..=4096u32{
            let off=i.checked_mul(8).ok_or("Thunk overflow")?;
            let at=lookup.checked_add(off).ok_or("Thunk overflow")?;
            let v=u64at(p.bytes(at,8).ok_or("Thunk outside file")?,0).ok_or("Thunk")?;
            if v==0{ended=true;break;}
            if report.functions>=4096{return Err("Too many imports");}
            // IAT must also be backed by the file; this inventory never modifies it.
            p.bytes(iat.checked_add(off).ok_or("IAT overflow")?,8).ok_or("IAT outside file")?;
            report.functions+=1;
            if v&(1u64<<63)!=0{
                if v&0x7fff_ffff_ffff_0000!=0{return Err("Invalid ordinal");}emit(dll,Symbol::Ordinal(v as u16));
            }else{
                let hint=u32::try_from(v).map_err(|_|"Invalid name RVA")?;
                p.bytes(hint,2).ok_or("Invalid import hint")?;
                let name=p.name(hint.checked_add(2).ok_or("Name overflow")?).ok_or("Invalid function name")?;
                if dll.eq_ignore_ascii_case("kernel32.dll")&&name=="ExitProcess"{report.fixture_apis+=1;}
                emit(dll,Symbol::Name(name));
            }
        }
        if !ended{return Err("Unterminated thunk table");}
    }Err("Unterminated import descriptors")
}
pub fn log(b:&[u8])->Result<Report,&'static str>{
    serial::line("H5K: PE_INSPECT_BEGIN");
    let r=inspect(b,|dll,s|{serial::write("H5K: PE_DEP|");serial::write(dll);serial::write("|");match s{Symbol::Name(n)=>serial::line(n),Symbol::Ordinal(n)=>{serial::write("#");serial::hex(n as u64);serial::line("");}}})?;
    serial::metric("H5K: PE_DLL_COUNT=",r.dlls as u64);serial::metric("H5K: PE_FUNCTION_COUNT=",r.functions as u64);
    serial::metric("H5K: PE_FIXTURE_API_COUNT=",r.fixture_apis as u64);serial::metric("H5K: PE_DELAY_IMPORTS=",r.delayed as u64);serial::metric("H5K: PE_RELOCATIONS=",r.reloc as u64);
    serial::line("H5K: PE_INSPECT_OK");Ok(r)
}
pub fn self_test()->bool{
    let b=compat::HELLO;let mut correct=0;
    let r=inspect(b,|d,s|{if d.eq_ignore_ascii_case("kernel32.dll")&&matches!(s,Symbol::Name("ExitProcess")){correct+=1;}});
    if !matches!(r,Ok(Report{dlls:1,functions:1,fixture_apis:1,..}))||correct!=1||inspect(&b[..64],|_,_|{}).is_ok(){return false;}
    // Corrupt import directory RVA and ensure no out-of-bounds read or successful report.
    let mut bad=[0u8;4096];if b.len()>bad.len(){return false;}bad[..b.len()].copy_from_slice(b);
    let opt=u32at(b,60).unwrap()as usize+24;bad[opt+120..opt+124].copy_from_slice(&u32::MAX.to_le_bytes());
    inspect(&bad[..b.len()],|_,_|{}).is_err()
}
