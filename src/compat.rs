//! Hi5Compat file-format validation foundation, not a loader or MSI engine.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind{Pe64,Pe32,Compound,Unknown}
fn u16at(b:&[u8],i:usize)->Option<u16>{Some(u16::from_le_bytes(b.get(i..i.checked_add(2)?)?.try_into().ok()?))}
fn u32at(b:&[u8],i:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(i..i.checked_add(4)?)?.try_into().ok()?))}
pub fn classify(b:&[u8])->Kind{
    if b.starts_with(&[0xd0,0xcf,0x11,0xe0,0xa1,0xb1,0x1a,0xe1]) {
        // Generic OLE compound file, NOT proof of an MSI database.
        if b.len()>=512&&u16at(b,28)==Some(0xfffe)&&matches!((u16at(b,26),u16at(b,30)),(Some(3),Some(9))|(Some(4),Some(12)))&&u16at(b,32)==Some(6){return Kind::Compound;}return Kind::Unknown;
    }
    if !b.starts_with(b"MZ"){return Kind::Unknown;}
    let Some(pe)=u32at(b,0x3c).map(|x|x as usize) else{return Kind::Unknown;};
    if pe<64||b.get(pe..pe.saturating_add(4))!=Some(&b"PE\0\0"[..]){return Kind::Unknown;}
    let Some(end)=pe.checked_add(24) else{return Kind::Unknown;};if end>b.len(){return Kind::Unknown;}
    let machine=u16at(b,pe+4);let n=u16at(b,pe+6).unwrap_or(0)as usize;
    let opt=u16at(b,pe+20).unwrap_or(0)as usize;let flags=u16at(b,pe+22).unwrap_or(0);
    if n==0||n>96||flags&2==0{return Kind::Unknown;}
    let kind=match(machine,u16at(b,end)){(Some(0x8664),Some(0x20b)) if opt>=112=>Kind::Pe64,(Some(0x14c),Some(0x10b)) if opt>=96=>Kind::Pe32,_=>return Kind::Unknown};
    let Some(sections)=end.checked_add(opt) else{return Kind::Unknown;};
    let Some(table_end)=sections.checked_add(n*40) else{return Kind::Unknown;};if table_end>b.len(){return Kind::Unknown;}
    let image=u32at(b,end+56).unwrap_or(0);let headers=u32at(b,end+60).unwrap_or(0);let entry=u32at(b,end+16).unwrap_or(0);
    if image==0||image>256*1024*1024||(headers as usize)<table_end||headers as usize>b.len()||entry>=image{return Kind::Unknown;}
    for j in 0..n{let s=sections+j*40;let vs=u32at(b,s+8).unwrap_or(0);let va=u32at(b,s+12).unwrap_or(0);let raw=u32at(b,s+16).unwrap_or(0);let rp=u32at(b,s+20).unwrap_or(0);
        if va.checked_add(vs.max(raw)).map_or(true,|v|v>image)||raw!=0&&rp.checked_add(raw).map_or(true,|v|v as usize>b.len()){return Kind::Unknown;}
    }
    kind
}
fn put16(b:&mut[u8],i:usize,v:u16){b[i..i+2].copy_from_slice(&v.to_le_bytes());}
fn put32(b:&mut[u8],i:usize,v:u32){b[i..i+4].copy_from_slice(&v.to_le_bytes());}
pub fn self_test()->bool{
    let mut b=[0u8;1024];b[..2].copy_from_slice(b"MZ");put32(&mut b,60,64);b[64..68].copy_from_slice(b"PE\0\0");
    put16(&mut b,68,0x8664);put16(&mut b,70,1);put16(&mut b,84,112);put16(&mut b,86,2);put16(&mut b,88,0x20b);
    put32(&mut b,104,4096);put32(&mut b,144,8192);put32(&mut b,148,512);
    put32(&mut b,208,1);put32(&mut b,212,4096);put32(&mut b,216,512);put32(&mut b,220,512);
    if classify(&b)!=Kind::Pe64||classify(&b[..100])!=Kind::Unknown{return false;}
    put32(&mut b,220,u32::MAX);if classify(&b)!=Kind::Unknown{return false;}
    put32(&mut b,220,512);put16(&mut b,68,0x14c);put16(&mut b,88,0x10b);if classify(&b)!=Kind::Pe32{return false;}
    b.fill(0);b[..8].copy_from_slice(&[0xd0,0xcf,0x11,0xe0,0xa1,0xb1,0x1a,0xe1]);put16(&mut b,26,3);put16(&mut b,28,0xfffe);put16(&mut b,30,9);put16(&mut b,32,6);
    classify(&b)==Kind::Compound&&classify(&b[..8])==Kind::Unknown
}
