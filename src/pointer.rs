//! Bounded VMware virtual pointer protocol, independent of PS/2 relative decoding.
// Protocol reference: Linux vmmouse and QEMU vmport/vmmouse documentation.
// Only pointer/version commands; no clipboard, host files or RPC.
use core::arch::{asm,x86_64::__cpuid};
use core::sync::atomic::{AtomicBool,Ordering};
const MAGIC:u32=0x564d5868;
const VERSION:u32=0x3442554a;
const DATA:u32=39;const STATUS:u32=40;const COMMAND:u32=41;
const ENABLE:u32=0x45414552;const ABSOLUTE:u32=0x53424152;const DISABLE:u32=0xf5;
static ACTIVE:AtomicBool=AtomicBool::new(false);
static DOWN:AtomicBool=AtomicBool::new(false);
#[derive(Clone,Copy)]pub struct Event{pub x:i64,pub y:i64,pub absolute:bool,pub pressed:bool,pub was:bool}
impl Event{
    pub fn position(self,x:usize,y:usize,w:usize,h:usize)->(usize,usize){
        if self.absolute{(scale(self.x,w),scale(self.y,h))}else{((x as i64+self.x).clamp(0,w.saturating_sub(1)as i64)as usize,(y as i64-self.y).clamp(0,h.saturating_sub(1)as i64)as usize)}
    }
}
fn scale(v:i64,extent:usize)->usize{((v.clamp(0,65535)as u64*extent.saturating_sub(1)as u64+32767)/65535)as usize}
fn decode(flags:u32,x:u32,y:u32,was:bool)->Option<Event>{
    if flags&0xffff0000==0xffff0000{return None;}
    let absolute=flags&0x10000==0;
    if absolute&&(x>65535||y>65535){return None;}
    Some(Event{x:if absolute{x as i64}else{x as i32 as i64},y:if absolute{y as i64}else{y as i32 as i64},absolute,pressed:flags&0x20!=0,was})
}
fn queue(status:u32)->Option<usize>{if status&0xffff0000==0xffff0000||status&3!=0{None}else{Some((status&0xffff)as usize/4)}}
fn supported()->bool{
    let basic=unsafe{__cpuid(1)};if basic.ecx&(1<<31)==0{return false;}
    let h=unsafe{__cpuid(0x40000000)};let b=unsafe{__cpuid(0)};
    // Explicit test CPU signature used only with QEMU vmport=on in CI.
    (h.ebx.to_le_bytes()==*b"VMwa"&&h.ecx.to_le_bytes()==*b"reVM"&&h.edx.to_le_bytes()==*b"ware")
    ||(b.ebx.to_le_bytes()==*b"Hi5A"&&b.edx.to_le_bytes()==*b"bsTe"&&b.ecx.to_le_bytes()==*b"stVM")
}
fn call(cmd:u32,arg:u32)->[u32;4]{
    let mut a=MAGIC;let mut b=arg;let mut c=cmd;let mut d=0x5658u32;
    // RBX is reserved by the Rust compiler; preserve it across the port ABI.
    unsafe{asm!("push rbx","mov ebx, {b:e}","in eax, dx","mov {b:e}, ebx","pop rbx",b=inout(reg)b,inout("eax")a,inout("ecx")c,inout("edx")d,inout("esi")0u32=>_,inout("edi")0u32=>_);}
    [a,b,c,d]
}
pub fn active()->bool{ACTIVE.load(Ordering::Relaxed)}
pub fn init()->bool{
    if !supported(){return false;}
    let v=call(10,!MAGIC);if v[1]!=MAGIC||v[0]==u32::MAX{return false;}
    call(COMMAND,ENABLE);let status=call(STATUS,0)[0];
    if status&0xffff0000==0xffff0000||status&0xffff!=1||call(DATA,1)[0]!=VERSION{call(COMMAND,DISABLE);return false;}
    call(COMMAND,ABSOLUTE);
    if queue(call(STATUS,0)[0]).is_none(){call(COMMAND,DISABLE);return false;}
    DOWN.store(false,Ordering::Relaxed);ACTIVE.store(true,Ordering::Relaxed);true
}
fn disable(){call(COMMAND,DISABLE);ACTIVE.store(false,Ordering::Relaxed);DOWN.store(false,Ordering::Relaxed);crate::serial::line("H5K: ABS_POINTER_FALLBACK");}
pub fn next()->Option<Event>{
    if !active(){return None;}
    match queue(call(STATUS,0)[0]){None=>{disable();None},Some(0)=>None,Some(_)=>{
        let p=call(DATA,4);let Some(e)=decode(p[0],p[1],p[2],DOWN.load(Ordering::Relaxed))else{disable();return None;};DOWN.store(e.pressed,Ordering::Relaxed);Some(e)
    }}
}
pub fn self_test()->bool{
    let a=decode(0x20,65535,0,false).unwrap();let b=decode(0,32768,32768,true).unwrap();let r=decode(0x10000,(-20i32)as u32,10,true).unwrap();
    a.position(5,5,640,480)==(639,0)&&a.pressed&&!a.was&&b.position(0,0,1280,720)==(640,360)&&!b.pressed&&b.was&&r.position(30,20,640,480)==(10,10)&&decode(0,65536,0,false).is_none()&&queue(0xffff0000).is_none()&&queue(3).is_none()&&queue(8)==Some(2)
}
