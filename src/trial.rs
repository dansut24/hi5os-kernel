//! Experimental single-CPU, pinned-binary PE startup probe. Not a general sandbox.
use core::{arch::{asm,global_asm,x86_64::__cpuid},ptr};
use crate::{platform,imports,relocate,serial,cpu};
const BASE:u64=0x1_0000_0000;
const AUX:u64=BASE+0x200000;
const ENV:u64=AUX+0x1000;
const STACK:u64=AUX+0x10000;
const TOP:u64=AUX+0x30000;
const HEAP:u64=AUX+0x40000;
const HEAP_END:u64=AUX+0x200000;
const NX:u64=1<<63;
const LIMIT:usize=2*1024*1024;
#[repr(align(4096))]struct Pages([u8;LIMIT]);
static mut IMAGE:Pages=Pages([0;LIMIT]);
static mut DATA:Pages=Pages([0;LIMIT]);
static mut SEEN:[u8;LIMIT/8]=[0;LIMIT/8];
static mut PT0:platform::Table=platform::Table([0;512]);
static mut PT1:platform::Table=platform::Table([0;512]);
#[repr(align(16))]struct Fp([u8;512]);
#[no_mangle]static mut H5_TRIAL_FX:Fp=Fp([0;512]);
#[no_mangle]static mut H5_TRIAL_ACTIVE:u64=0;
#[no_mangle]static mut H5_TRIAL_STATUS:u64=0;
#[no_mangle]static mut H5_TRIAL_BUDGET:u64=0;
#[no_mangle]static mut H5_TRIAL_RSP:u64=0;
#[no_mangle]static mut H5_TRIAL_FAULT:u64=0;
#[no_mangle]static mut H5_TRIAL_RIP:u64=0;
const PIN:&[u8;32]=include_bytes!("../target/hi5-fixture/7zip.sha256");
pub const BUNDLED:&[u8]=include_bytes!("../target/hi5-fixture/7za.exe");
#[derive(Clone,Copy)]struct Api{dll:[u8;48],dn:usize,name:[u8;128],nn:usize,kind:u8}
const EMPTY:Api=Api{dll:[0;48],dn:0,name:[0;128],nn:0,kind:0};
static mut APIS:[Api;256]=[EMPTY;256];
static mut N:usize=0;
static mut IMAGE_SIZE:usize=0;
static mut EXIT:u32=0;
static mut LAST:u32=0;
static mut APP_TYPE:u32=0;
static mut OUT:[u8;8192]=[0;8192];
static mut OUT_LEN:usize=0;
#[derive(Clone,Copy)]struct Block{at:u64,len:usize,used:bool}
const FREE:Block=Block{at:0,len:0,used:false};
static mut BLOCKS:[Block;128]=[FREE;128];
static mut NEXT:u64=HEAP;
static mut FS:crate::trialfs::Fs=crate::trialfs::Fs::new();
static mut MATH_HANDLER:u64=0;
static mut CALLS:u64=0;
static mut LAST_API:usize=usize::MAX;
static mut HELP_OK:bool=false;
static mut DETAIL:[u8;192]=[0;192];
static mut DETAIL_LEN:usize=0;
static mut ONEXIT:[u64;64]=[0;64];
static mut ONEXIT_LEN:usize=0;
#[derive(Clone,Copy)]struct Lock{at:u64,depth:u32,spin:u64}
const EMPTY_LOCK:Lock=Lock{at:0,depth:0,spin:0};
static mut LOCKS:[Lock;128]=[EMPTY_LOCK;128];
static mut CONSOLE_MODE:u32=3;
static mut CTRL_HANDLER:u64=0;
// Compatibility identity, not the native OS version or a claim of complete Win7 support.
const WIN_MAJOR:u32=6;const WIN_MINOR:u32=1;const WIN_BUILD:u32=7601;
const WIN_VERSION:u32=(WIN_BUILD<<16)|(WIN_MINOR<<8)|WIN_MAJOR;
static mut FILE_OEM:bool=false;
// Private process token handles; no host credentials or privileges.
#[derive(Clone,Copy)]struct Token{handle:u64,access:u32}
const EMPTY_TOKEN:Token=Token{handle:0,access:0};
static mut TOKENS:[Token;16]=[EMPTY_TOKEN;16];
static mut TOKEN_NEXT:u64=0x10000;
static mut CLEANUP_ACTIVE:bool=false;
static mut DLL_POLICY:u32=0;
const MODULES:[&str;6]=["kernel32.dll","msvcrt.dll","advapi32.dll","oleaut32.dll","user32.dll","ntdll.dll"];
const MOD_BASE:u64=0x60000000;
#[repr(C)]struct Frame{rax:u64,rcx:u64,rdx:u64,r8:u64,r9:u64,rip:u64,cs:u64,flags:u64,rsp:u64,ss:u64}
global_asm!(r#"
.text
.global h5_cleanup_start
.global h5_cleanup_end
h5_cleanup_start:
    push rbx
    push rsi
    push rdi
    push r12
    sub rsp,40
    mov rbx,rcx
    mov rsi,rdx
    mov rdi,r8
    mov r12,r9
h5_cleanup_loop:
    test rsi,rsi
    jz h5_cleanup_done
    dec rsi
    mov rax,[rbx+rsi*8]
    call rax
    jmp h5_cleanup_loop
h5_cleanup_done:
    mov ecx,r12d
    mov edx,edi
    mov eax,257
    int 0x81
    add rsp,40
    pop r12
    pop rdi
    pop rsi
    pop rbx
    ret
h5_cleanup_end:
.global h5_initterm_start
.global h5_initterm_end
h5_initterm_start:
    push rbx
    push rsi
    sub rsp,40
    mov rbx,rcx
    mov rsi,rdx
h5_initterm_loop:
    cmp rbx,rsi
    jae h5_initterm_done
    mov rax,[rbx]
    add rbx,8
    test rax,rax
    jz h5_initterm_loop
    call rax
    jmp h5_initterm_loop
h5_initterm_done:
    add rsp,40
    pop rsi
    pop rbx
    ret
h5_initterm_end:
.global h5_trial_enter
h5_trial_enter:
    pushfq
    cli
    push rbx
    push rbp
    push rsi
    push rdi
    push r12
    push r13
    push r14
    push r15
    fxsave64 [rip + H5_TRIAL_FX]
    mov qword ptr [rip + H5_TRIAL_RSP], rsp
    push 0x1b
    push rdx
    push 0x202
    push 0x23
    push rcx
    xor eax,eax
    xor ebx,ebx
    xor ecx,ecx
    xor edx,edx
    xor esi,esi
    xor edi,edi
    xor ebp,ebp
    xor r8d,r8d
    xor r9d,r9d
    xor r10d,r10d
    xor r11d,r11d
    xor r12d,r12d
    xor r13d,r13d
    xor r14d,r14d
    xor r15d,r15d
    iretq
.global h5_trial_return
h5_trial_return:
    cli
    cld
    mov rsp, qword ptr [rip + H5_TRIAL_RSP]
    fxrstor64 [rip + H5_TRIAL_FX]
    pop r15
    pop r14
    pop r13
    pop r12
    pop rdi
    pop rsi
    pop rbp
    pop rbx
    popfq
    ret
.global h5_trial_fault_error
h5_trial_fault_error:
    cmp qword ptr [rip + H5_TRIAL_ACTIVE],1
    jne h5_bad
    cmp qword ptr [rsp+16],0x23
    jne h5_bad
    mov rax,[rsp+8]
    mov [rip+H5_TRIAL_RIP],rax
    mov rax,cr2
    mov [rip+H5_TRIAL_FAULT],rax
    mov qword ptr [rip+H5_TRIAL_STATUS],2
    jmp h5_trial_return
.global h5_trial_fault_noerror
h5_trial_fault_noerror:
    cmp qword ptr [rip+H5_TRIAL_ACTIVE],1
    jne h5_bad
    cmp qword ptr [rsp+8],0x23
    jne h5_bad
    mov rax,[rsp]
    mov [rip+H5_TRIAL_RIP],rax
    mov qword ptr [rip+H5_TRIAL_STATUS],2
    jmp h5_trial_return
.global h5_trial_sys
h5_trial_sys:
    cmp qword ptr [rip+H5_TRIAL_ACTIVE],1
    jne h5_bad
    cmp qword ptr [rsp+8],0x23
    jne h5_bad
    cld
    push r9
    push r8
    push rdx
    push rcx
    push rax
    mov rcx,rsp
    sub rsp,32
    call h5_trial_dispatch
    add rsp,32
    cmp qword ptr [rip+H5_TRIAL_STATUS],0
    jne h5_trial_return
    pop rax
    pop rcx
    pop rdx
    pop r8
    pop r9
    iretq
"#);
extern "efiapi"{fn h5_cleanup_start();fn h5_cleanup_end();fn h5_initterm_start();fn h5_initterm_end();fn h5_trial_enter(ip:u64,sp:u64);fn h5_trial_sys();fn h5_trial_fault_error();fn h5_trial_fault_noerror();}
pub unsafe fn install_gates(){
    for v in [0,1,4,5,6,7,16,19]{platform::gate(v,h5_trial_fault_noerror as usize,0x8e);}
    for v in [10,11,12,13,17,21]{platform::gate(v,h5_trial_fault_error as usize,0x8e);}
    platform::gate(129,h5_trial_sys as usize,0xee);
}
unsafe fn msr(n:u32)->u64{let a:u32;let d:u32;asm!("rdmsr",in("ecx")n,out("eax")a,out("edx")d,options(nostack));(d as u64)<<32|a as u64}
unsafe fn set_msr(n:u32,v:u64){asm!("wrmsr",in("ecx")n,in("eax")v as u32,in("edx")(v>>32)as u32,options(nostack));}
unsafe fn nx_available()->bool{__cpuid(0x80000000).eax>=0x80000001&&__cpuid(0x80000001).edx&(1<<20)!=0}
unsafe fn flush(){let x=cpu::cr3();asm!("mov cr3, {}",in(reg)x,options(nostack,preserves_flags));}
fn u32at(b:&[u8],p:usize)->Option<u32>{Some(u32::from_le_bytes(b.get(p..p.checked_add(4)?)?.try_into().ok()?))}
fn u16at(b:&[u8],p:usize)->Option<u16>{Some(u16::from_le_bytes(b.get(p..p.checked_add(2)?)?.try_into().ok()?))}
unsafe fn phys(v:u64,n:usize,write:bool)->Option<*mut u8>{
    let end=v.checked_add(n as u64)?;if n==0{return None;}
    let (base,table,raw)=if v>=BASE&&end<=BASE+IMAGE_SIZE as u64{(BASE,ptr::addr_of!(PT0.0).cast::<u64>(),ptr::addr_of_mut!(IMAGE.0).cast::<u8>())}
        else if v>=AUX&&end<=AUX+LIMIT as u64{(AUX,ptr::addr_of!(PT1.0).cast::<u64>(),ptr::addr_of_mut!(DATA.0).cast::<u8>())}else{return None;};
    let start=(v-base)as usize;let last=(end-base-1)as usize;
    for i in start/4096..=last/4096{let e=table.add(i).read();if e&5!=5||write&&e&2==0{return None;}}
    Some(raw.add(start))
}
unsafe fn executable(v:u64)->bool{
    if v<BASE||v>=BASE+IMAGE_SIZE as u64{return false;}
    let i=((v-BASE)/4096)as usize;let p=ptr::addr_of!(PT0.0).cast::<u64>().add(i).read();p&5==5&&p&NX==0
}
unsafe fn write32(v:u64,x:u32)->bool{if let Some(p)=phys(v,4,true){ptr::write_unaligned(p.cast(),x);true}else{false}}
unsafe fn write64(v:u64,x:u64)->bool{if let Some(p)=phys(v,8,true){ptr::write_unaligned(p.cast(),x);true}else{false}}
unsafe fn c_len(v:u64,wide:bool)->Option<usize>{let step=if wide{2}else{1};for n in 0..65536usize{let p=phys(v.checked_add((n*step)as u64)?,step,false)?;if if wide{ptr::read_unaligned(p.cast::<u16>())==0}else{*p==0}{return Some(n);}}None}
unsafe fn allocate(n:usize)->u64{allocate_aligned(n,16)}
unsafe fn allocate_aligned(n:usize,align:u64)->u64{
    let Some(n)=n.max(1).checked_add(15).map(|x|x&!15)else{return 0;};
    let blocks=&mut *ptr::addr_of_mut!(BLOCKS);
    if let Some(b)=blocks.iter_mut().find(|b|!b.used&&b.at!=0&&b.len>=n&&b.at%align==0){b.used=true;return b.at;}
    let Some(b)=blocks.iter_mut().find(|b|b.at==0)else{return 0;};let Some(at)=NEXT.checked_add(align-1).map(|v|v&!(align-1))else{return 0;};
    if at.checked_add(n as u64).map_or(true,|x|x>HEAP_END){return 0;}
    NEXT=at+n as u64;*b=Block{at,len:n,used:true};at
}
unsafe fn release(v:u64)->bool{if v==0{return true;}if let Some(b)=(&mut *ptr::addr_of_mut!(BLOCKS)).iter_mut().find(|b|b.at==v&&b.used){b.used=false;true}else{false}}
unsafe fn detail(d:&[u8],n:&[u8]){
    let p=ptr::addr_of_mut!(DETAIL).cast::<u8>();let a=d.len().min(48);let b=n.len().min(128);
    ptr::copy_nonoverlapping(d.as_ptr(),p,a);p.add(a).write(b'!');ptr::copy_nonoverlapping(n.as_ptr(),p.add(a+1),b);DETAIL_LEN=a+1+b;
}
// Windows processor-feature IDs; only advertise features enabled in this trial.
fn processor_feature(id:u32)->bool{unsafe{
    let a=__cpuid(1);
    let ext=__cpuid(0x80000000).eax;
    let cr4:u64;asm!("mov {},cr4",out(reg)cr4,options(nostack,preserves_flags));
    let sse=cr4&(1<<9)!=0;
    match id{
        2=>a.edx&(1<<8)!=0,3=>a.edx&(1<<23)!=0,6=>sse&&a.edx&(1<<25)!=0,
        7=>ext>=0x80000001&&__cpuid(0x80000001).edx&(1<<31)!=0,
        8=>a.edx&(1<<4)!=0,9=>true,10=>sse&&a.edx&(1<<26)!=0,
        12=>nx_available(),13=>sse&&a.ecx&1!=0,14=>a.ecx&(1<<13)!=0,
        36=>sse&&a.ecx&(1<<9)!=0,37=>sse&&a.ecx&(1<<19)!=0,38=>sse&&a.ecx&(1<<20)!=0,
        // XSAVE/AVX, FSGSBASE, fast-fail ABI, ARM, unknown IDs: not enabled/supported.
        _=>false
    }
}}
unsafe fn lock_sync(v:Lock)->bool{
    let Some(p)=phys(v.at,40,true)else{return false;};
    ptr::write_bytes(p,0,40);
    ptr::write_unaligned(p.add(8).cast::<i32>(),v.depth as i32-1);
    ptr::write_unaligned(p.add(12).cast::<u32>(),v.depth);
    ptr::write_unaligned(p.add(16).cast::<u64>(),if v.depth!=0{1}else{0});
    ptr::write_unaligned(p.add(32).cast::<u64>(),v.spin);true
}
unsafe fn lock_op(v:u64,op:u8,spin:u64)->bool{
    if v==0||v%8!=0||phys(v,40,true).is_none(){return false;}
    let ls=&mut *ptr::addr_of_mut!(LOCKS);
    if op==0{
        if ls.iter().any(|l|l.at==v){return false;}
        let Some(l)=ls.iter_mut().find(|l|l.at==0)else{return false;};
        *l=Lock{at:v,depth:0,spin};return lock_sync(*l);
    }
    let Some(l)=ls.iter_mut().find(|l|l.at==v)else{return false;};
    match op{
        1=>{if l.depth>=0x7ffffffe{return false;}l.depth+=1;lock_sync(*l)},
        2=>{if l.depth==0{return false;}l.depth-=1;lock_sync(*l)},
        3=>{if l.depth!=0{return false;}let p=phys(v,40,true).unwrap();ptr::write_bytes(p,0,40);*l=EMPTY_LOCK;true},
        _=>false
    }
}
unsafe fn version_info(v:u64,wide:bool)->bool{
    let Some(p)=phys(v,4,false)else{return false;};
    let len=ptr::read_unaligned(p.cast::<u32>())as usize;
    let basic=if wide{276}else{148};if len!=basic&&len!=basic+8{return false;}
    let Some(p)=phys(v,len,true)else{return false;};
    ptr::write_bytes(p,0,len);for(i,n)in [len as u32,WIN_MAJOR,WIN_MINOR,WIN_BUILD,2].iter().enumerate(){ptr::write_unaligned(p.add(4*i).cast::<u32>(),*n);}
    if len==basic+8{p.add(basic+6).write(1);} // workstation, no claimed service pack
    true
}
unsafe fn mb_to_wide(f:&mut Frame,x:u64,y:u64,z:u64,w:u64){
    let cp=x as u32;let flags=y as u32;
    // ASCII subset of ACP/OEM and full UTF-8; reject unsupported legacy non-ASCII rather than corrupt text.
    if !matches!(cp,0|1|437|1252|65001)||flags&!8!=0{LAST=87;return;}
    let count=w as u32 as i32;
    let n=if count==-1{match c_len(z,false){Some(n)=>n+1,None=>{LAST=87;return;}}}else if count>0{count as usize}else{LAST=87;return;};
    if n>65536{LAST=87;return;}let Some(p)=phys(z,n,false)else{LAST=87;return;};let bytes=core::slice::from_raw_parts(p,n);
    if cp!=65001&&!bytes.is_ascii(){LAST=1113;return;}
    let Ok(s)=core::str::from_utf8(bytes)else{LAST=1113;return;};
    let need=s.encode_utf16().count();
    let Some(dst)=arg(f,40)else{LAST=87;return;};let Some(cap)=arg(f,48).map(|v|v as u32 as i32)else{LAST=87;return;};
    if cap==0{f.rax=need as u64;return;}if cap<0||(cap as usize)<need{LAST=122;return;}
    let Some(q)=phys(dst,need*2,true)else{LAST=87;return;};
    // Reject overlapping user buffers, avoiding aliasing a Rust source slice.
    if dst<z+n as u64&&z<dst+(need*2)as u64{LAST=87;return;}
    for(i,c)in s.encode_utf16().enumerate(){ptr::write_unaligned(q.add(i*2).cast::<u16>(),c);}f.rax=need as u64;
}
unsafe fn wide_to_mb(f:&mut Frame,x:u64,y:u64,z:u64,w:u64){
    let cp=x as u32;let flags=y as u32;if !matches!(cp,0|1|437|1252|65001)||flags&!0x480!=0{LAST=87;return;}
    let count=w as u32 as i32;let n=if count==-1{match c_len(z,true){Some(n)=>n+1,None=>{LAST=87;return;}}}else if count>0{count as usize}else{LAST=87;return;};
    if n>65536{LAST=87;return;}let Some(p)=phys(z,n*2,false)else{LAST=87;return;};
    let src=||core::char::decode_utf16((0..n).map(|i|ptr::read_unaligned(p.add(i*2).cast::<u16>())));
    let mut need=0usize;for c in src(){let Ok(c)=c else{LAST=1113;return;};if cp!=65001&&!c.is_ascii(){LAST=1113;return;}need+=c.len_utf8();}
    let Some(dst)=arg(f,40)else{LAST=87;return;};let Some(cap)=arg(f,48).map(|v|v as u32 as i32)else{LAST=87;return;};
    let default=arg(f,56);let used=arg(f,64);
    if default!=Some(0)||used.is_none()||cp==65001&&used!=Some(0){LAST=87;return;}
    if let Some(u)=used{if u!=0&&phys(u,4,true).is_none(){LAST=87;return;}}
    if cap==0{f.rax=need as u64;return;}if cap<0||(cap as usize)<need{LAST=122;return;}
    let Some(q)=phys(dst,need,true)else{LAST=87;return;};
    if dst<z+(n*2)as u64&&z<dst+need as u64{LAST=87;return;}
    let mut i=0;for c in src(){let mut b=[0u8;4];let s=c.unwrap().encode_utf8(&mut b);ptr::copy_nonoverlapping(s.as_ptr(),q.add(i),s.len());i+=s.len();}
    if let Some(u)=used{if u!=0{write32(u,0);}}f.rax=need as u64;
}
unsafe fn token_access(h:u64)->Option<u32>{
    if h==0{return None;}
    (&*ptr::addr_of!(TOKENS)).iter().find(|v|v.handle==h).map(|v|v.access)
}
unsafe fn token_open(process:u64,access:u32,out:u64)->Result<(),u32>{
    if process!=u64::MAX{return Err(6);}
    let Some(p)=phys(out,8,true)else{return Err(87);};
    // TOKEN_QUERY, TOKEN_ADJUST_PRIVILEGES and READ_CONTROL only.
    if access&!0x20028!=0{return Err(5);}
    let Some(slot)=(&mut *ptr::addr_of_mut!(TOKENS)).iter_mut().find(|v|v.handle==0)else{return Err(4);};
    let h=TOKEN_NEXT;TOKEN_NEXT=TOKEN_NEXT.checked_add(1).ok_or(4u32)?;
    *slot=Token{handle:h,access};ptr::write_unaligned(p.cast::<u64>(),h);Ok(())
}
unsafe fn token_close(h:u64)->bool{
    if h==0{return false;}
    if let Some(slot)=(&mut *ptr::addr_of_mut!(TOKENS)).iter_mut().find(|v|v.handle==h){*slot=EMPTY_TOKEN;true}else{false}
}
unsafe fn privilege_lookup(system:u64,name:u64,out:u64)->Result<(),u32>{
    if system!=0{return Err(1355);}
    let mut b=[0u8;96];let n=read_ascii(name,true,&mut b).ok_or(87u32)?;
    // Stable local identifiers only. No privileges are assigned to the token.
    let names=["SeCreateTokenPrivilege","SeAssignPrimaryTokenPrivilege","SeLockMemoryPrivilege","SeIncreaseQuotaPrivilege","SeMachineAccountPrivilege","SeTcbPrivilege","SeSecurityPrivilege","SeTakeOwnershipPrivilege","SeLoadDriverPrivilege","SeSystemProfilePrivilege","SeSystemtimePrivilege","SeProfileSingleProcessPrivilege","SeIncreaseBasePriorityPrivilege","SeCreatePagefilePrivilege","SeCreatePermanentPrivilege","SeBackupPrivilege","SeRestorePrivilege","SeShutdownPrivilege","SeDebugPrivilege","SeAuditPrivilege","SeSystemEnvironmentPrivilege","SeChangeNotifyPrivilege","SeRemoteShutdownPrivilege","SeUndockPrivilege","SeSyncAgentPrivilege","SeEnableDelegationPrivilege","SeManageVolumePrivilege","SeImpersonatePrivilege","SeCreateGlobalPrivilege","SeTrustedCredManAccessPrivilege","SeRelabelPrivilege","SeIncreaseWorkingSetPrivilege","SeTimeZonePrivilege","SeCreateSymbolicLinkPrivilege"];
    let id=names.iter().position(|s|b[..n].eq_ignore_ascii_case(s.as_bytes())).ok_or(1313u32)?+2;
    if !write64(out,id as u64){return Err(87);}Ok(())
}
unsafe fn adjust_privileges(h:u64,disable:bool,state:u64,cap:u32,prev:u64,ret:u64)->Result<u32,u32>{
    let access=token_access(h).ok_or(6u32)?;
    if access&0x20==0||prev!=0&&access&8==0{return Err(5);}
    let count=if disable{0}else{
        let p=phys(state,4,false).ok_or(87u32)?;let n=ptr::read_unaligned(p.cast::<u32>())as usize;
        if n>64||phys(state,4+12*n,false).is_none(){return Err(87);}n
    };
    if prev!=0{
        if ret==0||phys(ret,4,true).is_none(){return Err(87);}
        if cap<4{write32(ret,4);return Err(122);}
        if phys(prev,4,true).is_none(){return Err(87);}
        write32(prev,0);write32(ret,4);
    }
    // Success return with ERROR_NOT_ALL_ASSIGNED is the documented result for absent privileges.
    Ok(if count==0{0}else{1300})
}
unsafe fn begin_cleanup(f:&mut Frame,exit:bool,code:u32)->bool{
    if CLEANUP_ACTIVE{return false;}
    let count=ONEXIT_LEN;
    if count==0{if exit{EXIT=code;H5_TRIAL_STATUS=1;}return true;}
    let addr=ENV+0xa00;
    let Some(p)=phys(addr,count*8,true)else{return false;};
    for i in 0..count{let cb=ptr::addr_of!(ONEXIT).cast::<u64>().add(i).read();if !executable(cb){return false;}ptr::write_unaligned(p.add(i*8).cast::<u64>(),cb);}
    ONEXIT_LEN=0;CLEANUP_ACTIVE=true;
    f.rip=AUX+0xd00;f.rcx=addr;f.rdx=count as u64;f.r8=exit as u64;f.r9=code as u64;true
}
unsafe fn abort_import(i:usize){let a=ptr::addr_of!(APIS).cast::<Api>().add(i).read();detail(&a.dll[..a.dn],&a.name[..a.nn]);serial::write("H5K: TRIAL_UNSUPPORTED|");serial::write(core::str::from_utf8(&a.dll[..a.dn]).unwrap_or("?"));serial::write("|");serial::line(core::str::from_utf8(&a.name[..a.nn]).unwrap_or("?"));H5_TRIAL_STATUS=4;}
pub fn diagnostic(out:&mut[u8])->usize{unsafe{let n=DETAIL_LEN.min(out.len());ptr::copy_nonoverlapping(ptr::addr_of!(DETAIL).cast::<u8>(),out.as_mut_ptr(),n);n}}
unsafe fn read_ascii(v:u64,wide:bool,out:&mut[u8])->Option<usize>{
    let step=if wide{2}else{1};for (i,c) in out.iter_mut().enumerate(){
        let p=phys(v.checked_add((i*step)as u64)?,step,false)?;
        let n=if wide{ptr::read_unaligned(p.cast::<u16>())}else{*p as u16};
        if n==0{return Some(i);}if !(32..127).contains(&n){return None;}*c=n as u8;
    }None
}
fn module_id(s:&[u8])->Option<usize>{
    MODULES.iter().position(|m|s.eq_ignore_ascii_case(m.as_bytes())||s.eq_ignore_ascii_case(m.as_bytes().strip_suffix(b".dll").unwrap()))
}
unsafe fn module_handle(v:u64,wide:bool)->u64{
    if v==0{return BASE;}let mut b=[0u8;128];let Some(n)=read_ascii(v,wide,&mut b)else{LAST=87;return 0;};
    let s=&b[..n];if s.eq_ignore_ascii_case(b"7za.exe"){return BASE;}
    if let Some(i)=module_id(s){return MOD_BASE+0x10000*i as u64;}
    LAST=126;serial::write("H5K: TRIAL_MODULE_MISSING|");serial::line(core::str::from_utf8(s).unwrap_or("?"));0
}
unsafe fn proc_address(h:u64,v:u64)->u64{
    let Some(i)=h.checked_sub(MOD_BASE).filter(|n|n%0x10000==0).map(|n|(n/0x10000)as usize).filter(|i|*i<MODULES.len())else{LAST=6;return 0;};
    let dll=MODULES[i];let mut b=[0u8;128];let Some(n)=read_ascii(v,false,&mut b)else{LAST=127;return 0;};
    let name=core::str::from_utf8(&b[..n]).unwrap_or("");
    serial::write("H5K: TRIAL_DYNAMIC|");serial::write(dll);serial::write("|");serial::line(name);
    // Return only implemented exports, never a successful result for an unknown API.
    for j in 0..N{let a=ptr::addr_of!(APIS).cast::<Api>().add(j).read();
        if a.dll[..a.dn].eq_ignore_ascii_case(dll.as_bytes())&&a.name[..a.nn]==b[..n]&&a.kind!=0{return AUX+j as u64*16;}
    }
    let k=kind(dll,name);if k==0||N>=208{LAST=127;return 0;}
    let mut a=EMPTY;a.dn=dll.len();a.dll[..a.dn].copy_from_slice(dll.as_bytes());a.nn=n;a.name[..n].copy_from_slice(&b[..n]);a.kind=k;
    let j=N;N+=1;ptr::addr_of_mut!(APIS).cast::<Api>().add(j).write(a);
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();let stub=[0xb8,j as u8,0,0,0,0xcd,0x81,0xc3];
    ptr::copy_nonoverlapping(stub.as_ptr(),d.add(j*16),stub.len());AUX+j as u64*16
}
unsafe fn capture(v:u64,n:usize)->bool{
    if n==0{return true;}if n>8192-OUT_LEN{return false;}let Some(p)=phys(v,n,false)else{return false;};
    ptr::copy_nonoverlapping(p,ptr::addr_of_mut!(OUT).cast::<u8>().add(OUT_LEN),n);OUT_LEN+=n;true
}
#[no_mangle]unsafe extern "efiapi" fn h5_trial_dispatch(f:*mut Frame){
    let f=&mut *f;let id=f.rax as usize;if id==256{EXIT=f.rcx as u32;H5_TRIAL_STATUS=1;return;}
    if id==257{if !CLEANUP_ACTIVE||ONEXIT_LEN!=0{H5_TRIAL_STATUS=4;return;}CLEANUP_ACTIVE=false;if f.rdx==1{EXIT=f.rcx as u32;H5_TRIAL_STATUS=1;}else{f.rax=0;}return;}if id>=N{H5_TRIAL_STATUS=4;return;}
    CALLS+=1;LAST_API=id;
    let a=ptr::addr_of!(APIS).cast::<Api>().add(id).read();
    if CALLS<=160{serial::write("H5K: TRIAL_CALL|");serial::write(core::str::from_utf8(&a.dll[..a.dn]).unwrap_or("?"));serial::write("|");serial::line(core::str::from_utf8(&a.name[..a.nn]).unwrap_or("?"));}
    let (x,y,z,w)=(f.rcx,f.rdx,f.r8,f.r9);f.rax=0;
    match a.kind{
        1=>{if &a.name[..a.nn]==b"exit"{if !begin_cleanup(f,true,x as u32){abort_import(id);}}else{EXIT=x as u32;H5_TRIAL_STATUS=1;}},
        2=>f.rax=1,
        3=>LAST=x as u32,
        4=>f.rax=LAST as u64,
        5=>f.rax=ENV+0x400,
        6=>f.rax=match x as u32{0xfffffff5=>1,0xfffffff4=>2,_=>u64::MAX},
        7=>{if x>=0x100{read_write(f,true,x,y,z,w);return;}let sync=f.rsp.checked_add(40).and_then(|v|phys(v,8,false)).map(|p|ptr::read_unaligned(p.cast::<u64>())==0).unwrap_or(false);
            if matches!(x,1|2)&&sync&&phys(w,4,true).is_some()&&capture(y,z as u32 as usize){write32(w,z as u32);f.rax=1;}else{LAST=87;}},
        8=>f.rax=allocate(x as usize),
        9=>{if !release(x){abort_import(id);}},
        10=>{if z<=LIMIT as u64{if z==0{f.rax=x;}else if let(Some(d),Some(s))=(phys(x,z as usize,true),phys(y,z as usize,false)){ptr::copy(s,d,z as usize);f.rax=x;}else{abort_import(id);}}else{abort_import(id);}},
        11=>{if z<=LIMIT as u64{if z==0{f.rax=x;}else if let Some(d)=phys(x,z as usize,true){ptr::write_bytes(d,y as u8,z as usize);f.rax=x;}else{abort_import(id);}}else{abort_import(id);}},
        12=>{if let Some(n)=c_len(x,false){f.rax=n as u64;}else{abort_import(id);}},
        13=>{if matches!(x,1|2){APP_TYPE=x as u32;}else{abort_import(id);}},
        14=>{if phys(x,4,true).is_some()&&phys(y,8,true).is_some()&&phys(z,8,true).is_some(){write32(x,2);write64(y,ENV+0x600);write64(z,ENV+0x620);f.rax=0;}else{abort_import(id);}},
        15=>f.rax=platform::ticks().wrapping_mul(10)as u32 as u64,
        16=>f.rax=u64::MAX,
        17=>f.rax=module_handle(x,&a.name[..a.nn]==b"GetModuleHandleW"),
        18=>{if z<=LIMIT as u64{if z==0{return;}if let (Some(a),Some(b))=(phys(x,z as usize,false),phys(y,z as usize,false)){for i in 0..z as usize{let d=*a.add(i)as i32-*b.add(i)as i32;if d!=0{f.rax=d as u32 as u64;break;}}}else{abort_import(id);}}else{abort_import(id);}},
        19=>f.rax=realloc(x,y as usize),
        20|21=>{if let Some(d)=text_cmp(x,y,a.kind==21){f.rax=d as u32 as u64;}else{abort_import(id);}},
        22=>{if x==0||phys(x,1,false).is_some(){MATH_HANDLER=x;}else{abort_import(id);}},
        23=>f.rax=if x as u32<=2{1}else{0},
        24=>f.rax=match x as u32{1=>1,2=>2,_=>u64::MAX},
        25=>{let mode=arg(f,40);let flags=arg(f,48);let template=arg(f,56);
            if w!=0||template!=Some(0)||!matches!(flags,Some(0|0x80))||mode.is_none(){LAST=87;f.rax=u64::MAX;return;}
            let Some(p)=path(x)else{LAST=123;f.rax=u64::MAX;return;};
            match fs().open(p,y as u32,z as u32,mode.unwrap()as u32){Ok((h,e))=>{f.rax=h;LAST=e},Err(e)=>{LAST=e;f.rax=u64::MAX;}}},
        26=>read_write(f,false,x,y,z,w),
        27=>{if token_close(x){f.rax=1;}else{let r=fs().close(x);bool_result(f,r);}},
        28=>{if y!=0&&!write32(y,0){LAST=87;f.rax=u32::MAX as u64;return;}match fs().size(x){Ok(v)=>f.rax=v as u64,Err(e)=>{LAST=e;f.rax=u32::MAX as u64;}}},
        29=>{if z!=0{LAST=87;f.rax=u32::MAX as u64;return;}match fs().seek(x,y as u32 as i32 as i64,w as u32){Ok(v)=>f.rax=v as u64,Err(e)=>{LAST=e;f.rax=u32::MAX as u64;}}},
        30=>{let r=fs().truncate(x);bool_result(f,r);},
        31|32|33=>{let Some(p)=path(x)else{LAST=123;return;};let r=match a.kind{31 if y==0=>fs().mkdir(p),31=>Err(87),32=>fs().rmdir(p),_=>fs().delete(p)};bool_result(f,r);},
        34=>{let r=path(x).ok_or(123u32).and_then(|p|fs().attrs(p));match r{Ok(v)=>f.rax=v as u64,Err(e)=>{LAST=e;f.rax=u32::MAX as u64;}}},
        35=>{if x<4{f.rax=4;}else if let Some(p)=phys(y,8,true){for(i,v)in [67u16,58,92,0].iter().enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),*v);}f.rax=3;}else{LAST=87;}},
        36=>{let stdout=ENV+0x700+48;let stderr=ENV+0x700+96;if y!=stdout&&y!=stderr{abort_import(id);return;}let Some(n)=c_len(x,false)else{abort_import(id);return;};if capture(x,n){f.rax=0;}else{f.rax=u32::MAX as u64;}},
        37=>{let stdout=ENV+0x700+48;let stderr=ENV+0x700+96;if y!=stdout&&y!=stderr{abort_import(id);return;}if OUT_LEN<8192{ptr::addr_of_mut!(OUT).cast::<u8>().add(OUT_LEN).write(x as u8);OUT_LEN+=1;f.rax=x as u8 as u64;}else{f.rax=u32::MAX as u64;}},
        38=>{if x!=0&&x!=ENV+0x700+48&&x!=ENV+0x700+96{abort_import(id);}},
        39=>f.rax=0, // GetLargePageMinimum: large pages not supported
        40=>f.rax=437, // narrow OEM code page
        41=>{if !matches!(x,1|2)||!write32(y,CONSOLE_MODE){LAST=87;}else{f.rax=1;}},
        42=>{if matches!(x,1|2)&&y<=3{CONSOLE_MODE=y as u32;f.rax=1;}else{LAST=87;}},
        43=>{if x==0{LAST=87;}else{f.rax=module_handle(x,true);}}, // built-in namespace only
        44=>{if !write64(x,100){LAST=87;}else{f.rax=1;}},
        45=>{if !write64(x,platform::ticks()){LAST=87;}else{f.rax=1;}},
        46=>{if x!=u64::MAX||!write64(y,1)||!write64(z,1){LAST=87;}else{f.rax=1;}},
        47=>{if x!=u64::MAX||y!=1{LAST=87;}else{f.rax=1;}},
        48=>{if x!=0{let Some(p)=phys(x,48,true)else{LAST=87;return;};ptr::write_bytes(p,0,48);ptr::write_unaligned(p.cast::<u16>(),9);ptr::write_unaligned(p.add(4).cast::<u32>(),4096);ptr::write_unaligned(p.add(8).cast::<u64>(),BASE);ptr::write_unaligned(p.add(16).cast::<u64>(),AUX+LIMIT as u64-1);ptr::write_unaligned(p.add(24).cast::<u64>(),1);ptr::write_unaligned(p.add(32).cast::<u32>(),1);ptr::write_unaligned(p.add(36).cast::<u32>(),8664);ptr::write_unaligned(p.add(40).cast::<u32>(),65536);}else{abort_import(id);}},
        49=>f.rax=proc_address(x,y),
        50=>{if MODULES.iter().enumerate().any(|(i,_)|x==MOD_BASE+0x10000*i as u64){f.rax=1;}else{LAST=6;}},
        51=>f.rax=platform::ticks().wrapping_mul(10),
        52=>{let path=b"C:\\7za.exe";let n=path.len();if x!=0&&x!=BASE{LAST=6;return;}let cap=z as u32 as usize;if cap==0{LAST=122;return;}let m=n.min(cap-1);
            if let Some(p)=phys(y,(m+1)*2,true){for(i,c)in path[..m].iter().enumerate(){ptr::write_unaligned(p.add(2*i).cast::<u16>(),*c as u16);}ptr::write_unaligned(p.add(m*2).cast::<u16>(),0);f.rax=if m<n{LAST=122;cap as u64}else{m as u64};}else{LAST=87;}},
        53=>{let Some(sec)=crate::security::now()else{abort_import(id);return;};let ft=sec.checked_add(11644473600).and_then(|v|v.checked_mul(10000000));if ft.is_none()||!write64(x,ft.unwrap()){abort_import(id);}},
        54=>f.rax=1, // GetActiveProcessorCount: one virtual CPU
        55=>{if x==u64::MAX&&write32(y,0){f.rax=1;}else{LAST=87;}},
        56=>{if x==0||ONEXIT_LEN>=64||!executable(x){f.rax=0;}else{ptr::addr_of_mut!(ONEXIT).cast::<u64>().add(ONEXIT_LEN).write(x);ONEXIT_LEN+=1;f.rax=x;}},
        57=>{if &a.name[..a.nn]==b"_cexit"&&!begin_cleanup(f,false,0){abort_import(id);}}, // _c_exit does not run callbacks
        58=>{if x==0&&y!=0&&(z as u32&!0x3000)==0&&z as u32&0x1000!=0&&w==4{let n=(y as usize).checked_add(4095).map(|v|v&!4095);if let Some(n)=n{f.rax=allocate_aligned(n,4096);if f.rax!=0{ptr::write_bytes(phys(f.rax,n,true).unwrap(),0,n);}}}if f.rax==0{LAST=8;}},
        59=>{if y==0&&z==0x8000&&x%4096==0&&release(x){f.rax=1;}else{LAST=87;}},
        60=>{f.rax=processor_feature(x as u32)as u64;serial::metric("H5K: TRIAL_CPU_FEATURE=",x as u32 as u64);serial::metric("H5K: TRIAL_CPU_FEATURE_VALUE=",f.rax);},
        61|62|63|64=>{if !lock_op(x,a.kind-61,0){abort_import(id);}},
        65=>{if lock_op(x,0,y as u32 as u64){f.rax=1;}else{LAST=87;}},
        66=>{f.rax=lock_op(x,1,0)as u64;},
        67=>{if z==0&&lock_op(x,0,y as u32 as u64){f.rax=1;}else{LAST=87;}},
        68=>{if !matches!(x,1|2){LAST=6;return;}let Some(p)=phys(y,22,true)else{LAST=87;return;};
            // Virtual text console, 80x25, cursor from captured printable output.
            let bytes=core::slice::from_raw_parts(ptr::addr_of!(OUT).cast::<u8>(),OUT_LEN);
            let mut row=0u16;let mut col=0u16;for c in bytes{match *c{10=>{row=row.saturating_add(1);col=0;},13=>col=0,32..=126=>{col+=1;if col==80{row=row.saturating_add(1);col=0;}},_=>{}}}
            let fields=[80u16,25,col,row.min(24),7,0,0,79,24,80,25];
            for(i,v)in fields.iter().enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),*v);}f.rax=1;},
        69=>{if y==1&&(x==0||executable(x)){CTRL_HANDLER=x;f.rax=1;}else if y==0&&x==CTRL_HANDLER{CTRL_HANDLER=0;f.rax=1;}else{LAST=87;}},
        70=>{if let (Some(nx),Some(ny))=(c_len(x,true),c_len(y,true)){if ny==0{f.rax=x;}else if ny<=nx{for i in 0..=nx-ny{let at=x+2*i as u64;let n=ny*2;if let(Some(a),Some(b))=(phys(at,n,false),phys(y,n,false)){if core::slice::from_raw_parts(a,n)==core::slice::from_raw_parts(b,n){f.rax=at;break;}}}}}else{abort_import(id);}},
        71=>{if x==0{f.rax=0;}else if matches!(x,1|2){f.rax=2;}}, // FILE_TYPE_CHAR for virtual output, unknown otherwise
        72=>{if x==u64::MAX&&y==1{f.rax=1;}else{LAST=87;}}, // single process affinity only
        73=>{f.rax=WIN_VERSION as u64;serial::metric("H5K: TRIAL_COMPAT_VERSION=",f.rax);},
        74=>{if version_info(x,&a.name[..a.nn]!=b"GetVersionExA"){f.rax=1;}else{LAST=87;}},
        75=>{f.rax=if version_info(x,true){0}else{0xc000000d};},
        76=>{FILE_OEM=true;},
        77=>{FILE_OEM=false;},
        78=>f.rax=(!FILE_OEM)as u64,
        79=>f.rax=1252,
        80=>mb_to_wide(f,x,y,z,w),
        81=>wide_to_mb(f,x,y,z,w),
        82=>{let n=match x as u32{0=>0,0xc0000008=>6,0xc000000d=>87,0xc000000f=>2,0xc0000022=>5,0xc0000034=>2,0xc000003a=>3,0xc0000043=>32,0xc000007f=>112,0xc00000bb=>50,_=>317};f.rax=n;},
        83=>{ // GetLogicalDriveStringsW: private C: namespace only
            if x<5{f.rax=5;}else if let Some(p)=phys(y,10,true){for(i,v)in [67u16,58,92,0,0].iter().enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),*v);}f.rax=4;}else{LAST=87;}
        },
        84=>{ // GetTempPathW, root of private volatile namespace
            if x<4{f.rax=4;}else if let Some(p)=phys(y,8,true){for(i,v)in [67u16,58,92,0].iter().enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),*v);}f.rax=3;}else{LAST=87;}
        },
        85=>{ // LocalFree only accepts pointers from our allocator
            if release(x){f.rax=0;}else{LAST=6;f.rax=x;}
        },
        86=>{ // GetConsoleOutputCP: UTF-8 output capture
            f.rax=65001;
        },
        87=>{match token_open(x,y as u32,z){Ok(())=>f.rax=1,Err(e)=>LAST=e}},
        88=>{match privilege_lookup(x,y,z){Ok(())=>f.rax=1,Err(e)=>LAST=e}},
        89=>{let (Some(prev),Some(ret))=(arg(f,40),arg(f,48))else{LAST=87;return;};
            match adjust_privileges(x,y as u32!=0,z,w as u32,prev,ret){Ok(e)=>{LAST=e;f.rax=1;},Err(e)=>LAST=e}},
        90=>{ // SetDefaultDllDirectories: only built-in modules exist; no current directory loading.
            let flags=x as u32;if flags!=0&&flags&!0x1e00==0{DLL_POLICY=flags;f.rax=1;}else{LAST=87;}},
        91=>{ // RtlGenRandom: use actual CPU entropy or fail closed.
            let n=y as u32 as usize;if n==0{f.rax=1;}else if n<=65536{if let Some(p)=phys(x,n,true){let out=core::slice::from_raw_parts_mut(p,n);if crate::security::random(out){f.rax=1;}else{out.fill(0);}}}},
        92=>{ // RegOpenKeyExW: no registry keys exist in this private namespace.
            let root=x as u32;let Some(out)=arg(f,40)else{f.rax=87;return;};
            if !(0x80000000..=0x80000005).contains(&root)||x!=root as u64&&x!=(root as i32 as i64)as u64{f.rax=6;}
            else if z!=0||read_ascii(y,true,&mut [0u8;256]).is_none()||!write64(out,0){f.rax=87;}else{f.rax=2;}},
        93=>f.rax=6, // RegQueryValueExW / RegCloseKey: no opened registry handle
        94=>{ // No raw file security descriptors in this volatile namespace.
            LAST=50;f.rax=0;},
        95=>{ // GetTokenInformation(TokenPrivileges / TokenElevation)
            let Some(access)=token_access(x)else{LAST=6;return;};
            if access&8==0{LAST=5;return;}
            if !matches!(y as u32,3|20){LAST=87;return;}
            let Some(ret)=arg(f,40)else{LAST=87;return;};if !write32(ret,4){LAST=87;return;}
            if w<4{LAST=122;return;}if write32(z,0){f.rax=1;}else{LAST=87;}},
        _=>abort_import(id)
    }
}
unsafe fn fs()->&'static mut crate::trialfs::Fs{&mut *ptr::addr_of_mut!(FS)}
unsafe fn path(v:u64)->Option<crate::trialfs::Path>{
    let n=c_len(v,true)?;if n>96{return None;}let mut b=[0u8;96];for i in 0..n{
        let w=ptr::read_unaligned(phys(v+2*i as u64,2,false)?.cast::<u16>());if w>127{return None;}b[i]=w as u8;
    }crate::trialfs::Path::parse(&b[..n])
}
unsafe fn arg(f:&Frame,off:u64)->Option<u64>{Some(ptr::read_unaligned(phys(f.rsp.checked_add(off)?,8,false)?.cast::<u64>()))}
unsafe fn bool_result(f:&mut Frame,r:Result<(),u32>){match r{Ok(())=>f.rax=1,Err(e)=>{LAST=e;f.rax=0;}}}
unsafe fn read_write(f:&mut Frame,write:bool,x:u64,y:u64,z:u64,w:u64){
    let n=z as u32 as usize;
    if arg(f,40)!=Some(0)||phys(w,4,true).is_none()||n>65536{LAST=87;return;}
    if n==0{let r=if write{fs().write(x,&[])}else{fs().read(x,&mut [])};match r{Ok(k)=>{write32(w,k as u32);f.rax=1;},Err(e)=>LAST=e}return;}
    let Some(p)=phys(y,n,!write)else{LAST=87;return;};
    let r=if write{fs().write(x,core::slice::from_raw_parts(p,n))}else{fs().read(x,core::slice::from_raw_parts_mut(p,n))};
    match r{Ok(k)=>{write32(w,k as u32);f.rax=1;},Err(e)=>LAST=e}
}
unsafe fn realloc(v:u64,n:usize)->u64{
    if v==0{return allocate(n);}if n==0{release(v);return 0;}
    let Some(b)=(&*ptr::addr_of!(BLOCKS)).iter().find(|b|b.used&&b.at==v).copied()else{return 0;};
    if n<=b.len{return v;}let at=allocate(n);if at==0{return 0;}
    if let (Some(a),Some(b))=(phys(at,b.len,true),phys(v,b.len,false)){ptr::copy_nonoverlapping(b,a,n.min((&*ptr::addr_of!(BLOCKS)).iter().find(|q|q.at==v).map_or(0,|q|q.len)));release(v);at}else{release(at);0}
}
unsafe fn text_cmp(x:u64,y:u64,wide:bool)->Option<i32>{
    let step=if wide{2}else{1};for i in 0..65536usize{
        let a=phys(x.checked_add((i*step)as u64)?,step,false)?;let b=phys(y.checked_add((i*step)as u64)?,step,false)?;
        let a=if wide{ptr::read_unaligned(a.cast::<u16>())as i32}else{*a as i32};let b=if wide{ptr::read_unaligned(b.cast::<u16>())as i32}else{*b as i32};
        if a!=b||a==0{return Some(a-b);}
    }None
}
fn kind(d:&str,n:&str)->u8{
    if d.eq_ignore_ascii_case("kernel32.dll"){match n{"ExitProcess"=>1,"GetCurrentProcessId"|"GetCurrentThreadId"=>2,"SetLastError"=>3,"GetLastError"=>4,"GetCommandLineW"=>5,"GetStdHandle"=>6,"WriteFile"=>7,"GetTickCount"=>15,"GetCurrentProcess"=>16,"GetModuleHandleA"|"GetModuleHandleW"=>17,"CreateFileW"=>25,"ReadFile"=>26,"CloseHandle"=>27,"GetFileSize"=>28,"SetFilePointer"=>29,"SetEndOfFile"=>30,"CreateDirectoryW"=>31,"RemoveDirectoryW"=>32,"DeleteFileW"=>33,"GetFileAttributesW"=>34,"GetCurrentDirectoryW"=>35,"GetLargePageMinimum"=>39,"GetOEMCP"=>40,"GetConsoleMode"=>41,"SetConsoleMode"=>42,"LoadLibraryW"=>43,"QueryPerformanceFrequency"=>44,"QueryPerformanceCounter"=>45,"GetProcessAffinityMask"=>46,"SetProcessAffinityMask"=>47,"GetSystemInfo"|"GetNativeSystemInfo"=>48,"GetProcAddress"=>49,"FreeLibrary"=>50,"GetTickCount64"=>51,"GetModuleFileNameW"=>52,"GetSystemTimeAsFileTime"|"GetSystemTimePreciseAsFileTime"=>53,"GetActiveProcessorCount"=>54,"IsWow64Process"=>55,"VirtualAlloc"=>58,"VirtualFree"=>59,"IsProcessorFeaturePresent"=>60,"InitializeCriticalSection"=>61,"EnterCriticalSection"=>62,"LeaveCriticalSection"=>63,"DeleteCriticalSection"=>64,"InitializeCriticalSectionAndSpinCount"=>65,"TryEnterCriticalSection"=>66,"InitializeCriticalSectionEx"=>67,"GetConsoleScreenBufferInfo"=>68,"SetConsoleCtrlHandler"=>69,"GetFileType"=>71,"GetVersion"=>73,"GetVersionExA"|"GetVersionExW"=>74,"SetFileApisToOEM"=>76,"SetFileApisToANSI"=>77,"AreFileApisANSI"=>78,"GetACP"=>79,"MultiByteToWideChar"=>80,"WideCharToMultiByte"=>81,"GetLogicalDriveStringsW"=>83,"GetTempPathW"=>84,"LocalFree"=>85,"GetConsoleOutputCP"=>86,"SetDefaultDllDirectories"=>90,_=>0}}
    else if d.eq_ignore_ascii_case("msvcrt.dll"){match n{"exit"|"_exit"=>1,"malloc"=>8,"free"=>9,"memcpy"|"memmove"=>10,"memset"=>11,"strlen"=>12,"__set_app_type"=>13,"__getmainargs"=>14,"memcmp"=>18,"realloc"=>19,"strcmp"=>20,"wcscmp"=>21,"__setusermatherr"=>22,"_isatty"=>23,"_get_osfhandle"=>24,"fputs"=>36,"fputc"=>37,"fflush"=>38,"_onexit"=>56,"_cexit"|"_c_exit"=>57,"wcsstr"=>70,_=>0}}else if d.eq_ignore_ascii_case("advapi32.dll"){match n{"OpenProcessToken"=>87,"LookupPrivilegeValueW"=>88,"AdjustTokenPrivileges"=>89,"SystemFunction036"=>91,"RegOpenKeyExW"=>92,"RegQueryValueExW"|"RegCloseKey"=>93,"GetFileSecurityW"|"SetFileSecurityW"=>94,"GetTokenInformation"=>95,_=>0}}else if d.eq_ignore_ascii_case("ntdll.dll"){match n{"RtlInitializeCriticalSection"=>61,"RtlEnterCriticalSection"=>62,"RtlLeaveCriticalSection"=>63,"RtlDeleteCriticalSection"=>64,"RtlTryEnterCriticalSection"=>66,"RtlGetVersion"=>75,"RtlNtStatusToDosError"=>82,_=>0}}else{0}
}
unsafe fn init_env(){
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();
    let set=|o:usize,v:u64|ptr::write_unaligned(d.add(0x1000+o).cast::<u64>(),v);
    set(8,TOP);set(16,STACK);set(0x30,ENV);set(0x40,1);set(0x48,1);set(0x60,ENV+0x200);
    set(0x210,BASE);set(0x220,ENV+0x300);
    for(i,c)in "7za.exe -h".encode_utf16().enumerate(){ptr::write_unaligned(d.add(0x1400+2*i).cast::<u16>(),c);}
    // RTL_USER_PROCESS_PARAMETERS command line UNICODE_STRING at 0x70.
    ptr::write_unaligned(d.add(0x1370).cast::<u16>(),20);ptr::write_unaligned(d.add(0x1372).cast::<u16>(),22);set(0x378,ENV+0x400);
    ptr::copy_nonoverlapping(b"7za.exe\0-h\0".as_ptr(),d.add(0x1500),11);
    set(0x600,ENV+0x500);set(0x608,ENV+0x508);set(0x610,0);set(0x620,0);
    // Narrow legacy CRT stream records: only output via fputs/fputc/fflush is supported.
    for i in 0..3usize{ptr::write_unaligned(d.add(0x1700+i*48+28).cast::<u32>(),i as u32);}

}
unsafe fn reset(){
    ptr::write_bytes(ptr::addr_of_mut!(DATA.0).cast::<u8>(),0,LIMIT);
    ptr::write_bytes(ptr::addr_of_mut!(PT0.0).cast::<u64>(),0,512);ptr::write_bytes(ptr::addr_of_mut!(PT1.0).cast::<u64>(),0,512);
    fs().reset();MATH_HANDLER=0;CALLS=0;LAST_API=usize::MAX;HELP_OK=false;DETAIL_LEN=0;ONEXIT_LEN=0;LOCKS=[EMPTY_LOCK;128];CONSOLE_MODE=3;CTRL_HANDLER=0;FILE_OEM=false;TOKENS=[EMPTY_TOKEN;16];TOKEN_NEXT=0x10000;CLEANUP_ACTIVE=false;DLL_POLICY=0;BLOCKS=[FREE;128];NEXT=HEAP;N=0;IMAGE_SIZE=0;OUT_LEN=0;LAST=0;EXIT=0;APP_TYPE=0;
    H5_TRIAL_STATUS=0;H5_TRIAL_FAULT=0;H5_TRIAL_RIP=0;
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>()as u64;
    let t=ptr::addr_of_mut!(PT1.0).cast::<u64>();
    t.write(d|5);t.add(1).write(d+4096|7|NX);
    for i in 16..48usize{t.add(i).write(d+i as u64*4096|7|NX);}
    for i in 64..512usize{t.add(i).write(d+i as u64*4096|7|NX);}
    init_env();
}
unsafe fn supported()->bool{
    if !nx_available()||__cpuid(1).edx&(1<<24)==0{return false;}
    for base in [ptr::addr_of!(IMAGE)as u64,ptr::addr_of!(DATA)as u64]{if base+LIMIT as u64>1<<32||!(0..LIMIT).step_by(4096).all(|i|cpu::writable_identity(base+i as u64)){return false;}}
    [ptr::addr_of!(PT0)as u64,ptr::addr_of!(PT1)as u64].iter().all(|v|cpu::writable_identity(*v))
}
unsafe fn enter(ip:u64,budget:u64)->u64{
    let pd=ptr::addr_of_mut!(platform::USER_PD.0).cast::<u64>();let a=pd.read();let b=pd.add(1).read();
    let oldgs=msr(0xc0000101);let oldfs=msr(0xc0000100);let efer=msr(0xc0000080);
    let cr4:u64;asm!("mov {},cr4",out(reg)cr4,options(nostack));
    // Disable SYSCALL, FSGSBASE and AVX during the single-thread trial.
    set_msr(0xc0000080,(efer|1<<11)&!1);
    asm!("mov cr4,{}",in(reg)(cr4&!((1<<16)|(1<<18))),options(nostack));
    set_msr(0xc0000101,ENV);set_msr(0xc0000100,0);
    pd.write(ptr::addr_of!(PT0)as u64|7);pd.add(1).write(ptr::addr_of!(PT1)as u64|7);flush();
    // Windows x64 entry requires a return address and 32-byte caller home area.
    let sp=TOP-40;
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();
    ptr::write_unaligned(d.add((sp-AUX)as usize).cast::<u64>(),AUX+0xe00);
    // Return trampoline: propagate the entry point's EAX as exit status.
    let ret=[0x89u8,0xc1,0xb8,0x00,0x01,0x00,0x00,0xcd,0x81,0x0f,0x0b];
    ptr::copy_nonoverlapping(ret.as_ptr(),d.add(0xe00),ret.len());
    H5_TRIAL_BUDGET=budget;H5_TRIAL_ACTIVE=1;h5_trial_enter(ip,sp);H5_TRIAL_ACTIVE=0;
    pd.write(a);pd.add(1).write(b);flush();set_msr(0xc0000101,oldgs);set_msr(0xc0000100,oldfs);set_msr(0xc0000080,efer);
    asm!("mov cr4,{}",in(reg)cr4,options(nostack));
    H5_TRIAL_STATUS
}
pub fn self_test()->bool{unsafe{
    if !supported(){serial::line("H5K: TRIAL_NX_UNAVAILABLE");return false;}
    if !fs().test(){return false;}serial::line("H5K: TRIAL_RAMFS_OK");
    reset();let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();ptr::copy_nonoverlapping([0x0fu8,0x0b].as_ptr(),d,2);
    if enter(AUX,50)!=2{return false;}serial::line("H5K: TRIAL_FAULT_RECOVERED");
    reset();ptr::copy_nonoverlapping([0xebu8,0xfe].as_ptr(),d,2);
    if enter(AUX,10)!=3{return false;}serial::line("H5K: TRIAL_TIMEOUT_RECOVERED");
    reset();
    // Exercise module name lookup and absent export behavior without relying on 7-Zip.
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();
    ptr::copy_nonoverlapping(b"kernel32.dll\0".as_ptr(),d.add(0x1800),13);
    ptr::copy_nonoverlapping(b"GetTickCount64\0".as_ptr(),d.add(0x1820),15);
    ptr::copy_nonoverlapping(b"NotARealExport\0".as_ptr(),d.add(0x1840),15);
    let h=module_handle(ENV+0x800,false);
    if h!=MOD_BASE||proc_address(h,ENV+0x820)!=AUX||proc_address(h,ENV+0x840)!=0||LAST!=127{return false;}
    serial::line("H5K: TRIAL_DLL_RESOLVER_OK");
    reset();
    // CPU feature behavior and recursive lock ownership/underflow validation.
    if !processor_feature(9)||processor_feature(39)||processor_feature(22)||processor_feature(0xffffffff){return false;}
    let a=__cpuid(1);if processor_feature(2)!=(a.edx&(1<<8)!=0){return false;}
    serial::line("H5K: TRIAL_CPU_FEATURES_OK");
    let v=ENV+0x900;
    if !lock_op(v,0,0)||lock_op(v,0,0)||!lock_op(v,1,0)||!lock_op(v,1,0)||lock_op(v,3,0){return false;}
    let Some(p)=phys(v,40,false)else{return false;};
    if ptr::read_unaligned(p.add(12).cast::<u32>())!=2||ptr::read_unaligned(p.add(16).cast::<u64>())!=1{return false;}
    if !lock_op(v,2,0)||!lock_op(v,2,0)||lock_op(v,2,0)||!lock_op(v,3,0)||lock_op(v,1,0){return false;}
    serial::line("H5K: TRIAL_CRITICAL_SECTION_OK");
    reset();
    let v=ENV+0x800;if !write32(v,284)||!version_info(v,true){return false;}
    let Some(p)=phys(v,284,false)else{return false;};
    if ptr::read_unaligned(p.add(4).cast::<u32>())!=WIN_MAJOR||ptr::read_unaligned(p.add(8).cast::<u32>())!=WIN_MINOR||ptr::read_unaligned(p.add(12).cast::<u32>())!=WIN_BUILD||WIN_VERSION!=0x1db10106{return false;}
    if !write32(v,1)||version_info(v,true){return false;}
    serial::line("H5K: TRIAL_VERSION_INFO_OK");
    reset();
    let v=ENV+0x800;let state=v+16;let prev=v+64;let ret=v+80;
    if token_open(u64::MAX,0x28,v).is_err()||token_open(u64::MAX,0x100,v)!=Err(5){return false;}
    let Some(p)=phys(v,8,false)else{return false;};let h=ptr::read_unaligned(p.cast::<u64>());
    if !write32(state,1)||!write64(state+4,17)||!write32(state+12,2){return false;}
    if adjust_privileges(h,false,state,4,prev,ret)!=Ok(1300)||adjust_privileges(h,true,0,4,prev,ret)!=Ok(0)||!token_close(h)||token_access(h).is_some(){return false;}
    serial::line("H5K: TRIAL_RESTRICTED_TOKEN_OK");
    reset();
    // Store into all four x64 home slots, then return 42 through the entry trampoline.
    let code=[0x48u8,0xc7,0x44,0x24,0x08,1,0,0,0,0x48,0xc7,0x44,0x24,0x10,2,0,0,0,0x48,0xc7,0x44,0x24,0x18,3,0,0,0,0x48,0xc7,0x44,0x24,0x20,4,0,0,0,0xb8,42,0,0,0,0xc3];
    ptr::copy_nonoverlapping(code.as_ptr(),d,code.len());
    if enter(AUX,50)!=1||EXIT!=42{return false;}serial::line("H5K: TRIAL_WIN64_ENTRY_OK");true
}}
pub const PACKAGE_SIZE:usize=1328640;
pub fn verified(b:&[u8])->bool{b.len()==PACKAGE_SIZE&&sha256(b)==*PIN}
pub fn help_ok()->bool{unsafe{HELP_OK}}
pub fn run(b:&[u8])->Result<u64,&'static str>{unsafe{
    if !verified(b){return Err("Only the pinned official 7-Zip executable is allowed");}
    if !self_test(){return Err("Trial containment checks failed");}
    reset();
    let image=&mut *ptr::addr_of_mut!(IMAGE.0);let r=relocate::map_at(b,image,&mut *ptr::addr_of_mut!(SEEN),Some(BASE))?;
    IMAGE_SIZE=r.size;
    let pe=u32at(b,60).ok_or("PE")?as usize;let opt=pe+24;let opts=u16at(b,pe+20).ok_or("Optional header")?as usize;let count=u16at(b,pe+6).ok_or("Sections")?as usize;
    let entry=u32at(b,opt+16).ok_or("Entry")?as u64;
    let table=ptr::addr_of_mut!(PT0.0).cast::<u64>();let raw=image.as_mut_ptr()as u64;
    let headers=u32at(b,opt+60).ok_or("Headers")?as usize;for i in 0..headers.div_ceil(4096){table.add(i).write(raw+i as u64*4096|5|NX);}
    for i in 0..count{let s=opt+opts+i*40;let va=u32at(b,s+12).unwrap()as usize;let span=u32at(b,s+8).unwrap().max(u32at(b,s+16).unwrap())as usize;let attr=u32at(b,s+36).unwrap();
        let flags=5|if attr&0x80000000!=0{2}else{0}|if attr&0x20000000==0{NX}else{0};
        for j in va/4096..(va+span).div_ceil(4096){table.add(j).write(raw+j as u64*4096|flags);}
    }
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();let mut ok=true;let mut implemented=0;
    let start=h5_initterm_start as usize;let end=h5_initterm_end as usize;let len=end.checked_sub(start).ok_or("CRT stub layout")?;
    if len>256{return Err("CRT stub too large");}ptr::copy_nonoverlapping(start as *const u8,d.add(0xf00),len);
    let start=h5_cleanup_start as usize;let len=(h5_cleanup_end as usize).checked_sub(start).ok_or("Cleanup stub layout")?;
    if len>256{return Err("Cleanup stub too large");}ptr::copy_nonoverlapping(start as *const u8,d.add(0xd00),len);
    let report=imports::inspect_slots(b,|dll,s,slot|{
        let i=N;if i>=208||dll.len()>48||slot as usize+8>r.size||slot%8!=0{ok=false;return;}
        let mut a=EMPTY;a.dn=dll.len();a.dll[..a.dn].copy_from_slice(dll.as_bytes());
        match s{imports::Symbol::Name(n)=>{if n.len()>128{ok=false;return;}a.nn=n.len();a.name[..a.nn].copy_from_slice(n.as_bytes());a.kind=kind(dll,n);},imports::Symbol::Ordinal(n)=>{a.name[0]=b'#';for j in 0..4{a.name[1+j]=b"0123456789abcdef"[((n>>(12-j*4))&15)as usize];}a.nn=5;}}
        ptr::addr_of_mut!(APIS).cast::<Api>().add(i).write(a);N+=1;if a.kind!=0||dll.eq_ignore_ascii_case("msvcrt.dll")&&matches!(&a.name[..a.nn],b"_initterm"){implemented+=1;}
        // Data imports must be data, not executable trampolines. Unsupported data remains unmapped.
        let name=&a.name[..a.nn];let target=if dll.eq_ignore_ascii_case("msvcrt.dll"){
            match name{b"__initenv"=>{let p=ENV+0x640;ptr::write_unaligned(d.add(0x1640).cast::<u64>(),ENV+0x620);p},b"_commode"=>ENV+0x648,b"_fmode"=>ENV+0x64c,b"_iob"=>ENV+0x700,b"_initterm"=>AUX+0xf00,_=>AUX+i as u64*16}
        }else{AUX+i as u64*16};
        let stub=[0xb8,i as u8,0,0,0,0xcd,0x81,0xc3];ptr::copy_nonoverlapping(stub.as_ptr(),d.add(i*16),stub.len());
        image[slot as usize..slot as usize+8].copy_from_slice(&target.to_le_bytes());
    })?;
    if !ok||report.functions!=145||report.delayed{return Err("Pinned import layout mismatch");}
    serial::metric("H5K: TRIAL_SUPPORTED_STATIC=",implemented);serial::line("H5K: TRIAL_ENTER_7ZIP");
    let status=enter(BASE+entry,500);
    serial::metric("H5K: TRIAL_RESULT=",status);serial::metric("H5K: TRIAL_EXIT=",EXIT as u64);
    serial::metric("H5K: TRIAL_FAULT_ADDRESS=",H5_TRIAL_FAULT);serial::metric("H5K: TRIAL_FAULT_RIP=",H5_TRIAL_RIP);
    serial::metric("H5K: TRIAL_API_CALLS=",CALLS);
    if DETAIL_LEN==0&&status==2{detail(b"CPU",b"application fault");}else if DETAIL_LEN==0&&status==3{detail(b"CPU",b"execution timeout");}
    if LAST_API<N{let a=ptr::addr_of!(APIS).cast::<Api>().add(LAST_API).read();serial::write("H5K: TRIAL_LAST_API|");serial::write(core::str::from_utf8(&a.dll[..a.dn]).unwrap_or("?"));serial::write("|");serial::line(core::str::from_utf8(&a.name[..a.nn]).unwrap_or("?"));}
    let bytes=core::slice::from_raw_parts(ptr::addr_of!(OUT).cast::<u8>(),OUT_LEN);
    serial::line("H5K: TRIAL_OUTPUT_BEGIN");for line in bytes.split(|c|*c==10){if let Ok(s)=core::str::from_utf8(line){if s.bytes().all(|c|c==13||c==9||(32..127).contains(&c)){serial::line(s);}}}
    serial::line("H5K: TRIAL_OUTPUT_END");
    let help=status==1&&EXIT==0&&bytes.windows(5).any(|w|w==b"7-Zip")&&bytes.windows(6).any(|w|w==b"Usage:");
    HELP_OK=help;if help{serial::line("H5K: SEVENZIP_HELP_OK");}else{serial::line("H5K: SEVENZIP_HELP_NOT_QUALIFIED");}
    serial::line("H5K: TRIAL_RETURNED");Ok(status)
}}
pub fn output(out:&mut[u8])->usize{unsafe{let n=OUT_LEN.min(out.len());ptr::copy_nonoverlapping(ptr::addr_of!(OUT).cast::<u8>(),out.as_mut_ptr(),n);n}}
// SHA-256 pins the exact official build, rather than trusting unauthenticated HTTP or CRC.
fn sha256(b:&[u8])->[u8;32]{
    const K:[u32;64]=[0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2];
    let mut h=[0x6a09e667u32,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let blocks=(b.len()+9).div_ceil(64);let bits=(b.len()as u64*8).to_be_bytes();
    for i in 0..blocks{let mut block=[0u8;64];for j in 0..64{let p=i*64+j;block[j]=if p<b.len(){b[p]}else if p==b.len(){128}else if p>=blocks*64-8{bits[p-(blocks*64-8)]}else{0};}
        let mut w=[0u32;64];for j in 0..16{w[j]=u32::from_be_bytes(block[j*4..j*4+4].try_into().unwrap());}
        for j in 16..64{let x=w[j-15];let y=w[j-2];w[j]=w[j-16].wrapping_add(x.rotate_right(7)^x.rotate_right(18)^(x>>3)).wrapping_add(w[j-7]).wrapping_add(y.rotate_right(17)^y.rotate_right(19)^(y>>10));}
        let [mut a,mut c,mut d,mut e,mut g,mut j,mut l,mut m]=h;
        for t in 0..64{let t1=m.wrapping_add(g.rotate_right(6)^g.rotate_right(11)^g.rotate_right(25)).wrapping_add((g&j)^(!g&l)).wrapping_add(K[t]).wrapping_add(w[t]);let t2=(a.rotate_right(2)^a.rotate_right(13)^a.rotate_right(22)).wrapping_add((a&c)^(a&d)^(c&d));m=l;l=j;j=g;g=e.wrapping_add(t1);e=d;d=c;c=a;a=t1.wrapping_add(t2);}
        for (x,y) in h.iter_mut().zip([a,c,d,e,g,j,l,m]){*x=x.wrapping_add(y);}
    }let mut result=[0;32];for(i,v)in h.iter().enumerate(){result[i*4..i*4+4].copy_from_slice(&v.to_be_bytes());}result
}
