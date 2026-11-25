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

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Image{pub entry:usize,pub size:usize,pub preferred_base:u64}
fn u64at(b:&[u8],i:usize)->Option<u64>{Some(u64::from_le_bytes(b.get(i..i.checked_add(8)?)?.try_into().ok()?))}
pub fn stage_pe64(b:&[u8],out:&mut[u8])->Result<Image,&'static str>{stage(b,out,false)}
fn stage(b:&[u8],out:&mut[u8],fixture_import:bool)->Result<Image,&'static str>{
    if classify(b)!=Kind::Pe64{return Err("Not a validated PE64 candidate");}
    let pe=u32at(b,60).unwrap()as usize;let opt=pe+24;
    let flags=u16at(b,pe+22).unwrap();if flags&0x2000!=0{return Err("DLL unsupported");}
    let sections=u16at(b,pe+6).unwrap()as usize;let opts=u16at(b,pe+20).unwrap()as usize;
    let size=u32at(b,opt+56).unwrap()as usize;let headers=u32at(b,opt+60).unwrap()as usize;
    let entry=u32at(b,opt+16).unwrap()as usize;let base=u64at(b,opt+24).unwrap();
    let sa=u32at(b,opt+32).unwrap()as usize;let fa=u32at(b,opt+36).unwrap()as usize;
    if size>out.len()||size>1024*1024||size%4096!=0||sa!=4096||!fa.is_power_of_two()||!(512..=4096).contains(&fa)||headers%fa!=0||headers>size||base==0||base%65536!=0{return Err("Unsupported image layout");}
    if u16at(b,opt+68)!=Some(3){return Err("Only console PE64 fixtures supported");}
    let dirs=u32at(b,opt+108).unwrap()as usize;
    if dirs>16||112+dirs*8>opts{return Err("Invalid data directory table");}
    for i in 0..dirs{if fixture_import&&matches!(i,1|12){continue;}if u32at(b,opt+112+i*8)!=Some(0)||u32at(b,opt+116+i*8)!=Some(0){return Err("Imports, relocations or other directories unsupported");}}
    let mut ranges=[(0usize,0usize);96];let mut exec=false;
    for i in 0..sections{
        let s=opt+opts+i*40;let vs=u32at(b,s+8).unwrap()as usize;let va=u32at(b,s+12).unwrap()as usize;
        let raw=u32at(b,s+16).unwrap()as usize;let rp=u32at(b,s+20).unwrap()as usize;let attr=u32at(b,s+36).unwrap();
        let span=vs.max(raw);let end=va.checked_add(span).ok_or("Section overflow")?;
        let rounded=end.checked_add(4095).ok_or("Section overflow")?&!4095;
        if span==0||va%4096!=0||va<headers||rounded>size||raw!=0&&(rp<headers||rp%fa!=0||raw%fa!=0)||attr&0xa0000000==0xa0000000{return Err("Invalid or W+X section");}
        if ranges[..i].iter().any(|&(lo,hi)|va<hi&&lo<rounded){return Err("Overlapping sections");}ranges[i]=(va,rounded);
        if attr&0x20000000!=0&&entry>=va&&entry<va+raw.min(vs){exec=true;}
    }
    if !exec{return Err("Entry point not in executable file-backed section");}
    // Only after validation: copy headers/sections and zero the rest. Never jump to entry.
    out[..size].fill(0);out[..headers].copy_from_slice(&b[..headers]);
    for i in 0..sections{let s=opt+opts+i*40;let va=u32at(b,s+12).unwrap()as usize;let raw=u32at(b,s+16).unwrap()as usize;let rp=u32at(b,s+20).unwrap()as usize;if raw!=0{out[va..va+raw].copy_from_slice(&b[rp..rp+raw]);}}
    Ok(Image{entry,size,preferred_base:base})
}
pub fn stage_test()->bool{
    let mut b=[0u8;1024];b[..2].copy_from_slice(b"MZ");put32(&mut b,60,64);b[64..68].copy_from_slice(b"PE\0\0");
    put16(&mut b,68,0x8664);put16(&mut b,70,1);put16(&mut b,84,112);put16(&mut b,86,2);put16(&mut b,88,0x20b);
    put32(&mut b,104,4096);put32(&mut b,112,0x400000);put32(&mut b,120,4096);put32(&mut b,124,512);put32(&mut b,144,8192);put32(&mut b,148,512);put16(&mut b,156,3);
    put32(&mut b,208,512);put32(&mut b,212,4096);put32(&mut b,216,512);put32(&mut b,220,512);put32(&mut b,236,0x60000020);b[512]=0xc3;
    let mut out=[0xa5u8;8192];
    if stage_pe64(&b,&mut out)!=Ok(Image{entry:4096,size:8192,preferred_base:0x400000})||out[4096]!=0xc3||out[512]!=0||out[8191]!=0{return false;}
    put32(&mut b,236,0xe0000020);if stage_pe64(&b,&mut out).is_ok(){return false;}
    put32(&mut b,236,0x60000020);put32(&mut b,104,512);stage_pe64(&b,&mut out).is_err()
}

