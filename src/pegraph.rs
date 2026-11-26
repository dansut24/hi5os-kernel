//! Validated export lookup, for mapped PE64 images. Forwarders and unsupported directories fail closed.
use crate::imports::Symbol;
fn u16at(b:&[u8],i:usize)->Option<u16>{Some(u16::from_le_bytes(b.get(i..i.checked_add(2)?)?.try_into().ok()?))}
fn u32at(b:&[u8],i:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(i..i.checked_add(4)?)?.try_into().ok()?))}
pub fn entry(b:&[u8])->Option<u32>{let p=u32at(b,60)?as usize;u32at(b,p+40)}
pub fn executable(b:&[u8],rva:u32)->bool{
    let Some(pe)=u32at(b,60).map(|v|v as usize)else{return false;};
    let (Some(n),Some(opt))=(u16at(b,pe+6),u16at(b,pe+20))else{return false;};
    (0..n as usize).any(|i|{let p=pe+24+opt as usize+i*40;let v=u32at(b,p+12).unwrap_or(u32::MAX);let size=u32at(b,p+8).unwrap_or(0);u32at(b,p+36).unwrap_or(0)&0x20000000!=0&&rva>=v&&rva-v<size})
}
pub fn export(b:&[u8],symbol:Symbol<'_>)->Option<u32>{
    let pe=u32at(b,60)?as usize;let opt=pe+24;
    if u32at(b,opt+108)?==0{return None;}
    let r=u32at(b,opt+112)?as usize;let n=u32at(b,opt+116)?as usize;let end=r.checked_add(n)?;if r==0||n<40||end>b.len(){return None;}
    let base=u32at(b,r+16)?;let count=u32at(b,r+20)?as usize;let names=u32at(b,r+24)?as usize;
    if count==0||count>65536||names>count{return None;}
    let funcs=u32at(b,r+28)?as usize;let name_table=u32at(b,r+32)?as usize;let ord=u32at(b,r+36)?as usize;
    b.get(funcs..funcs.checked_add(count*4)?)?;b.get(name_table..name_table.checked_add(names*4)?)?;b.get(ord..ord.checked_add(names*2)?)?;
    let i=match symbol{Symbol::Ordinal(v)=>u32::from(v).checked_sub(base)?as usize,Symbol::Name(name)=>{
        let mut found=None;for j in 0..names{let p=u32at(b,name_table+j*4)?as usize;let tail=b.get(p..)?;let len=tail.iter().take(256).position(|v|*v==0)?;if &tail[..len]==name.as_bytes(){found=Some(u16at(b,ord+j*2)?as usize);break;}}found?
    }};
    if i>=count{return None;}let v=u32at(b,funcs+i*4)?;if (v as usize)>=r&&(v as usize)<end||!executable(b,v){return None;}Some(v)
}
pub fn no_special_dirs(b:&[u8])->bool{
    let Some(pe)=u32at(b,60).map(|x|x as usize)else{return false;};let opt=pe+24;
    let Some(n)=u32at(b,opt+108)else{return false;};[9usize,13].iter().all(|i|*i>=n as usize||u32at(b,opt+112+8*i)==Some(0)&&u32at(b,opt+116+8*i)==Some(0))
}
