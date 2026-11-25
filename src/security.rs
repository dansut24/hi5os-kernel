//! Clock and hardware RNG foundations. No trusted time attestation or TLS yet.
use core::{arch::{asm,x86_64::__cpuid,x86_64::__cpuid_count},sync::atomic::{AtomicU64,AtomicU8,Ordering}};
#[repr(C)]
#[derive(Clone,Copy)]
pub struct EfiTime{pub year:u16,pub month:u8,pub day:u8,pub hour:u8,pub minute:u8,pub second:u8,pad1:u8,pub nanos:u32,pub zone:i16,pub daylight:u8,pad2:u8}
impl EfiTime{pub const fn empty()->Self{Self{year:0,month:0,day:0,hour:0,minute:0,second:0,pad1:0,nanos:0,zone:2047,daylight:0,pad2:0}}}
const _:()=assert!(core::mem::size_of::<EfiTime>()==16);
#[derive(Clone,Copy)]pub struct Clock{pub epoch:u64,pub explicit_utc:bool}
fn leap(y:u16)->bool{y%4==0&&(y%100!=0||y%400==0)}
fn days(y:u16,m:u8)->u8{match m{1|3|5|7|8|10|12=>31,4|6|9|11=>30,2=>if leap(y){29}else{28},_=>0}}
pub fn convert(t:EfiTime)->Option<Clock>{
    // Accept only UTC or unspecified zone (VM RTC assumed UTC), never guess a local offset.
    if !(1970..=2099).contains(&t.year)||t.day==0||t.day>days(t.year,t.month)||t.hour>23||t.minute>59||t.second>59||t.nanos>=1_000_000_000||!matches!(t.zone,0|2047)||t.daylight!=0{return None;}
    let mut d=0u64;for y in 1970..t.year{d+=if leap(y){366}else{365};}
    for m in 1..t.month{d+=days(t.year,m)as u64;}d+=u64::from(t.day-1);
    Some(Clock{epoch:d*86400+u64::from(t.hour)*3600+u64::from(t.minute)*60+u64::from(t.second),explicit_utc:t.zone==0})
}
static EPOCH:AtomicU64=AtomicU64::new(0);
static CLOCK_MODE:AtomicU8=AtomicU8::new(0);
static RNG_MODE:AtomicU8=AtomicU8::new(0);
pub fn clock_status()->u8{CLOCK_MODE.load(Ordering::Relaxed)}
pub fn entropy_status()->u8{RNG_MODE.load(Ordering::Relaxed)}
pub fn now()->Option<u64>{if clock_status()==0{None}else{EPOCH.load(Ordering::Relaxed).checked_add(crate::platform::ticks()/100)}}
fn features()->(bool,bool){
    unsafe{let max=__cpuid(0).eax;if max<1{return(false,false);}
        let rdrand=__cpuid(1).ecx&(1<<30)!=0;
        let rdseed=max>=7&&__cpuid_count(7,0).ebx&(1<<18)!=0;(rdseed,rdrand)}
}
fn word(mode:u8)->Option<u64>{
    if mode==0{return None;}for _ in 0..64{let v:u64;let ok:u8;
        unsafe{if mode==2{asm!("rdseed {v}","setc {ok}",v=lateout(reg)v,ok=lateout(reg_byte)ok,options(nomem,nostack));}
        else{asm!("rdrand {v}","setc {ok}",v=lateout(reg)v,ok=lateout(reg_byte)ok,options(nomem,nostack));}}
        if ok!=0{return Some(v);}core::hint::spin_loop();
    }None
}
fn fill_mode(mode:u8,out:&mut[u8])->bool{
    for part in out.chunks_mut(8){let Some(v)=word(mode)else{out.fill(0);return false;};part.copy_from_slice(&v.to_le_bytes()[..part.len()]);}true
}
pub fn random(out:&mut[u8])->bool{fill_mode(entropy_status(),out)}
fn test_clock()->bool{
    let mut t=EfiTime::empty();t.year=1970;t.month=1;t.day=1;t.zone=0;
    if convert(t).map(|v|v.epoch)!=Some(0){return false;}
    t.year=2000;t.month=2;t.day=29;
    if convert(t).map(|v|v.epoch)!=Some(951782400){return false;}
    t.year=2001;if convert(t).is_some(){return false;}
    t.year=2026;t.month=1;t.day=1;t.zone=60;if convert(t).is_some(){return false;}
    t.zone=2047;convert(t).map(|v|v.explicit_utc)==Some(false)
}
pub fn init(clock:Option<Clock>)->bool{
    if !test_clock(){return false;}
    if let Some(c)=clock{EPOCH.store(c.epoch,Ordering::Relaxed);CLOCK_MODE.store(if c.explicit_utc{2}else{1},Ordering::Relaxed);
        crate::serial::metric("H5K: CLOCK_EPOCH=",c.epoch);crate::serial::line(if c.explicit_utc{"H5K: CLOCK_UTC_REPORTED"}else{"H5K: CLOCK_UTC_ASSUMED"});
    }else{crate::serial::line("H5K: CLOCK_UNAVAILABLE");}
    let (seed,rand)=features();let mut a=[0;32];let mut b=[0;32];
    // Prefer RDSEED; fall back to RDRAND only if CPUID reports it.
    for mode in [2,1]{if (mode==2&&!seed)||(mode==1&&!rand){continue;}
        if fill_mode(mode,&mut a)&&fill_mode(mode,&mut b)&&a!=b&&a!=[0;32]&&b!=[0;32]{RNG_MODE.store(mode,Ordering::Relaxed);break;}
    }
    a.fill(0);b.fill(0);
    let mut sample=[0;16];let ok=random(&mut sample);sample.fill(0);
    if !ok{RNG_MODE.store(0,Ordering::Relaxed);}
    crate::serial::line(match entropy_status(){2=>"H5K: ENTROPY_RDSEED",1=>"H5K: ENTROPY_RDRAND",_=>"H5K: ENTROPY_UNAVAILABLE"});
    crate::serial::line("H5K: SECURITY_FOUNDATIONS_OK");true
}
pub fn status_text()->&'static str{match entropy_status(){2=>"RDSEED available",1=>"RDRAND available",_=>"CPU entropy unavailable"}}