// Only the CI-built, image-embedded program is eligible for execution. No browser/file input.
pub const HELLO:&[u8]=include_bytes!("../target/hi5-fixture/hello.exe");
pub fn stage_hello(out:&mut[u8],shim:u64)->Result<Image,&'static str>{
    let b=HELLO;let image=stage(b,out,true)?;
    if image.preferred_base!=0x1_0000_0000||image.size!=0x3000||image.entry!=0x1000{return Err("Unexpected fixture image layout");}
    let opt=u32at(b,60).ok_or("PE offset")? as usize+24;
    let imp=u32at(b,opt+120).ok_or("Import directory")?as usize;
    let imp_size=u32at(b,opt+124).ok_or("Import size")?as usize;
    let iat=u32at(b,opt+112+12*8).ok_or("IAT")?as usize;
    let iat_size=u32at(b,opt+116+12*8).ok_or("IAT size")?as usize;
    let mem=&mut out[..image.size];
    if imp_size!=40||iat_size!=16||!(0x2000..=0x2fd8).contains(&imp)||!(0x2000..=0x2ff0).contains(&iat){return Err("Unexpected import layout");}
    if mem[imp+20..imp+40].iter().any(|v|*v!=0)||u32at(mem,imp+4)!=Some(0)||u32at(mem,imp+8)!=Some(0){return Err("Multiple or bound imports unsupported");}
    let ilt=u32at(mem,imp).ok_or("ILT")?as usize;let name=u32at(mem,imp+12).ok_or("DLL name")?as usize;
    if u32at(mem,imp+16)!=Some(iat as u32)||!(0x2000..=0x2ff0).contains(&ilt)||!(0x2000..=0x2ff3).contains(&name){return Err("Import pointer out of range");}
    if !mem.get(name..name+13).is_some_and(|v|v.eq_ignore_ascii_case(b"KERNEL32.dll\0")){return Err("Unsupported DLL");}
    let thunk=u64at(mem,ilt).ok_or("Import thunk")?;
    if !(0x2000..=0x2ff0).contains(&thunk)||u64at(mem,ilt+8)!=Some(0)||u64at(mem,iat)!=Some(thunk)||u64at(mem,iat+8)!=Some(0){return Err("Unsupported import thunk");}
    let name=thunk as usize+2;
    if mem.get(name..name+12)!=Some(b"ExitProcess\0".as_slice()){return Err("Unsupported Win32 API");}
    mem[iat..iat+8].copy_from_slice(&shim.to_le_bytes());Ok(image)
}

const RUNTIME:&[u8]=include_bytes!("../target/hi5-fixture/runtime.exe");
pub fn stage_runtime(out:&mut[u8],shim:u64)->Result<Image,&'static str>{
    let image=stage(RUNTIME,out,true)?;
    if image.preferred_base!=0x1_0000_0000||image.size!=0x3000||image.entry!=0x1000{return Err("Unexpected runtime fixture layout");}
    let mut mask=0u8;let mut valid=true;
    let r=crate::imports::inspect_slots(RUNTIME,|dll,s,slot|{
        if !dll.eq_ignore_ascii_case("kernel32.dll"){valid=false;return;}
        let id=match s{crate::imports::Symbol::Name("ExitProcess")=>0,crate::imports::Symbol::Name("GetCurrentProcessId")=>1,
            crate::imports::Symbol::Name("SetLastError")=>2,crate::imports::Symbol::Name("GetLastError")=>3,_=>{valid=false;return;}};
        let slot=slot as usize;if mask&(1<<id)!=0||!(0x2000..=0x2ff8).contains(&slot)||slot%8!=0{valid=false;return;}
        mask|=1<<id;out[slot..slot+8].copy_from_slice(&(shim+id*16).to_le_bytes());
    })?;
    if !valid||mask!=15||r.dlls!=1||r.functions!=4{return Err("Unexpected runtime imports");}Ok(image)
}
