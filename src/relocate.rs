//! Inert PE64 mapping and DIR64 relocation. No executable mappings or entry invocation.
//! Single-CPU scratch only, discarded on every call.
use crate::{compat,serial};
use core::ptr;
const CAP:usize=8*1024*1024;
static mut IMAGE:[u8;CAP]=[0;CAP];
static mut SEEN:[u8;CAP/8]=[0;CAP/8];
fn u16at(b:&[u8],p:usize)->Option<u16>{Some(u16::from_le_bytes(b.get(p..p.checked_add(2)?)?.try_into().ok()?))}
fn u32at(b:&[u8],p:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(p..p.checked_add(4)?)?.try_into().ok()?))}
fn u64at(b:&[u8],p:usize)->Option<u64>{Some(u64::from_le_bytes(b.get(p..p.checked_add(8)?)?.try_into().ok()?))}
#[derive(Clone,Copy,Default)]pub struct Report{pub size:usize,pub count:u32,pub base:u64,pub crc:u32}
#[derive(Clone,Copy)]struct Section{va:usize,end:usize,raw:usize,rp:usize,attr:u32}
const EMPTY:Section=Section{va:0,end:0,raw:0,rp:0,attr:0};
fn map(b:&[u8],out:&mut[u8],seen:&mut[u8])->Result<Report,&'static str>{
    if compat::classify(b)!=compat::Kind::Pe64{return Err("Invalid PE64");}
    let pe=u32at(b,60).ok_or("PE offset")?as usize;let opt=pe+24;
    let opts=u16at(b,pe+20).ok_or("Optional header")?as usize;
    let n=u16at(b,pe+6).ok_or("Section count")?as usize;
    let size=u32at(b,opt+56).ok_or("Image size")?as usize;let headers=u32at(b,opt+60).ok_or("Headers")?as usize;
    let entry=u32at(b,opt+16).ok_or("Entry")?as usize;let preferred=u64at(b,opt+24).ok_or("Image base")?;
    let sa=u32at(b,opt+32).ok_or("Alignment")?as usize;let fa=u32at(b,opt+36).ok_or("File alignment")?as usize;
    let flags=u16at(b,pe+22).ok_or("Flags")?;
    if flags&0x2001!=0||size==0||size>out.len()||size>CAP||size%4096!=0||seen.len()<size.div_ceil(8)||sa!=4096||!fa.is_power_of_two()||!(512..=4096).contains(&fa)||headers>size||headers%fa!=0||preferred==0||preferred%65536!=0{return Err("Unsupported relocatable image layout");}
    let base=preferred.checked_add(0x10000000).ok_or("Base overflow")?;
    if base.checked_add(size as u64).map_or(true,|v|v>0x0000_8000_0000_0000){return Err("Noncanonical image range");}
    let dirs=u32at(b,opt+108).ok_or("Directories")?as usize;if !(6..=16).contains(&dirs)||112+dirs*8>opts{return Err("Invalid directories");}
    let rr=u32at(b,opt+112+5*8).ok_or("Relocation RVA")?as usize;let rn=u32at(b,opt+116+5*8).ok_or("Relocation size")?as usize;
    let re=rr.checked_add(rn).ok_or("Relocation overflow")?;if rr==0||rn<8||re>size{return Err("Missing relocation directory");}
    let mut sections=[EMPTY;96];let mut exec=false;
    for i in 0..n{
        let s=opt+opts+i*40;let vs=u32at(b,s+8).ok_or("Virtual size")?as usize;let va=u32at(b,s+12).ok_or("Section RVA")?as usize;
        let raw=u32at(b,s+16).ok_or("Raw size")?as usize;let rp=u32at(b,s+20).ok_or("Raw offset")?as usize;let attr=u32at(b,s+36).ok_or("Attributes")?;
        let span=vs.max(raw);let end=va.checked_add(span).ok_or("Section overflow")?;let rounded=end.checked_add(4095).ok_or("Section rounding")?&!4095;
        if span==0||va%4096!=0||va<headers||rounded>size||raw!=0&&(rp<headers||rp%fa!=0||raw%fa!=0)||attr&0xa0000000==0xa0000000{return Err("Invalid section");}
        if sections[..i].iter().any(|q|va<q.end.div_ceil(4096)*4096&&q.va<rounded){return Err("Overlapping sections");}
        if raw!=0&&sections[..i].iter().any(|q|q.raw!=0&&rp<q.rp+q.raw&&q.rp<rp+raw){return Err("Overlapping raw sections");}
        if attr&0x20000000!=0&&entry>=va&&entry<va+vs.min(raw){exec=true;}
        sections[i]=Section{va,end,raw,rp,attr};
    }
    if !exec{return Err("Invalid entry");}
    let sec=sections[..n].iter().find(|s|rr>=s.va&&re<=s.va+s.raw).ok_or("Relocations not file-backed")?;
    let table=b.get(sec.rp+rr-sec.va..sec.rp+re-sec.va).ok_or("Relocation table bounds")?;
    out[..size].fill(0);seen[..size.div_ceil(8)].fill(0);out[..headers].copy_from_slice(&b[..headers]);
    for s in &sections[..n]{if s.raw!=0{out[s.va..s.va+s.raw].copy_from_slice(&b[s.rp..s.rp+s.raw]);}}
    let mut p=0;let mut count=0u32;
    while p<table.len(){
        let page=u32at(table,p).ok_or("Relocation page")?as usize;let len=u32at(table,p+4).ok_or("Relocation block")?as usize;
        let end=p.checked_add(len).ok_or("Block overflow")?;
        if page%4096!=0||page>=size||len<8||len%4!=0||end>table.len(){return Err("Invalid relocation block");}
        for i in (p+8..end).step_by(2){
            let v=u16at(table,i).ok_or("Relocation entry")?;let ty=v>>12;if ty==0{continue;}if ty!=10{return Err("Unsupported relocation type");}
            let at=page+(v&4095)as usize;let end=at.checked_add(8).ok_or("Relocation target overflow")?;
            if end>size||at<re&&rr<end||!sections[..n].iter().any(|s|at>=s.va&&end<=s.end&&s.attr&0xe0000000!=0){return Err("Invalid relocation target");}
            for j in at..end{let mask=1u8<<(j%8);if seen[j/8]&mask!=0{return Err("Overlapping relocation targets");}seen[j/8]|=mask;}
            let old=u64at(out,at).ok_or("Relocation value")?;
            let new=old.wrapping_add(base-preferred);out[at..end].copy_from_slice(&new.to_le_bytes());
            count=count.checked_add(1).ok_or("Relocation count")?;
        }p=end;
    }
    if count==0{return Err("No DIR64 relocations");}
    let mut crc=0xffff_ffffu32;for x in &out[..size]{crc^=*x as u32;for _ in 0..8{crc=(crc>>1)^if crc&1!=0{0xedb88320}else{0};}}
    Ok(Report{size,count,base,crc:!crc})
}
pub fn inspect(b:&[u8])->Result<Report,&'static str>{
    // Synchronous single CPU UI and boot callers; no interrupt handler accesses scratch.
    let r=unsafe{map(b,&mut *ptr::addr_of_mut!(IMAGE),&mut *ptr::addr_of_mut!(SEEN))}?;
    serial::metric("H5K: PE_REBASE_SIZE=",r.size as u64);serial::metric("H5K: PE_REBASE_BASE=",r.base);
    serial::metric("H5K: PE_REBASE_COUNT=",r.count as u64);serial::metric("H5K: PE_REBASE_CRC=",r.crc as u64);
    serial::line("H5K: PE_REBASE_OK");Ok(r)
}
pub fn self_test()->bool{
    let mut b=[0u8;1536];
    fn w16(b:&mut[u8],p:usize,v:u16){b[p..p+2].copy_from_slice(&v.to_le_bytes());}
    fn w32(b:&mut[u8],p:usize,v:u32){b[p..p+4].copy_from_slice(&v.to_le_bytes());}
    b[..2].copy_from_slice(b"MZ");w32(&mut b,60,64);b[64..68].copy_from_slice(b"PE\0\0");
    w16(&mut b,68,0x8664);w16(&mut b,70,2);w16(&mut b,84,240);w16(&mut b,86,2);w16(&mut b,88,0x20b);
    w32(&mut b,104,4096);w32(&mut b,112,0x400000);w32(&mut b,120,4096);w32(&mut b,124,512);w32(&mut b,144,12288);w32(&mut b,148,512);
    w32(&mut b,196,16);w32(&mut b,240,8192);w32(&mut b,244,12);
    for (s,va,rp,attr) in [(328,4096,512,0x60000020),(368,8192,1024,0x42000040)]{w32(&mut b,s+8,512);w32(&mut b,s+12,va);w32(&mut b,s+16,512);w32(&mut b,s+20,rp);w32(&mut b,s+36,attr);}
    b[512]=0xc3;b[528..536].copy_from_slice(&0x401000u64.to_le_bytes());w32(&mut b,1024,4096);w32(&mut b,1028,12);w16(&mut b,1032,0xa010);
    let mut out=[0u8;12288];let mut seen=[0u8;1536];
    if !matches!(map(&b,&mut out,&mut seen),Ok(Report{count:1,base:0x10400000,..}))||u64at(&out,4112)!=Some(0x10401000){return false;}
    w16(&mut b,1034,0xa010);if map(&b,&mut out,&mut seen).is_ok(){return false;}
    w16(&mut b,1034,0);w16(&mut b,1032,0x3010);map(&b,&mut out,&mut seen).is_err()
}
