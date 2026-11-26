//! Single local desktop account. Not a kernel security principal or encrypted storage.
use crate::{security,trial::sha256};
pub const ROUNDS:u32=600_000;
#[derive(Clone,Copy,PartialEq,Eq)]pub struct Account{pub name:[u8;32],pub len:u8,pub host:[u8;32],pub host_len:u8,pub region:u8,pub dark:bool,pub salt:[u8;16],pub verifier:[u8;32]}
pub fn valid_name(s:&[u8])->bool{!s.is_empty()&&s.len()<=32&&s[0].is_ascii_alphanumeric()&&s.iter().all(|b|b.is_ascii_alphanumeric()||*b==b'-'||*b==b'_')}
pub fn wipe(s:&mut[u8]){for x in s{unsafe{core::ptr::write_volatile(x,0);}}core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);}
fn hmac(key:&[u8],msg:&[u8])->[u8;32]{
    let mut inner=[0x36u8;96];let mut outer=[0x5cu8;96];
    for(i,k)in key.iter().enumerate(){inner[i]^=*k;outer[i]^=*k;}
    inner[64..64+msg.len()].copy_from_slice(msg);let mut digest=sha256(&inner[..64+msg.len()]);
    outer[64..].copy_from_slice(&digest);let out=sha256(&outer);wipe(&mut digest);wipe(&mut inner);wipe(&mut outer);out
}
fn derive(password:&[u8],salt:&[u8],rounds:u32)->[u8;32]{let mut block=[0u8;32];block[..salt.len()].copy_from_slice(salt);block[salt.len()+3]=1;let mut u=hmac(password,&block[..salt.len()+4]);let mut out=u;for _ in 1..rounds{u=hmac(password,&u);for i in 0..32{out[i]^=u[i];}}wipe(&mut u);out}
pub fn self_test()->bool{derive(b"password",b"salt",2)==[0xae,0x4d,0x0c,0x95,0xaf,0x6b,0x46,0xd3,0x2d,0x0a,0xdf,0xf9,0x28,0xf0,0x6d,0xd0,0x2a,0x30,0x3f,0x8e,0xf3,0xc2,0x51,0xdf,0xd6,0xe2,0xd8,0x5a,0x95,0x47,0x4c,0x43]}
impl Account{
    pub fn new(name:&[u8],host:&[u8],region:u8,dark:bool,password:&[u8])->Option<Self>{if !valid_name(name)||!valid_name(host)||region>1||!(12..=64).contains(&password.len()){return None;}let mut salt=[0;16];if !security::random(&mut salt)||salt==[0;16]{return None;}let mut n=[0;32];n[..name.len()].copy_from_slice(name);let mut h=[0;32];h[..host.len()].copy_from_slice(host);Some(Self{name:n,len:name.len()as u8,host:h,host_len:host.len()as u8,region,dark,salt,verifier:derive(password,&salt,ROUNDS)})}
    pub fn verify(&self,password:&[u8])->bool{if password.len()>64{return false;}let mut got=derive(password,&self.salt,ROUNDS);let mut diff=0;for i in 0..32{diff|=got[i]^self.verifier[i];}wipe(&mut got);diff==0}
    pub fn username(&self)->&str{core::str::from_utf8(&self.name[..self.len as usize]).unwrap_or("?")}
}
