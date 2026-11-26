//! Experimental single-CPU, pinned-binary PE startup probe. Not a general sandbox.
use core::{arch::{asm,global_asm,x86_64::__cpuid},ptr};
use crate::{platform,imports,relocate,serial,cpu};
const BASE:u64=0x1_0000_0000;
const AUX:u64=BASE+0x200000;
const ENV:u64=AUX+0x1000;
const STACK:u64=AUX+0x10000;
const TOP:u64=AUX+0x30000;
const HEAP:u64=AUX+0x200000;
const HEAP_BYTES:usize=32*1024*1024;
const HEAP_END:u64=HEAP+HEAP_BYTES as u64;
#[repr(align(4096))]struct HeapPages([u8;HEAP_BYTES]);
static mut HEAP_DATA:HeapPages=HeapPages([0;HEAP_BYTES]);
static mut HEAP_PT:[platform::Table;16]=[const {platform::Table([0;512])};16];
const NX:u64=1<<63;
const LIMIT:usize=2*1024*1024;
const CODEC_BASE:u64=BASE+18*0x200000;
const CODEC_FILE:&[u8]=include_bytes!("../target/hi5-fixture/7z.dll");
const CODEC_PIN:&[u8;32]=include_bytes!("../target/hi5-fixture/7z.dll.sha256");
#[repr(align(4096))]struct Pages([u8;LIMIT]);
static mut IMAGE:Pages=Pages([0;LIMIT]);
static mut CODEC_IMAGE:Pages=Pages([0;LIMIT]);
static mut CODEC_PT:platform::Table=platform::Table([0;512]);
static mut CODEC_SIZE:usize=0;
static mut CODEC_READY:bool=false;
static mut CODEC_PENDING:Option<(u64,u64,u64)>=None;
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
const API_LIMIT:usize=464;
static mut APIS:[Api;API_LIMIT]=[EMPTY;API_LIMIT];
static mut ORIGINAL_GUI:bool=false;
static mut GUI_STARTUP_RESULT:u64=0;
#[no_mangle]static mut H5_GUEST_FRAME:[u64;10]=[0;10];
#[no_mangle]static mut H5_GUEST_NV:[u64;8]=[0;8];
#[no_mangle]static mut H5_GUEST_FX:Fp=Fp([0;512]);
#[no_mangle]static mut H5_RESUME:u64=0;
static mut LIVE_REQUEST:bool=false;
static mut LIVE:bool=false;
static mut LIVE_ROOT:u64=0;
static mut LIVE_MENU:u64=0;
// Logical desktop used by the isolated startup probe, not host display access.
const GUI_WORK_WIDTH:i32=800;const GUI_WORK_HEIGHT:i32=560;
const GUI_DIALOG_X:u32=6;const GUI_DIALOG_Y:u32=12;
static mut COMMON_INIT_SEEN:bool=false;
static mut PRNG:u32=1;
static mut N:usize=0;
static mut IMAGE_SIZE:usize=0;
static mut EXIT:u32=0;
static mut LAST:u32=0;
static mut APP_TYPE:u32=0;
pub const OUTPUT_CAP:usize=32768;
static mut OUT:[u8;OUTPUT_CAP]=[0;OUTPUT_CAP];
static mut OUT_LEN:usize=0;
#[derive(Clone,Copy)]struct Block{at:u64,len:usize,used:bool}
const FREE:Block=Block{at:0,len:0,used:false};
static mut BLOCKS:[Block;2048]=[FREE;2048];
static mut NEXT:u64=HEAP;
static mut FS:crate::trialfs::Fs=crate::trialfs::Fs::new();
static mut WORK:crate::trialfs::Fs=crate::trialfs::Fs::new();
static mut WORK_ACTIVE:bool=false;
static mut WORK_READY:bool=false;
static mut WORK_BACKUP:[u8;crate::trialfs::SNAP_CAP]=[0;crate::trialfs::SNAP_CAP];
static mut ARGC:u32=2;
static mut FIND:[Search;16]=[EMPTY_SEARCH;16];
#[derive(Clone,Copy)]struct Search{used:bool,pat:[u8;128],len:usize,cursor:usize}
const EMPTY_SEARCH:Search=Search{used:false,pat:[0;128],len:0,cursor:0};
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
// Process-local event handles. Blocking waits require a real scheduler.
#[derive(Clone,Copy)]struct Event{handle:u64,manual:bool,signaled:bool}
const EMPTY_EVENT:Event=Event{handle:0,manual:false,signaled:false};
static mut EVENTS:[Event;64]=[EMPTY_EVENT;64];
static mut EVENT_NEXT:u64=0x40000000;
static mut CLEANUP_ACTIVE:bool=false;
static mut DLL_POLICY:u32=0;
const MODULES:[&str;12]=["kernel32.dll","msvcrt.dll","advapi32.dll","oleaut32.dll","user32.dll","ntdll.dll","gdi32.dll","comctl32.dll","comdlg32.dll","ole32.dll","shell32.dll","uxtheme.dll"];
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
    cmp qword ptr [rip+H5_RESUME],0
    jne h5_guest_resume
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
h5_guest_resume:
    mov qword ptr [rip+H5_RESUME],0
    lea rax,[rip+H5_GUEST_FRAME]
    push qword ptr [rax+72]
    push qword ptr [rax+64]
    push qword ptr [rax+56]
    push qword ptr [rax+48]
    push qword ptr [rax+40]
    fxrstor64 [rip+H5_GUEST_FX]
    mov rbx,[rip+H5_GUEST_NV]
    mov rbp,[rip+H5_GUEST_NV+8]
    mov rsi,[rip+H5_GUEST_NV+16]
    mov rdi,[rip+H5_GUEST_NV+24]
    mov r12,[rip+H5_GUEST_NV+32]
    mov r13,[rip+H5_GUEST_NV+40]
    mov r14,[rip+H5_GUEST_NV+48]
    mov r15,[rip+H5_GUEST_NV+56]
    mov rcx,[rax+8]
    mov rdx,[rax+16]
    mov r8,[rax+24]
    mov r9,[rax+32]
    xor r10d,r10d
    xor r11d,r11d
    mov rax,[rax]
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
    cmp qword ptr [rip+H5_TRIAL_STATUS],5
    jne h5_guest_no_yield
    mov [rip+H5_GUEST_NV],rbx
    mov [rip+H5_GUEST_NV+8],rbp
    mov [rip+H5_GUEST_NV+16],rsi
    mov [rip+H5_GUEST_NV+24],rdi
    mov [rip+H5_GUEST_NV+32],r12
    mov [rip+H5_GUEST_NV+40],r13
    mov [rip+H5_GUEST_NV+48],r14
    mov [rip+H5_GUEST_NV+56],r15
    fxsave64 [rip+H5_GUEST_FX]
    mov rsi,rsp
    lea rdi,[rip+H5_GUEST_FRAME]
    mov ecx,10
    rep movsq
    jmp h5_trial_return
h5_guest_no_yield:
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
    if v>=HEAP&&end<=HEAP_END{return Some(ptr::addr_of_mut!(HEAP_DATA.0).cast::<u8>().add((v-HEAP)as usize));}
    let (base,table,raw)=if v>=BASE&&end<=BASE+IMAGE_SIZE as u64{(BASE,ptr::addr_of!(PT0.0).cast::<u64>(),ptr::addr_of_mut!(IMAGE.0).cast::<u8>())}
        else if v>=CODEC_BASE&&end<=CODEC_BASE+CODEC_SIZE as u64{(CODEC_BASE,ptr::addr_of!(CODEC_PT.0).cast::<u64>(),ptr::addr_of_mut!(CODEC_IMAGE.0).cast::<u8>())}
        else if v>=AUX&&end<=AUX+LIMIT as u64{(AUX,ptr::addr_of!(PT1.0).cast::<u64>(),ptr::addr_of_mut!(DATA.0).cast::<u8>())}else{return None;};
    let start=(v-base)as usize;let last=(end-base-1)as usize;
    for i in start/4096..=last/4096{let e=table.add(i).read();if e&5!=5||write&&e&2==0{return None;}}
    Some(raw.add(start))
}
unsafe fn executable(v:u64)->bool{
    if v>=CODEC_BASE&&v<CODEC_BASE+CODEC_SIZE as u64{let i=((v-CODEC_BASE)/4096)as usize;let p=ptr::addr_of!(CODEC_PT.0).cast::<u64>().add(i).read();return p&5==5&&p&NX==0;}
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
    // Find best-fit free range with correct alignment. Split when descriptors permit.
    let mut best=None;let mut best_len=usize::MAX;
    for (i,b) in blocks.iter().enumerate(){
        if b.used||b.at==0{continue;}
        let Some(at)=b.at.checked_add(align-1).map(|v|v&!(align-1))else{continue;};
        if at.checked_add(n as u64).is_some_and(|end|end<=b.at+b.len as u64)&&b.len<best_len{best=Some((i,at));best_len=b.len;}
    }
    if let Some((i,at))=best{
        let b=blocks[i];let prefix=(at-b.at)as usize;let suffix=b.len-prefix-n;
        let free=blocks.iter().filter(|v|v.at==0).count();
        if free>=(prefix!=0)as usize+(suffix!=0)as usize{
            blocks[i]=Block{at,len:n,used:true};
            for (p,len) in [(b.at,prefix),(at+n as u64,suffix)]{if len!=0{if let Some(slot)=blocks.iter_mut().find(|v|v.at==0){*slot=Block{at:p,len,used:false};}}}
            return at;
        }
        if prefix==0{blocks[i].used=true;return at;}
    }
    let Some(i)=blocks.iter().position(|b|b.at==0)else{serial::metric("H5K: TRIAL_ALLOC_RECORDS_EXHAUSTED=",n as u64);return 0;};
    let Some(at)=NEXT.checked_add(align-1).map(|v|v&!(align-1))else{return 0;};
    if at.checked_add(n as u64).map_or(true,|x|x>HEAP_END){serial::metric("H5K: TRIAL_ALLOC_OOM_BYTES=",n as u64);return 0;}
    if at>NEXT{if let Some(j)=blocks.iter().enumerate().find(|(j,b)|*j!=i&&b.at==0).map(|(j,_)|j){blocks[j]=Block{at:NEXT,len:(at-NEXT)as usize,used:false};}}
    NEXT=at+n as u64;blocks[i]=Block{at,len:n,used:true};at
}
unsafe fn release(v:u64)->bool{
    if v==0{return true;}let blocks=&mut *ptr::addr_of_mut!(BLOCKS);
    let Some(i)=blocks.iter().position(|b|b.at==v&&b.used)else{return false;};blocks[i].used=false;
    loop{
        let a=blocks[i];let Some(j)=blocks.iter().enumerate().find(|(j,b)|*j!=i&&!b.used&&b.at!=0&&(b.at+b.len as u64==a.at||a.at+a.len as u64==b.at)).map(|(j,_)|j)else{break;};
        let b=blocks[j];blocks[i]=Block{at:a.at.min(b.at),len:a.len+b.len,used:false};blocks[j]=FREE;
    }
    let a=blocks[i];if a.at+a.len as u64==NEXT{NEXT=a.at;blocks[i]=FREE;}true
}
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
const CP437:[u16;128]=[0xc7,0xfc,0xe9,0xe2,0xe4,0xe0,0xe5,0xe7,0xea,0xeb,0xe8,0xef,0xee,0xec,0xc4,0xc5,0xc9,0xe6,0xc6,0xf4,0xf6,0xf2,0xfb,0xf9,0xff,0xd6,0xdc,0xa2,0xa3,0xa5,0x20a7,0x192,0xe1,0xed,0xf3,0xfa,0xf1,0xd1,0xaa,0xba,0xbf,0x2310,0xac,0xbd,0xbc,0xa1,0xab,0xbb,0x2591,0x2592,0x2593,0x2502,0x2524,0x2561,0x2562,0x2556,0x2555,0x2563,0x2551,0x2557,0x255d,0x255c,0x255b,0x2510,0x2514,0x2534,0x252c,0x251c,0x2500,0x253c,0x255e,0x255f,0x255a,0x2554,0x2569,0x2566,0x2560,0x2550,0x256c,0x2567,0x2568,0x2564,0x2565,0x2559,0x2558,0x2552,0x2553,0x256b,0x256a,0x2518,0x250c,0x2588,0x2584,0x258c,0x2590,0x2580,0x3b1,0xdf,0x393,0x3c0,0x3a3,0x3c3,0xb5,0x3c4,0x3a6,0x398,0x3a9,0x3b4,0x221e,0x3c6,0x3b5,0x2229,0x2261,0xb1,0x2265,0x2264,0x2320,0x2321,0xf7,0x2248,0xb0,0x2219,0xb7,0x221a,0x207f,0xb2,0x25a0,0xa0];
const CP1252:[u16;32]=[0x20ac,0x81,0x201a,0x192,0x201e,0x2026,0x2020,0x2021,0x2c6,0x2030,0x160,0x2039,0x152,0x8d,0x17d,0x8f,0x90,0x2018,0x2019,0x201c,0x201d,0x2022,0x2013,0x2014,0x2dc,0x2122,0x161,0x203a,0x153,0x9d,0x17e,0x178];
fn codepage(cp:u32)->Option<u32>{match cp{0|3|1252=>Some(1252),1|437=>Some(437),65001=>Some(65001),_=>None}}
fn legacy_decode(cp:u32,v:u8)->u16{
    if cp==437&&v>=128{CP437[(v-128)as usize]}else if cp==1252&&(128..160).contains(&v){CP1252[(v-128)as usize]}else{v as u16}
}
fn legacy_encode(cp:u32,c:char)->Option<u8>{
    let v=c as u32;if v<128{return Some(v as u8);}
    if cp==437{CP437.iter().position(|n|*n as u32==v).map(|i|(128+i)as u8)}
    else if (160..=255).contains(&v){Some(v as u8)}else{CP1252.iter().position(|n|*n as u32==v).map(|i|(128+i)as u8)}
}
fn nls_failure(name:&str,cp:u32,flags:u32,error:u32){
    unsafe{LAST=error;}
    serial::write("H5K: TRIAL_NLS_FAILURE|");serial::line(name);
    serial::metric("H5K: TRIAL_NLS_CP=",cp as u64);serial::metric("H5K: TRIAL_NLS_FLAGS=",flags as u64);serial::metric("H5K: TRIAL_NLS_ERROR=",error as u64);
}
unsafe fn mb_to_wide(f:&mut Frame,x:u64,y:u64,z:u64,w:u64){
    let flags=y as u32;let fail=|e|nls_failure("MultiByteToWideChar",x as u32,flags,e);
    let Some(cp)=codepage(x as u32)else{fail(87);return;};if flags&!8!=0{fail(1004);return;}
    let count=w as u32 as i32;
    let n=if count==-1{match c_len(z,false){Some(n)=>n+1,None=>{fail(87);return;}}}else if count>0{count as usize}else{fail(87);return;};
    if n>65536{fail(87);return;}let Some(p)=phys(z,n,false)else{fail(87);return;};
    let bytes=core::slice::from_raw_parts(p,n);
    let utf=if cp==65001{match core::str::from_utf8(bytes){Ok(s)=>Some(s),Err(_)=>{fail(1113);return;}}}else{None};
    let need=if let Some(s)=utf{s.encode_utf16().count()}else{n};
    let Some(dst)=arg(f,40)else{fail(87);return;};let Some(cap)=arg(f,48).map(|v|v as u32 as i32)else{fail(87);return;};
    if cap==0{f.rax=need as u64;return;}if cap<0{fail(87);return;}if (cap as usize)<need{fail(122);return;}
    let Some(q)=phys(dst,need*2,true)else{fail(87);return;};
    if dst<z+n as u64&&z<dst+(need*2)as u64{fail(87);return;}
    if let Some(s)=utf{for(i,c)in s.encode_utf16().enumerate(){ptr::write_unaligned(q.add(i*2).cast::<u16>(),c);}}
    else{for(i,c)in bytes.iter().enumerate(){ptr::write_unaligned(q.add(i*2).cast::<u16>(),legacy_decode(cp,*c));}}
    f.rax=need as u64;
}
unsafe fn wide_to_mb(f:&mut Frame,x:u64,y:u64,z:u64,w:u64){
    let flags=y as u32;let fail=|e|nls_failure("WideCharToMultiByte",x as u32,flags,e);
    let Some(cp)=codepage(x as u32)else{fail(87);return;};
    if cp==65001&&flags&!0x80!=0||cp!=65001&&flags&!0x400!=0{fail(1004);return;}
    let count=w as u32 as i32;let n=if count==-1{match c_len(z,true){Some(n)=>n+1,None=>{fail(87);return;}}}else if count>0{count as usize}else{fail(87);return;};
    if n>65536{fail(87);return;}let Some(p)=phys(z,n*2,false)else{fail(87);return;};
    let (Some(dst),Some(cap),Some(default),Some(used))=(arg(f,40),arg(f,48).map(|v|v as u32 as i32),arg(f,56),arg(f,64))else{fail(87);return;};
    if cp==65001&&(default!=0||used!=0)||cap<0{fail(87);return;}
    let fallback=if default==0{b'?'}else{let Some(q)=phys(default,1,false)else{fail(87);return;};*q};
    if used!=0&&phys(used,4,true).is_none(){fail(87);return;}
    let src=||core::char::decode_utf16((0..n).map(|i|ptr::read_unaligned(p.add(i*2).cast::<u16>())));
    let mut need=0usize;let mut substituted=false;
    for item in src(){
        let c=match item{Ok(c)=>c,Err(_)=>{if cp==65001&&flags&0x80!=0{fail(1113);return;}core::char::REPLACEMENT_CHARACTER}};
        if cp==65001{need+=c.len_utf8();}else{need+=1;if legacy_encode(cp,c).is_none(){substituted=true;}}
    }
    if cap==0{if used!=0{write32(used,substituted as u32);}f.rax=need as u64;return;}
    if (cap as usize)<need{fail(122);return;}
    let Some(q)=phys(dst,need,true)else{fail(87);return;};
    if dst<z+(n*2)as u64&&z<dst+need as u64||used!=0&&used<dst+need as u64&&dst<used+4{fail(87);return;}
    let mut i=0;
    for item in src(){
        let c=item.unwrap_or(core::char::REPLACEMENT_CHARACTER);
        if cp==65001{let mut b=[0u8;4];let s=c.encode_utf8(&mut b);ptr::copy_nonoverlapping(s.as_ptr(),q.add(i),s.len());i+=s.len();}
        else{q.add(i).write(legacy_encode(cp,c).unwrap_or(fallback));i+=1;}
    }
    if used!=0{write32(used,substituted as u32);}f.rax=need as u64;
}
unsafe fn nls_test()->bool{
    let src=ENV+0x800;let dst=ENV+0x840;let fallback=ENV+0x880;let used=ENV+0x884;let stack=STACK+0x100;
    let Some(p)=phys(src,8,true)else{return false;};for(i,c)in [0x41u16,0x00e9,0x20ac,0].iter().enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),*c);}
    let Some(q)=phys(fallback,1,true)else{return false;};q.write(b'!');
    let mut frame=Frame{rax:0,rcx:0,rdx:0,r8:0,r9:0,rip:0,cs:0,flags:0,rsp:stack,ss:0};
    // Exact two-pass pattern used by upstream 7-Zip: null/default args on sizing,
    // caller-provided default character and BOOL on the legacy output pass.
    if !write64(stack+40,0)||!write64(stack+48,0)||!write64(stack+56,0)||!write64(stack+64,0){return false;}
    wide_to_mb(&mut frame,437,0,src,3);if frame.rax!=3{return false;}
    if !write64(stack+40,dst)||!write64(stack+48,3)||!write64(stack+56,fallback)||!write64(stack+64,used){return false;}
    frame.rax=0;wide_to_mb(&mut frame,437,0,src,3);
    let Some(q)=phys(dst,3,false)else{return false;};let Some(u)=phys(used,4,false)else{return false;};
    if frame.rax!=3||core::slice::from_raw_parts(q,3)!=[0x41,0x82,b'!']||ptr::read_unaligned(u.cast::<u32>())!=1{return false;}
    frame.rax=0;wide_to_mb(&mut frame,1252,0,src,3);
    if frame.rax!=3||core::slice::from_raw_parts(q,3)!=[0x41,0xe9,0x80]||ptr::read_unaligned(u.cast::<u32>())!=0{return false;}
    if !write64(stack+40,src)||!write64(stack+48,4){return false;}
    frame.rax=0;mb_to_wide(&mut frame,1252,0,dst,3);
    if frame.rax!=3||ptr::read_unaligned(p.add(2).cast::<u16>())!=0xe9||ptr::read_unaligned(p.add(4).cast::<u16>())!=0x20ac{return false;}
    if !write64(stack+40,dst)||!write64(stack+48,8)||!write64(stack+56,0)||!write64(stack+64,0){return false;}
    frame.rax=0;wide_to_mb(&mut frame,65001,0,src,3);if frame.rax!=6||core::slice::from_raw_parts(q,6)!=b"A\xc3\xa9\xe2\x82\xac"{return false;}
    true
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
unsafe fn fill_find(out:u64,e:crate::trialfs::Info)->bool{
    let Some(p)=phys(out,592,true)else{return false;};ptr::write_bytes(p,0,592);
    ptr::write_unaligned(p.cast::<u32>(),e.attrs);for j in 0..3{ptr::write_unaligned(p.add(4+j*8).cast::<u64>(),e.times[j]);}
    ptr::write_unaligned(p.add(32).cast::<u32>(),e.size);
    for (j,c) in e.path.name().iter().enumerate(){ptr::write_unaligned(p.add(44+j*2).cast::<u16>(),*c as u16);}true
}
unsafe fn find_first(v:u64,out:u64)->Result<u64,u32>{
    if phys(out,592,true).is_none(){return Err(87);}let mut s=EMPTY_SEARCH;
    s.len=read_ascii(v,true,&mut s.pat).ok_or(123u32)?;
    let slot=(&*ptr::addr_of!(FIND)).iter().position(|v|!v.used).ok_or(4u32)?;
    let e=fs().matching(&s.pat[..s.len],&mut s.cursor).ok_or(2u32)?;
    if !fill_find(out,e){return Err(87);}s.used=true;ptr::addr_of_mut!(FIND).cast::<Search>().add(slot).write(s);Ok(0x20000+slot as u64)
}
unsafe fn find_next(h:u64,out:u64)->Result<(),u32>{
    let i=h.checked_sub(0x20000).filter(|v|*v<16).ok_or(6u32)? as usize;
    if phys(out,592,true).is_none(){return Err(87);}
    let s=&mut *ptr::addr_of_mut!(FIND).cast::<Search>().add(i);if !s.used{return Err(6);}
    let e=fs().matching(&s.pat[..s.len],&mut s.cursor).ok_or(18u32)?;if !fill_find(out,e){return Err(87);}Ok(())
}
unsafe fn find_close(h:u64)->Result<(),u32>{let i=h.checked_sub(0x20000).filter(|v|*v<16).ok_or(6u32)?as usize;let s=&mut *ptr::addr_of_mut!(FIND).cast::<Search>().add(i);if !s.used{return Err(6);}*s=EMPTY_SEARCH;Ok(())}
unsafe fn file_info(h:u64,out:u64)->Result<(),u32>{
    let e=fs().handle_info(h)?;let p=phys(out,52,true).ok_or(87u32)?;ptr::write_bytes(p,0,52);
    ptr::write_unaligned(p.cast::<u32>(),e.attrs);for j in 0..3{ptr::write_unaligned(p.add(4+j*8).cast::<u64>(),e.times[j]);}
    ptr::write_unaligned(p.add(28).cast::<u32>(),1);ptr::write_unaligned(p.add(36).cast::<u32>(),e.size);ptr::write_unaligned(p.add(40).cast::<u32>(),1);ptr::write_unaligned(p.add(48).cast::<u32>(),h as u32);Ok(())
}
unsafe fn bstr_len(v:u64)->Option<usize>{if v==0{return Some(0);}let at=v.checked_sub(4)?;let b=(&*ptr::addr_of!(BLOCKS)).iter().find(|b|b.at==at&&b.used)?;let n=ptr::read_unaligned(phys(at,4,false)?.cast::<u32>())as usize;if n%2!=0||n+6>b.len{return None;}Some(n/2)}
unsafe fn bstr_alloc(src:u64,n:usize)->u64{
    if n>65536{return 0;}let bytes=n*2;
    let source=if src!=0&&bytes!=0{let Some(p)=phys(src,bytes,false)else{return 0;};Some(p)}else{None};
    let at=allocate(bytes+6);if at==0{return 0;}let p=phys(at,bytes+6,true).unwrap();ptr::write_bytes(p,0,bytes+6);ptr::write_unaligned(p.cast::<u32>(),bytes as u32);
    if let Some(q)=source{ptr::copy(q,p.add(4),bytes);}at+4
}
unsafe fn bstr_free(v:u64)->bool{v==0||bstr_len(v).is_some()&&release(v-4)}
unsafe fn variant_clear(v:u64)->u32{
    let Some(p)=phys(v,24,true)else{return 0x80070057;};let ty=ptr::read_unaligned(p.cast::<u16>());
    if ty==8{let b=ptr::read_unaligned(p.add(8).cast::<u64>());if !bstr_free(b){return 0x80070057;}}
    else if !matches!(ty,0|1|2|3|4|5|6|7|10|11|16|17|18|19|20|21|22|23){return 0x80020008;}
    ptr::write_bytes(p,0,24);0
}
unsafe fn variant_copy(dst:u64,src:u64)->u32{
    if dst==src{return 0;}let Some(p)=phys(src,24,false)else{return 0x80070057;};let Some(q)=phys(dst,24,true)else{return 0x80070057;};if dst<src+24&&src<dst+24{return 0x80070057;}
    let mut b=[0u8;24];ptr::copy_nonoverlapping(p,b.as_mut_ptr(),24);let ty=u16::from_le_bytes(b[..2].try_into().unwrap());
    if !matches!(ty,0|1|2|3|4|5|6|7|8|10|11|16|17|18|19|20|21|22|23){return 0x80020008;}
    let mut new=0;if ty==8{let v=u64::from_le_bytes(b[8..16].try_into().unwrap());if v!=0{let Some(n)=bstr_len(v)else{return 0x80070057;};new=bstr_alloc(v,n);if new==0{return 0x8007000e;}b[8..16].copy_from_slice(&new.to_le_bytes());}}
    let code=variant_clear(dst);if code!=0{bstr_free(new);return code;}ptr::copy_nonoverlapping(b.as_ptr(),q,24);0
}
unsafe fn format_message(f:&mut Frame,x:u64,y:u64,z:u64,w:u64){
    let flags=x as u32;let id=z as u32;let (Some(buf),Some(cap),Some(args))=(arg(f,40),arg(f,48),arg(f,56))else{LAST=87;return;};
    // Only built-in system messages, no external module, format string or insert execution.
    if flags&0x1000==0||flags&!0x13ff!=0||y!=0||!matches!(w as u32,0|0x400|0x409)||args!=0&&flags&0x200==0{LAST=87;return;}
    let msg=match id{
        0=>"The operation completed successfully.",2=>"The system cannot find the file specified.",3=>"The system cannot find the path specified.",5=>"Access is denied.",6=>"The handle is invalid.",8=>"Not enough memory resources are available to process this command.",18=>"There are no more files.",32=>"The process cannot access the file because it is being used by another process.",50=>"The request is not supported.",80=>"The file exists.",87=>"The parameter is incorrect.",112=>"There is not enough space on the disk.",122=>"The data area passed to a system call is too small.",123=>"The filename, directory name, or volume label syntax is incorrect.",126=>"The specified module could not be found.",127=>"The specified procedure could not be found.",145=>"The directory is not empty.",183=>"Cannot create a file when that file already exists.",131=>"An attempt was made to move the file pointer before the beginning of the file.",1300=>"Not all privileges or groups referenced are assigned to the caller.",1113=>"No mapping for the Unicode character exists in the target multi-byte code page.",
        _=>{LAST=317;return;}
    };
    let n=msg.len();let allocate_buffer=flags&0x100!=0;
    let dest=if allocate_buffer{
        if phys(buf,8,true).is_none(){LAST=87;return;}
        let Some(bytes)=(n+1).max(cap as u32 as usize).checked_mul(2)else{LAST=8;return;};
        if bytes>65536{LAST=8;return;}let at=allocate(bytes);if at==0{LAST=8;return;}write64(buf,at);at
    }else{if cap as u32 as usize<=n{LAST=122;return;}buf};
    let Some(p)=phys(dest,(n+1)*2,true)else{LAST=87;return;};
    for(i,c)in msg.bytes().enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),c as u16);}ptr::write_unaligned(p.add(n*2).cast::<u16>(),0);f.rax=n as u64;
}
unsafe fn create_file(f:&mut Frame,x:u64,y:u64,z:u64,w:u64){
    // DWORD stack arguments occupy only the low 32 bits of their 8-byte ABI slots.
    // Upper padding may contain arbitrary caller data; pointer arguments remain full width.
    let mode=arg(f,40).map(|v|v as u32);let flags=arg(f,48).map(|v|v as u32);let template=arg(f,56);
    f.rax=u64::MAX;
    let mut name=[0u8;128];
    if let Some(n)=read_ascii(x,true,&mut name){serial::write("H5K: TRIAL_CREATE_FILE|");serial::line(core::str::from_utf8(&name[..n]).unwrap_or("?"));}
    serial::metric("H5K: TRIAL_FILE_ACCESS=",y as u32 as u64);serial::metric("H5K: TRIAL_FILE_SHARE=",z as u32 as u64);
    serial::metric("H5K: TRIAL_FILE_MODE=",mode.unwrap_or(u32::MAX)as u64);serial::metric("H5K: TRIAL_FILE_FLAGS=",flags.unwrap_or(u32::MAX)as u64);
    let result=(||{
        let mode=mode.ok_or(87u32)?;let flags=flags.ok_or(87u32)?;
        // Regular private files only: allow normal archive attributes and cache hints.
        // Backup/reparse flags do not confer extra access; this namespace has no reparse points.
        // Reject overlapped, delete-on-close, no-buffering and unknown flags.
        const ALLOWED:u32=0x08000000|0x04000000|0x02000000|0x00200000|0xa7;
        if w!=0||template!=Some(0)||flags&!ALLOWED!=0{return Err(87);}
        let p=path(x).ok_or(123u32)?;
        // Read-attributes and synchronize are metadata-only rights in this private namespace.
        let access=y as u32;if access&!0xc0100080!=0{return Err(87);}
        let (h,e)=fs().open(p,access&0xc0000000,z as u32,mode)?;
        Ok((h,e))
    })();
    match result{Ok((h,e))=>{f.rax=h;LAST=e;},Err(e)=>{LAST=e;serial::metric("H5K: TRIAL_FILE_OPEN_ERROR=",e as u64);}}
}
unsafe fn file_abi_test()->bool{
    let name=ENV+0x800;let st=STACK+0x100;
    let Some(p)=phys(name,32,true)else{return false;};for(i,c)in "abi-test.zip".encode_utf16().chain(core::iter::once(0)).enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),c);}
    let mut frame=Frame{rax:0,rcx:0,rdx:0,r8:0,r9:0,rip:0,cs:0,flags:0,rsp:st,ss:0};
    // CREATE_NEW and FILE_ATTRIBUTE_NORMAL passed with deliberately nonzero padding.
    if !write64(st+40,0xdeadbeef00000001)||!write64(st+48,0xa5a5a5a500000080)||!write64(st+56,0){return false;}
    create_file(&mut frame,name,0x40000000,1,0);let h=frame.rax;if h==u64::MAX||fs().write(h,b"abi")!=Ok(3)||fs().close(h).is_err(){return false;}
    if !write64(st+40,0x1234567800000003)||!write64(st+48,0xa5a5a5a508000080){return false;}
    create_file(&mut frame,name,0x80000000,1,0);let h=frame.rax;let mut b=[0u8;3];
    if h==u64::MAX||fs().read(h,&mut b)!=Ok(3)||&b!=b"abi"||fs().close(h).is_err(){return false;}
    if !write64(st+48,0x40000080){return false;}create_file(&mut frame,name,0x80000000,1,0);
    frame.rax==u64::MAX&&LAST==87
}
// Windows FILETIME is UTC 100ns ticks since 1601-01-01. The private
// compatibility environment uses UTC for its local time zone.
const FT_SECOND:u64=10_000_000;
fn leap(y:u32)->bool{y%4==0&&(y%100!=0||y%400==0)}
fn month_days(y:u32,m:u32)->Option<u32>{match m{1|3|5|7|8|10|12=>Some(31),4|6|9|11=>Some(30),2=>Some(if leap(y){29}else{28}),_=>None}}
fn time_fields(ft:u64)->Option<[u16;8]>{
    // SYSTEMTIME supports year 1601 through 30827; bound the loop.
    let secs=ft/FT_SECOND;let mut days=secs/86400;let weekday=((days+1)%7)as u16;let mut year=1601u32;
    while year<=30827{let n=if leap(year){366}else{365};if days<n{break;}days-=n;year+=1;}
    if year>30827{return None;}let mut month=1u32;
    while month<=12{let n=month_days(year,month)?as u64;if days<n{break;}days-=n;month+=1;}
    if month>12{return None;}
    Some([year as u16,month as u16,weekday,(days+1)as u16,((secs/3600)%24)as u16,((secs/60)%60)as u16,(secs%60)as u16,((ft%FT_SECOND)/10000)as u16])
}
fn fields_time(v:[u16;8])->Option<u64>{
    let (year,month,day)=(v[0]as u32,v[1]as u32,v[3]as u32);
    if !(1601..=30827).contains(&year)||day==0||day>month_days(year,month)?||v[4]>=24||v[5]>=60||v[6]>=60||v[7]>=1000{return None;}
    let mut days=0u64;for y in 1601..year{days+=if leap(y){366}else{365};}for m in 1..month{days+=month_days(year,m)?as u64;}days+=day as u64-1;
    let secs=days*86400+v[4]as u64*3600+v[5]as u64*60+v[6]as u64;
    secs.checked_mul(FT_SECOND)?.checked_add(v[7]as u64*10000)
}
fn to_dos(ft:u64)->Option<(u16,u16)>{
    let v=time_fields(ft)?;if !(1980..=2107).contains(&v[0]){return None;}
    Some((((v[0]-1980)<<9)|(v[1]<<5)|v[3],(v[4]<<11)|(v[5]<<5)|(v[6]/2)))
}
fn from_dos(d:u16,t:u16)->Option<u64>{
    fields_time([1980+(d>>9),(d>>5)&15,0,d&31,t>>11,(t>>5)&63,(t&31)*2,0])
}
unsafe fn calendar_api(f:&mut Frame,kind:u8,x:u64,y:u64,z:u64){
    f.rax=0;let result=(||{
        match kind{
            119=>{let p=phys(x,8,false)?;let (date,time)=to_dos(ptr::read_unaligned(p.cast::<u64>()))?;let q=phys(y,2,true)?;let r=phys(z,2,true)?;if y<z+2&&z<y+2{return None;}ptr::write_unaligned(q.cast::<u16>(),date);ptr::write_unaligned(r.cast::<u16>(),time);},
            120=>{let ft=from_dos(x as u16,y as u16)?;if !write64(z,ft){return None;}},
            121=>{let p=phys(x,8,false)?;let v=time_fields(ptr::read_unaligned(p.cast::<u64>()))?;let q=phys(y,16,true)?;for(i,n)in v.iter().enumerate(){ptr::write_unaligned(q.add(i*2).cast::<u16>(),*n);}},
            122=>{let p=phys(x,16,false)?;let mut v=[0u16;8];for(i,n)in v.iter_mut().enumerate(){*n=ptr::read_unaligned(p.add(i*2).cast::<u16>());}if !write64(y,fields_time(v)?){return None;}},
            _=>return None
        }Some(())
    })();if result.is_some(){f.rax=1;}else{LAST=87;}
}
unsafe fn archive_io_time_test()->bool{
    // Known epoch and leap-day cases; reject invalid DOS date and pre-1980.
    let epoch=116444736000000000u64;
    if time_fields(epoch)!=Some([1970,1,4,1,0,0,0,0])||to_dos(epoch).is_some()||fields_time([1900,2,0,29,0,0,0,0]).is_some(){return false;}
    let v=[2024,2,0,29,23,59,59,123];let Some(ft)=fields_time(v)else{return false;};
    let Some((date,time))=to_dos(ft)else{return false;};
    if date!=((44<<9)|(2<<5)|29)||time!=((23<<11)|(59<<5)|29)||from_dos(date,time)!=fields_time([2024,2,0,29,23,59,58,0])||from_dos(0,0).is_some(){return false;}
    let src=ENV+0x800;let dd=ENV+0x810;let dt=ENV+0x812;let st=STACK+0x100;let count=ENV+0x820;
    let mut frame=Frame{rax:0,rcx:0,rdx:0,r8:0,r9:0,rip:0,cs:0,flags:0,rsp:st,ss:0};
    if !write64(src,ft){return false;}calendar_api(&mut frame,119,src,dd,dt);
    let (Some(pd),Some(pt))=(phys(dd,2,false),phys(dt,2,false))else{return false;};
    if frame.rax!=1||ptr::read_unaligned(pd.cast::<u16>())!=date||ptr::read_unaligned(pt.cast::<u16>())!=time{return false;}
    calendar_api(&mut frame,120,date as u64,time as u64,src);if frame.rax!=1{return false;}
    let Some(p)=phys(src,8,false)else{return false;};if Some(ptr::read_unaligned(p.cast::<u64>()))!=from_dos(date,time){return false;}
    // Real 1 MiB read request against a tiny file; confirm short read, EOF and untouched tail.
    let Some(path)=crate::trialfs::Path::parse(b"short-read.txt")else{return false;};
    if !fs().put(path,b"abc"){return false;}let Ok((h,_))=fs().open(path,0x80000000,1,3)else{return false;};
    let buffer=allocate(1024*1024);if buffer==0{return false;}let Some(p)=phys(buffer,1024*1024,true)else{return false;};ptr::write_bytes(p,0x5a,4);
    if !write64(st+40,0){return false;}frame.rax=0;read_write(&mut frame,false,h,buffer,1024*1024,count);
    let Some(c)=phys(count,4,false)else{return false;};
    if frame.rax!=1||ptr::read_unaligned(c.cast::<u32>())!=3||core::slice::from_raw_parts(p,4)!=b"abcZ"{return false;}
    frame.rax=0;read_write(&mut frame,false,h,buffer,1024*1024,count);
    if frame.rax!=1||ptr::read_unaligned(c.cast::<u32>())!=0{return false;}
    fs().close(h).is_ok()&&release(buffer)
}
// USER32 character navigation for the private ANSI/OEM codepages and UTF-8.
unsafe fn char_prev(cp:u32,start:u64,current:u64,flags:u32)->Option<u64>{
    if flags!=0||current<start{return None;}
    let cp=codepage(cp)?;
    let distance=usize::try_from(current-start).ok()?;
    if distance>65536{return None;}
    if distance==0{phys(start,1,false)?;return Some(start);}
    let p=phys(start,distance,false)?;
    let prefix=core::slice::from_raw_parts(p,distance);
    // Never scan past the first NUL or a validated user buffer.
    if prefix.contains(&0){return None;}
    if cp==65001{
        let s=core::str::from_utf8(prefix).ok()?;
        let (i,_)=s.char_indices().last()?;Some(start+i as u64)
    }else{Some(current-1)}
}
fn upper_unit(v:u16)->u16{
    let Some(c)=char::from_u32(v as u32)else{return v;};
    let mut upper=c.to_uppercase();let Some(first)=upper.next()else{return v;};
    if upper.next().is_some()||first as u32>65535{v}else{first as u16}
}
unsafe fn char_upper(v:u64)->Option<u64>{
    // Win32 supports a single UTF-16 code unit encoded in the low word, or a writable string.
    if v<=0xffff{return Some(upper_unit(v as u16)as u64);}
    let n=c_len(v,true)?;let p=phys(v,(n+1)*2,true)?;
    for i in 0..n{let q=p.add(i*2).cast::<u16>();ptr::write_unaligned(q,upper_unit(ptr::read_unaligned(q)));}
    Some(v)
}
unsafe fn user_text_test()->bool{
    let a=ENV+0x800;let Some(p)=phys(a,32,true)else{return false;};
    ptr::copy_nonoverlapping(b"A\x82Z\0".as_ptr(),p,4);
    if char_prev(437,a,a,0)!=Some(a)||char_prev(437,a,a+3,0)!=Some(a+2)||char_prev(1252,a,a+2,0)!=Some(a+1)||char_prev(1,a,a+1,0)!=Some(a)||char_prev(437,a,a+4,0).is_some()||char_prev(437,a,a+2,1).is_some()||char_prev(932,a,a+1,0).is_some(){return false;}
    ptr::copy_nonoverlapping(b"A\xc3\xa9Z\0".as_ptr(),p,5);
    if char_prev(65001,a,a+3,0)!=Some(a+1)||char_prev(65001,a,a+4,0)!=Some(a+3)||char_prev(65001,a,a+2,0).is_some(){return false;}
    for (i,c) in [0x61u16,0xe9,0,0x5a].iter().enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),*c);}
    if char_upper(0x61)!=Some(0x41)||char_upper(a)!=Some(a){return false;}
    ptr::read_unaligned(p.cast::<u16>())==0x41&&ptr::read_unaligned(p.add(2).cast::<u16>())==0xc9&&ptr::read_unaligned(p.add(4).cast::<u16>())==0&&ptr::read_unaligned(p.add(6).cast::<u16>())==0x5a
}
static mut GUI:crate::winwindow::Desktop=crate::winwindow::Desktop::new();
static mut GUI_ACTIVE:bool=false;
static mut GUI_RECT:Option<(u32,u32,u32,u32)>=None;
static mut GUI_DC:u64=0;
#[derive(Clone,Copy)]struct GuiResource{handle:u64,kind:u16,id:u16,offset:usize,len:usize}
const EMPTY_GUI_RESOURCE:GuiResource=GuiResource{handle:0,kind:0,id:0,offset:0,len:0};
static mut GUI_RESOURCES:[GuiResource;64]=[EMPTY_GUI_RESOURCE;64];
static mut GUI_RESOURCE_NEXT:u64=0x51000000;
static mut GUI_CLASS_MENU:[u64;16]=[0;16];
static mut MENUS:crate::winmenu::Menus=crate::winmenu::Menus::new();
static mut CONTROLS:crate::wincontrols::Controls=crate::wincontrols::Controls::new();
#[derive(Clone,Copy)]struct Creation{handle:u64,proc:u64,struct_at:u64,sp:u64,rip:u64,phase:u8}
const NO_CREATION:Creation=Creation{handle:0,proc:0,struct_at:0,sp:0,rip:0,phase:0};
static mut CREATIONS:[Creation;8]=[NO_CREATION;8];
static mut CREATION_DEPTH:usize=0;
#[derive(Clone,Copy)]struct ClipboardFormat{len:usize,name:[u16;256]}
const EMPTY_FORMAT:ClipboardFormat=ClipboardFormat{len:0,name:[0;256]};
static mut CLIP_FORMATS:[ClipboardFormat;32]=[EMPTY_FORMAT;32];
unsafe fn clipboard_format(v:u64,wide:bool)->Result<u32,u32>{
    let n=c_len(v,wide).ok_or(87u32)?;
    if n==0||n>255{return Err(87);}
    let mut name=[0u16;256];let step=if wide{2}else{1};
    for i in 0..n{let p=phys(v+(i*step)as u64,step,false).ok_or(87u32)?;name[i]=if wide{ptr::read_unaligned(p.cast::<u16>())}else{*p as u16};if !wide&&name[i]>127{return Err(50);}}
    let formats=&mut *ptr::addr_of_mut!(CLIP_FORMATS);
    if let Some(i)=formats.iter().position(|f|f.len==n&&f.name[..n].iter().zip(name[..n].iter()).all(|(a,b)|upper_unit(*a)==upper_unit(*b))){return Ok(0xc000+i as u32);}
    let i=formats.iter().position(|f|f.len==0).ok_or(8u32)?;formats[i]=ClipboardFormat{len:n,name};
    if ORIGINAL_GUI{serial::line("H5K: ORIGINAL_GUI_CLIPBOARD_FORMAT_REGISTERED");}Ok(0xc000+i as u32)
}
unsafe fn clipboard_format_name(id:u32,dst:u64,cap:i32)->Result<u64,u32>{
    let i=id.checked_sub(0xc000).ok_or(87u32)? as usize;
    let f=(&*ptr::addr_of!(CLIP_FORMATS)).get(i).filter(|f|f.len!=0).ok_or(87u32)?;
    if cap<=0{return Err(87);}let n=f.len.min(cap as usize-1);
    let p=phys(dst,(n+1)*2,true).ok_or(87u32)?;
    for j in 0..n{ptr::write_unaligned(p.add(j*2).cast::<u16>(),f.name[j]);}
    ptr::write_unaligned(p.add(n*2).cast::<u16>(),0);Ok(n as u64)
}
pub fn gui_rect()->Option<(u32,u32,u32,u32)>{unsafe{GUI_RECT}}
unsafe fn gui_api(f:&mut Frame,k:u8,x:u64,y:u64,z:u64,w:u64){
    if !GUI_ACTIVE{LAST=50;return;}
    let g=&mut *ptr::addr_of_mut!(GUI);
    match k{
        125=>{let Some(p)=phys(x,72,false)else{LAST=87;return;};let proc=ptr::read_unaligned(p.add(8).cast::<u64>());let name=ptr::read_unaligned(p.add(64).cast::<u64>());let mut b=[0u8;64];let Some(n)=read_ascii(name,true,&mut b)else{LAST=87;return;};if !executable(proc){LAST=87;return;}
            let mut menu=0;
            if ORIGINAL_GUI{
                let module=ptr::read_unaligned(p.add(24).cast::<u64>());
                let icon=ptr::read_unaligned(p.add(32).cast::<u64>());let cursor=ptr::read_unaligned(p.add(40).cast::<u64>());
                let menu_id=ptr::read_unaligned(p.add(56).cast::<u64>());
                if module!=BASE||icon!=0&&!gui_resource_valid(icon,14)||cursor!=0&&!gui_resource_valid(cursor,12){LAST=87;return;}
                if menu_id!=0{match gui_resource(module,menu_id,4){Ok(h)=>menu=h,Err(e)=>{LAST=e;serial::metric("H5K: ORIGINAL_GUI_CLASS_MENU_ERROR=",e as u64);return;}}}
            }
            if let Some(atom)=g.register(&b[..n],proc){GUI_CLASS_MENU[atom as usize-1]=menu;f.rax=atom as u64;}else{LAST=1410;}
        },
        126=>{let mut b=[0u8;64];let atom=if ORIGINAL_GUI&&y>0&&y<=0xffff{y as u16}else{let Some(n)=read_ascii(y,true,&mut b)else{LAST=87;return;};let a=if let Some(a)=g.atom(&b[..n]){a}else if ORIGINAL_GUI&&b[..n].eq_ignore_ascii_case(b"ToolbarWindow32"){let Some(a)=g.register(b"ToolbarWindow32",crate::wincontrols::TOOLBAR_PROC)else{LAST=8;return;};a}else{LAST=1407;return;};a};
            let (Some(px),Some(py),Some(ww),Some(hh),Some(parent),Some(menu),Some(instance),Some(param))=(arg(f,40),arg(f,48),arg(f,56),arg(f,64),arg(f,72),arg(f,80),arg(f,88),arg(f,96))else{LAST=87;return;};
            if !ORIGINAL_GUI{
                if x as u32!=0||w as u32!=0||menu!=0||instance!=0||param!=0||c_len(z,true).is_none(){LAST=87;return;}
                f.rax=g.create(atom,parent,[px as i32,py as i32,ww as i32,hh as i32]).unwrap_or(0);
            }else{
                // Application-registered classes only; native common controls remain a separate implementation.
                let child=w as u32&0x40000000!=0;
                if CREATION_DEPTH>=8||instance!=BASE||z!=0&&c_len(z,true).is_none()||child!=(parent!=0)||!child&&menu!=0&&!gui_resource_valid(menu,4){LAST=50;return;}
                if !child&&(x as u32!=0||w as u32&!0x10cf0000!=0){LAST=50;return;}
                let default=|v:u64,d:i32|if v as u32==0x80000000{d}else{v as i32};
                let rect=if child{[px as i32,py as i32,ww as i32,hh as i32]}else{[default(px,20),default(py,20),default(ww,GUI_WORK_WIDTH-40),default(hh,GUI_WORK_HEIGHT-40)]};
                let Some(h)=g.create_sync(atom,parent,rect)else{LAST=87;return;};
                let Some(proc)=g.procedure(h)else{g.destroy(h);LAST=87;return;};
                let menu=if child||menu!=0{menu}else{GUI_CLASS_MENU[atom as usize-1]};
                if proc==crate::wincontrols::TOOLBAR_PROC{if child&&g.configure(h,w as u32,x as u32,menu)&&CONTROLS.add_bar(h){f.rax=h;serial::line("H5K: ORIGINAL_GUI_TOOLBAR_OBJECT_CREATED");}else{g.discard_tree(h);LAST=87;}return;}
                if !g.configure(h,w as u32,x as u32,menu)||!begin_creation(f,h,proc,rect,w as u32,x as u32,instance,menu,parent,param,z,y){GUI.discard_tree(h);LAST=87;return;}
                if !child&&LIVE_ROOT==0{LIVE_ROOT=h;}
                serial::line(if child{"H5K: ORIGINAL_GUI_CHILD_OBJECT_CREATED"}else{"H5K: ORIGINAL_GUI_TOPLEVEL_OBJECT_CREATED"});
            }},
        127=>{let old=g.visible(x);if !g.show(x,y as u32!=0){LAST=6;return;}f.rax=old as u64;},
        128=>{if !g.visible(x)||GUI_DC!=0{LAST=6;return;}GUI_DC=x;f.rax=0x40000+x;},
        129=>{if GUI_DC==x&&x!=0&&y==0x40000+x{GUI_DC=0;f.rax=1;}else{LAST=6;}},
        130=>{if GUI_DC==0||x!=0x40000+GUI_DC{LAST=6;return;}let Some(bottom)=arg(f,40).map(|v|v as i32)else{LAST=87;return;};let (l,top,r)=(y as i32,z as i32,w as i32);let Some(rect)=g.rect(GUI_DC)else{LAST=6;return;};if l<0||top<0||r<=l||bottom<=top||r>rect[2]||bottom>rect[3]{LAST=87;return;}GUI_RECT=Some((l as u32,top as u32,(r-l)as u32,(bottom-top)as u32));f.rax=1;},
        131=>{f.rax=g.post(crate::winwindow::Message{hwnd:x,id:y as u32,wparam:z,lparam:w})as u64;},
        132=>{let Some(remove)=arg(f,40).map(|v|v as u32)else{LAST=87;return;};if y!=0||z as u32!=0||w as u32!=0||remove>1{LAST=87;return;}let Some(p)=phys(x,48,true)else{LAST=87;return;};if let Some(m)=g.peek(remove==1){ptr::write_bytes(p,0,48);ptr::write_unaligned(p.cast::<u64>(),m.hwnd);ptr::write_unaligned(p.add(8).cast::<u32>(),m.id);ptr::write_unaligned(p.add(16).cast::<u64>(),m.wparam);ptr::write_unaligned(p.add(24).cast::<u64>(),m.lparam);f.rax=1;}},
        _=>LAST=50
    }
}
// Resource directory traversal is bounded to the mapped, pinned executable.
// Integer resource IDs only; prefer US English, then neutral, then first language.
fn resource_entry(b:&[u8],root:usize,end:usize,dir:usize,id:Option<u16>)->Option<u32>{
    let at=root.checked_add(dir)?;if at.checked_add(16)?>end{return None;}
    let named=u16at(b,at+12)? as usize;let ids=u16at(b,at+14)? as usize;
    let total=named.checked_add(ids)?;if at.checked_add(16+total.checked_mul(8)?)?>end{return None;}
    let mut first=None;let mut neutral=None;
    for i in named..total{
        let p=at+16+i*8;let n=u32at(b,p)?;let v=u32at(b,p+4)?;if n>0xffff{continue;}
        if let Some(w)=id{if n==w as u32{return Some(v);}}
        else{if n==0x409{return Some(v);}if n==0{neutral=Some(v);}if first.is_none(){first=Some(v);}}
    }
    if id.is_none(){neutral.or(first)}else{None}
}
fn image_resource(b:&[u8],typ:u16,id:u16)->Option<(usize,usize)>{
    let pe=u32at(b,60)? as usize;let opt=pe.checked_add(24)?;
    if u16at(b,opt)?!=0x20b||u32at(b,opt+108)?<=2{return None;}
    let root=u32at(b,opt+128)? as usize;let size=u32at(b,opt+132)? as usize;
    let end=root.checked_add(size)?;if root==0||size<16||end>b.len(){return None;}
    let t=resource_entry(b,root,end,0,Some(typ))?;if t&0x80000000==0{return None;}
    let n=resource_entry(b,root,end,(t&0x7fffffff)as usize,Some(id))?;if n&0x80000000==0{return None;}
    let l=resource_entry(b,root,end,(n&0x7fffffff)as usize,None)?;if l&0x80000000!=0{return None;}
    let p=root.checked_add(l as usize)?;if p.checked_add(16)?>end{return None;}
    let data=u32at(b,p)? as usize;let len=u32at(b,p+4)? as usize;
    if data.checked_add(len)?>b.len(){return None;}Some((data,len))
}
unsafe fn gui_resource_valid(h:u64,typ:u16)->bool{
    if typ==4{return MENUS.valid(h);}
    (&*ptr::addr_of!(GUI_RESOURCES)).iter().any(|r|r.handle==h&&h!=0&&r.kind==typ)
}
unsafe fn gui_resource(module:u64,id:u64,typ:u16)->Result<u64,u32>{
    if id==0||id>0xffff{return Err(87);}
    let (offset,len)=if module==0&&typ==12&&matches!(id,32512..=32516|32640..=32651){(0,0)}
        else{
            if module!=BASE{return Err(87);}
            let b=core::slice::from_raw_parts(ptr::addr_of!(IMAGE.0).cast::<u8>(),IMAGE_SIZE);
            let (off,len)=image_resource(b,typ,id as u16).ok_or(1814u32)?;
            if len==0{return Err(13);}
            if typ==4{let data=&b[off..off+len];let result=MENUS.load(data).ok_or(13u32);if ORIGINAL_GUI{serial::metric("H5K: ORIGINAL_GUI_MENU_VERSION=",u16at(data,0).unwrap_or(0xffff)as u64);if let Ok(h)=result{serial::metric("H5K: ORIGINAL_GUI_MENU_ITEMS=",MENUS.count(h).unwrap_or(0)as u64);}else{serial::line("H5K: ORIGINAL_GUI_MENU_PARSE_FAILED");}}return result;}
            if typ==14||typ==12{
                let group=&b[off..off+len];let expected=if typ==14{1}else{2};
                if u16at(group,0)!=Some(0)||u16at(group,2)!=Some(expected){return Err(13);}
                let count=u16at(group,4).ok_or(13u32)? as usize;
                if count==0||count>256||6+14*count>len{return Err(13);}
                for i in 0..count{
                    let p=6+14*i;let size=u32at(group,p+8).ok_or(13u32)? as usize;
                    let child=u16at(group,p+12).ok_or(13u32)?;
                    let (_,actual)=image_resource(b,if typ==14{3}else{1},child).ok_or(1814u32)?;
                    if size==0||size!=actual{return Err(13);}
                }
            }(off,len)
        };
    if let Some(r)=(&*ptr::addr_of!(GUI_RESOURCES)).iter().find(|r|r.handle!=0&&r.kind==typ&&r.id==id as u16&&r.offset==offset&&r.len==len){return Ok(r.handle);}
    let i=(&*ptr::addr_of!(GUI_RESOURCES)).iter().position(|r|r.handle==0).ok_or(8u32)?;
    let h=GUI_RESOURCE_NEXT;GUI_RESOURCE_NEXT=GUI_RESOURCE_NEXT.checked_add(1).ok_or(8u32)?;GUI_RESOURCES[i]=GuiResource{handle:h,kind:typ,id:id as u16,offset,len};
    if ORIGINAL_GUI&&typ==14{serial::line("H5K: ORIGINAL_GUI_ICON_RESOURCE_LOADED");}
    Ok(h)
}
// Windows x64 CREATESTRUCTW in trial-owned memory. The callback returns to a
// dedicated RX trampoline, then the dispatcher resumes the original API caller.
unsafe fn begin_creation(f:&mut Frame,h:u64,proc:u64,rect:[i32;4],style:u32,ex:u32,instance:u64,menu:u64,parent:u64,param:u64,title:u64,class:u64)->bool{
    if CREATION_DEPTH>=8||!executable(proc)||f.rsp<STACK+256||f.rsp>TOP{return false;}
    let Some(sp)=f.rsp.checked_sub(184).map(|v|(v&!15)-8)else{return false;};
    let at=sp+48;
    let Some(p)=phys(at,80,true)else{return false;};ptr::write_bytes(p,0,80);
    for(i,v)in [param,instance,menu,parent].iter().enumerate(){ptr::write_unaligned(p.add(i*8).cast::<u64>(),*v);}
    for(i,v)in [rect[3],rect[2],rect[1],rect[0],style as i32].iter().enumerate(){ptr::write_unaligned(p.add(32+i*4).cast::<i32>(),*v);}
    ptr::write_unaligned(p.add(56).cast::<u64>(),title);ptr::write_unaligned(p.add(64).cast::<u64>(),class);ptr::write_unaligned(p.add(72).cast::<u32>(),ex);
    if !write64(sp,AUX+0xe40){return false;}
    CREATIONS[CREATION_DEPTH]=Creation{handle:h,proc,struct_at:at,sp:f.rsp,rip:f.rip,phase:1};CREATION_DEPTH+=1;
    f.rsp=sp;f.rip=proc;f.rcx=h;f.rdx=0x81;f.r8=0;f.r9=at;true
}
unsafe fn creation_return(f:&mut Frame,value:u64){
    if CREATION_DEPTH==0||f.rip!=AUX+0xe4a||!GUI_ACTIVE{H5_TRIAL_STATUS=4;return;}
    let i=CREATION_DEPTH-1;let c=CREATIONS[i];
    if c.phase==0||ORIGINAL_GUI&&f.rsp!=c.struct_at-40{H5_TRIAL_STATUS=4;return;}
    if c.phase==1&&value!=0{
        let sp=c.struct_at-48;if !write64(sp,AUX+0xe40){H5_TRIAL_STATUS=4;return;}
        CREATIONS[i].phase=2;f.rsp=sp;f.rip=c.proc;f.rcx=c.handle;f.rdx=1;f.r8=0;f.r9=c.struct_at;
        serial::line("H5K: GUI_NCCREATE_CALLBACK_RETURNED");return;
    }
    let accepted=c.phase==2&&value as u32!=u32::MAX;
    CREATIONS[i]=NO_CREATION;CREATION_DEPTH-=1;f.rsp=c.sp;f.rip=c.rip;f.rax=if accepted{c.handle}else{GUI.discard_tree(c.handle);0};
    if accepted{serial::line("H5K: GUI_CREATE_CALLBACK_RETURNED");}else{LAST=1407;}
}
unsafe fn load_string_resource(module:u64,id:u32,dest:u64,cap:u32)->Result<u64,u32>{
    if module!=BASE||id>0xffff{return Err(87);}
    let b=core::slice::from_raw_parts(ptr::addr_of!(IMAGE.0).cast::<u8>(),IMAGE_SIZE);
    let (off,len)=image_resource(b,6,(id/16+1)as u16).ok_or(1814u32)?;
    let end=off+len;let mut p=off;
    for i in 0..16{
        if p.checked_add(2).is_none_or(|v|v>end){return Err(13);}
        let count=u16at(b,p).ok_or(13u32)? as usize;p+=2;
        if p.checked_add(count*2).is_none_or(|v|v>end){return Err(13);}
        if i==id%16{
            if cap==0{if !write64(dest,BASE+p as u64){return Err(87);}return Ok(count as u64);}
            let n=count.min(cap as usize-1);
            let out=phys(dest,(n+1)*2,true).ok_or(87u32)?;
            ptr::copy_nonoverlapping(b.as_ptr().add(p),out,n*2);ptr::write_unaligned(out.add(n*2).cast::<u16>(),0);
            return Ok(n as u64);
        }p+=count*2;
    }Err(1814)
}
// MENUITEMINFOW x64 layout; only string/separator/normal state and submenu
// semantics are implemented, not bitmap/owner-draw or menu presentation.
unsafe fn menu_info(f:&mut Frame,k:u8,h:u64,key:u32,by_pos:bool,addr:u64){
    use crate::winmenu::{EMPTY,LABEL};
    let Some(p)=phys(addr,4,false)else{LAST=87;return;};
    let size=ptr::read_unaligned(p.cast::<u32>()) as usize;
    if size!=72&&size!=80{LAST=87;return;}
    let Some(p)=phys(addr,size,k==182)else{LAST=87;return;};
    let read32=|i:usize|ptr::read_unaligned(p.add(i).cast::<u32>());
    let read64=|i:usize|ptr::read_unaligned(p.add(i).cast::<u64>());
    let mask=read32(4);if mask&!0x157!=0{LAST=50;return;}
    let text=read64(56);let cap=read32(64) as usize;
    let string=mask&0x50!=0;
    if k==182{
        let Some(v)=MENUS.get(h,key,by_pos)else{LAST=1456;return;};
        if string&&text!=0&&cap!=0{let n=v.len.min(cap-1);let Some(out)=phys(text,(n+1)*2,true)else{LAST=87;return;};for i in 0..n{ptr::write_unaligned(out.add(2*i).cast::<u16>(),v.text[i]);}ptr::write_unaligned(out.add(n*2).cast::<u16>(),0);ptr::write_unaligned(p.add(64).cast::<u32>(),n as u32);}
        else if string{ptr::write_unaligned(p.add(64).cast::<u32>(),v.len as u32);}
        for (bit,off,val) in [(0x110,8,v.typ),(1,12,v.state),(2,16,v.id)]{if mask&bit!=0{ptr::write_unaligned(p.add(off).cast::<u32>(),val);}}
        if mask&4!=0{ptr::write_unaligned(p.add(24).cast::<u64>(),v.sub);}
        f.rax=1;return;
    }
    let mut v=if k==184{let Some(v)=MENUS.get(h,key,by_pos)else{LAST=1456;return;};v}else{EMPTY};
    if mask&0x110!=0{v.typ=read32(8);}if mask&1!=0{v.state=read32(12);}if mask&2!=0{v.id=read32(16);}if mask&4!=0{v.sub=read64(24);}
    if string{v.len=0;if text!=0{let Some(n)=c_len(text,true)else{LAST=87;return;};if n>=LABEL{LAST=122;return;}if n!=0{let Some(src)=phys(text,n*2,false)else{LAST=87;return;};for i in 0..n{v.text[i]=ptr::read_unaligned(src.add(i*2).cast::<u16>());}}v.len=n;}}
    let ok=if k==184{MENUS.set(h,key,by_pos,v)}else{MENUS.insert(h,key,by_pos,v)};
    if ok{f.rax=1;}else{LAST=87;}
}
unsafe fn toolbar_message(f:&mut Frame,h:u64,msg:u32,wp:u64,lp:u64){
    match msg{
        0x41e=>{if !CONTROLS.abi(h,wp){LAST=87;}},
        0x430=>{if wp!=0{LAST=50;return;}if let Some(old)=CONTROLS.set_images(h,lp){f.rax=old;}else{LAST=6;}},
        0x444=>{let n=wp as usize;if n==0||n>32{LAST=87;return;}let Some(p)=phys(lp,n*32,false)else{LAST=87;return;};
            let mut buttons=[crate::wincontrols::BUTTON;32];
            for (i,b) in buttons[..n].iter_mut().enumerate(){let p=p.add(i*32);b.image=ptr::read_unaligned(p.cast::<i32>());b.command=ptr::read_unaligned(p.add(4).cast::<i32>());b.state=*p.add(8);b.style=*p.add(9);b.data=ptr::read_unaligned(p.add(16).cast::<u64>());let text=ptr::read_unaligned(p.add(24).cast::<u64>());
                if text!=0{let Some(n)=c_len(text,true)else{LAST=87;return;};if n>=96{LAST=122;return;}for j in 0..n{let Some(v)=phys(text+2*j as u64,2,false)else{LAST=87;return;};b.text[j]=ptr::read_unaligned(v.cast::<u16>());}b.len=n;}}
            if CONTROLS.add_buttons(h,&buttons[..n]){f.rax=1;}else{LAST=87;}
        },
        0x418=>f.rax=CONTROLS.button_count(h).unwrap_or(0)as u64,
        0x421=>{let Some((width,height))=CONTROLS.dimensions(h)else{LAST=6;return;};let Some(r)=GUI.rect(h)else{LAST=6;return;};let w=GUI.parent(h).and_then(|p|GUI.rect(p)).map_or(width,|p|p[2]);if !GUI.move_window(h,r[0],r[1],w,height){LAST=87;}},
        _=>{LAST=50;serial::metric("H5K: TRIAL_UNSUPPORTED_TOOLBAR_MESSAGE=",msg as u64);H5_TRIAL_STATUS=4;}
    }
}
unsafe fn gui_startup_api(f:&mut Frame,k:u8,x:u64,y:u64,z:u64,w:u64){
    if !GUI_ACTIVE{LAST=50;return;}
    match k{
        153=>{ // Legacy InitCommonControls is void; v6 performs no registration here.
            COMMON_INIT_SEEN=true;serial::line("H5K: GUI_COMMON_CONTROLS_INIT_REACHED");
        },
        154=>f.rax=((GUI_DIALOG_Y<<16)|GUI_DIALOG_X)as u64,
        155=>{if x as u32!=0x30||y!=0||w!=0{LAST=50;return;}
            let Some(p)=phys(z,16,true)else{LAST=87;return;};
            for(i,v)in [0i32,0,GUI_WORK_WIDTH,GUI_WORK_HEIGHT].iter().enumerate(){ptr::write_unaligned(p.add(i*4).cast::<i32>(),*v);}
            f.rax=1;
        },
        // No COM apartment or drag/drop support yet. Return E_NOTIMPL, not S_OK.
        156=>f.rax=0x80004001,
        157=>{}, // No apartment was acquired.
        158=>{match load_string_resource(x,y as u32,z,w as u32){Ok(n)=>f.rax=n,Err(e)=>LAST=e}},
        161..=164=>{let typ=match k{161=>14,162=>12,163=>4,_=>9};match gui_resource(x,y,typ){Ok(h)=>f.rax=h,Err(e)=>LAST=e}},
        165=>{if GUI.rect(x).is_none(){LAST=6;return;}match y as u32{0x81=>f.rax=1,1|0x82|0x18=>{},_=>LAST=50}},
        166=>{let Some(r)=GUI.rect(x)else{LAST=6;return;};let Some(p)=phys(y,16,true)else{LAST=87;return;};let v=[0,0,r[2],r[3]];for(i,n)in v.iter().enumerate(){ptr::write_unaligned(p.add(i*4).cast::<i32>(),*n);}f.rax=1;},
        167=>{if GUI.rect(x).is_some(){f.rax=1;}else{LAST=6;}},
        168=>{if GUI.rect(x).is_none(){LAST=6;return;}f.rax=GUI.visible(x) as u64;},
        169|170=>{match clipboard_format(x,k==169){Ok(n)=>f.rax=n as u64,Err(e)=>LAST=e}},
        171=>{match clipboard_format_name(x as u32,y,z as i32){Ok(n)=>f.rax=n,Err(e)=>LAST=e}},
        172=>{if let Some(h)=GUI.menu(x){f.rax=h;}else{LAST=6;}},
        173=>{if y!=0&&!gui_resource_valid(y,4){LAST=6;return;}if GUI.set_menu(x,y){f.rax=1;}else{LAST=6;}},
        174=>{if let Some(h)=GUI.parent(x){f.rax=h;}else{LAST=6;}},
        175=>{if let Some(id)=GUI.id(x){f.rax=id as u32 as u64;}else{LAST=6;}},
        176=>{if let Some(v)=GUI.long(x,y as i32){f.rax=v;}else{LAST=87;}},
        177=>{if y as i32!=-21{LAST=50;return;}if let Some(v)=GUI.set_user(x,z){f.rax=v;}else{LAST=6;}},
        178=>{let Some(r)=GUI.screen_rect(x)else{LAST=6;return;};let Some(p)=phys(y,16,true)else{LAST=87;return;};for(i,n)in r.iter().enumerate(){ptr::write_unaligned(p.add(i*4).cast::<i32>(),*n);}f.rax=1;},
        179=>{f.rax=MENUS.create().unwrap_or(0);if f.rax==0{LAST=8;}},
        180=>{f.rax=MENUS.get(x,y as u32,true).map_or(0,|i|i.sub);},
        181=>{f.rax=MENUS.count(x).map_or(u32::MAX as u64,|n|n as u64);},
        182..=184=>menu_info(f,k,x,y as u32,z as u32!=0,w),
        185=>{if MENUS.destroy(x){f.rax=1;}else{LAST=1401;}},
        186|187=>{if MENUS.remove(x,y as u32,z as u32&0x400!=0,k==187){f.rax=1;}else{LAST=1456;}},
        188=>f.rax=MENUS.get(x,y as u32,true).filter(|i|i.sub==0).map_or(u32::MAX as u64,|i|i.id as u64),
        189=>{let Some(v)=MENUS.get(x,y as u32,z as u32&0x400!=0)else{f.rax=u32::MAX as u64;return;};f.rax=(v.typ|v.state|if v.sub!=0{0x10|((MENUS.count(v.sub).unwrap_or(0)as u32)<<8)}else{0})as u64;},
        190|191=>{let pos=z as u32&0x400!=0;let Some(mut v)=MENUS.get(x,y as u32,pos)else{f.rax=u32::MAX as u64;return;};let mask=if k==190{8}else{3};let prev=v.state&mask;v.state=(v.state&!mask)|(z as u32&mask);f.rax=if MENUS.set(x,y as u32,pos,v){prev as u64}else{u32::MAX as u64};},
        192=>{if GUI.move_window(x,y as i32,z as i32,w as i32,arg(f,40).unwrap_or(u64::MAX)as i32){f.rax=1;}else{LAST=87;}},
        193=>{if let Some(old)=GUI.set_focus(x){f.rax=old;}else{LAST=6;}},
        194=>f.rax=GUI.focus(),
        195=>{if GUI.rect(x).is_none(){LAST=6;return;}let Some(p)=phys(y,8,true)else{LAST=87;return;};let Some(r)=GUI.screen_rect(x)else{LAST=87;return;};let a=ptr::read_unaligned(p.cast::<i32>());let b=ptr::read_unaligned(p.add(4).cast::<i32>());let (a,b)=(a.checked_sub(r[0]),b.checked_sub(r[1]));let (Some(a),Some(b))=(a,b)else{LAST=87;return;};ptr::write_unaligned(p.cast::<i32>(),a);ptr::write_unaligned(p.add(4).cast::<i32>(),b);f.rax=1;},
        197=>{if CONTROLS.is_bar(x){toolbar_message(f,x,y as u32,z,w);}else if let Some(proc)=GUI.procedure(x).filter(|p|executable(*p)){f.rip=proc;f.rcx=x;f.rdx=y as u32 as u64;f.r8=z;f.r9=w;}else{LAST=6;}},
        198=>{match gui_resource(x,y,2){Ok(h)=>{let Some(r)=(&*ptr::addr_of!(GUI_RESOURCES)).iter().find(|r|r.handle==h)else{return;};let b=core::slice::from_raw_parts(ptr::addr_of!(IMAGE.0).cast::<u8>().add(r.offset),r.len);if crate::wincontrols::dib_size(b).is_some(){f.rax=h;}else{LAST=13;}},Err(e)=>LAST=e}},
        199=>{let r=(&mut *ptr::addr_of_mut!(GUI_RESOURCES)).iter_mut().find(|r|r.handle==x&&r.kind==2&&x!=0);if let Some(r)=r{*r=EMPTY_GUI_RESOURCE;f.rax=1;}else{LAST=6;}},
        200=>{if z as u32&!0x21!=0{LAST=50;return;}f.rax=CONTROLS.create_list(x as u32,y as u32).unwrap_or(0);if f.rax==0{LAST=87;}},
        201=>f.rax=CONTROLS.count(x).map_or(u32::MAX as u64,|n|n as u64),
        202=>{f.rax=u32::MAX as u64;let Some(r)=(&*ptr::addr_of!(GUI_RESOURCES)).iter().find(|r|r.handle==y&&r.kind==2&&y!=0)else{LAST=6;return;};let b=core::slice::from_raw_parts(ptr::addr_of!(IMAGE.0).cast::<u8>().add(r.offset),r.len);let Some((width,height))=crate::wincontrols::dib_size(b)else{LAST=13;return;};if let Some(n)=CONTROLS.add_image(x,crate::wincontrols::Image{offset:r.offset,len:r.len,mask:z as u32},width,height){f.rax=n as u64;}else{LAST=87;}},
        203=>{if CONTROLS.destroy_list(x){f.rax=1;}else{LAST=6;}},
        204=>{f.rax=u32::MAX as u64;
            if y!=0||z!=0||w!=0||phys(x,48,true).is_none(){LAST=87;return;}
            if let Some(m)=GUI.peek(true){let p=phys(x,48,true).unwrap();ptr::write_bytes(p,0,48);ptr::write_unaligned(p.cast::<u64>(),m.hwnd);ptr::write_unaligned(p.add(8).cast::<u32>(),m.id);ptr::write_unaligned(p.add(16).cast::<u64>(),m.wparam);ptr::write_unaligned(p.add(24).cast::<u64>(),m.lparam);f.rax=(m.id!=0x12)as u64;}
            else if LIVE_REQUEST&&CREATION_DEPTH==0&&CODEC_PENDING.is_none(){f.rip-=2;f.rax=if LAST_API<208{LAST_API as u64}else{1024+LAST_API as u64};H5_TRIAL_STATUS=5;}
            else{LAST=50;H5_TRIAL_STATUS=4;}
        },
        205=>{if phys(x,48,false).is_none(){LAST=87;}}, // WM_CHAR supplied by input broker
        206=>{if !GUI.post(crate::winwindow::Message{hwnd:0,id:0x12,wparam:x as u32 as u64,lparam:0}){LAST=8;}},
        207|208=>{if GUI.rect(x).is_none(){LAST=6;return;}if k==208&&y!=0&&phys(y,16,false).is_none(){LAST=87;return;}if GUI.post(crate::winwindow::Message{hwnd:x,id:0xf,wparam:0,lparam:0}){f.rax=1;}else{LAST=8;}},
        196=>{if GUI.rect(x).is_none(){LAST=6;return;}let Some(p)=phys(y,8,true)else{LAST=87;return;};let Some(r)=GUI.screen_rect(x)else{LAST=87;return;};let (Some(a),Some(b))=(ptr::read_unaligned(p.cast::<i32>()).checked_add(r[0]),ptr::read_unaligned(p.add(4).cast::<i32>()).checked_add(r[1]))else{LAST=87;return;};ptr::write_unaligned(p.cast::<i32>(),a);ptr::write_unaligned(p.add(4).cast::<i32>(),b);f.rax=1;},
        _=>LAST=50
    }
}
unsafe fn gui_startup_test()->bool{
    GUI_ACTIVE=true;
    let mut f=Frame{rax:0,rcx:0,rdx:0,r8:0,r9:0,rip:0,cs:0,flags:0,rsp:0,ss:0};
    gui_startup_api(&mut f,153,0,0,0,0);
    let common=COMMON_INIT_SEEN&&kind("COMCTL32.dll","#0011")==153&&kind("comctl32.dll","InitCommonControls")==153;
    gui_startup_api(&mut f,154,0,0,0,0);let units=f.rax==((GUI_DIALOG_Y<<16)|GUI_DIALOG_X)as u64;
    let addr=ENV+0x800;f.rax=0;gui_startup_api(&mut f,155,0x30,0,addr,0);
    let rect=phys(addr,16,false).is_some_and(|p|ptr::read_unaligned(p.add(8).cast::<i32>())==GUI_WORK_WIDTH&&ptr::read_unaligned(p.add(12).cast::<i32>())==GUI_WORK_HEIGHT);
    let geom=f.rax==1&&rect;f.rax=0;gui_startup_api(&mut f,155,0x30,0,1,0);let bad=f.rax==0&&LAST==87;
    gui_startup_api(&mut f,156,0,0,0,0);let ole=f.rax==0x80004001;
    GUI_ACTIVE=false;common&&units&&geom&&bad&&ole
}
unsafe fn gui_creation_test()->bool{
    GUI=crate::winwindow::Desktop::new();GUI_ACTIVE=true;
    let r=(||{
        if gui_resource(0,999,12).is_ok(){return false;}
        let Ok(cursor)=gui_resource(0,32644,12)else{return false;};if !gui_resource_valid(cursor,12)||gui_resource_valid(cursor,14){return false;}
        IMAGE_SIZE=4096;ptr::addr_of_mut!(PT0.0).cast::<u64>().write(ptr::addr_of!(IMAGE)as u64|5);
        let raw=ptr::addr_of_mut!(IMAGE.0).cast::<u8>();
        // First stage simulates a successful NCCREATE return; second is a real guest
        // callback returning zero via RET through the production callback trampoline.
        // First stage jumps into the same trampoline with RAX=1.
        let first=[0xb8u8,1,0,0,0,0x48,0xb9,0x40,0x0e,0x20,0,1,0,0,0,0xff,0xe1];
        ptr::copy_nonoverlapping(first.as_ptr(),raw,first.len());
        let proc=[0x31u8,0xc0,0xc3];ptr::copy_nonoverlapping(proc.as_ptr(),raw.add(32),proc.len());
        let Some(atom)=GUI.register(b"CallbackTest",BASE+32)else{return false;};
        let Some(h)=GUI.create_sync(atom,0,[0,0,100,100])else{return false;};
        let mut f=Frame{rax:0,rcx:0,rdx:0,r8:0,r9:0,rip:AUX+0xe00,cs:0,flags:0,rsp:TOP-40,ss:0};
        if !begin_creation(&mut f,h,BASE+32,[0,0,100,100],0,0,BASE,0,0,0,0,0)||GUI.peek(false).is_some(){return false;}
        enter(BASE,50)==1&&EXIT==h as u32&&CREATION_DEPTH==0&&GUI.rect(h).is_some()
    })();GUI_ACTIVE=false;GUI=crate::winwindow::Desktop::new();r
}
unsafe fn clipboard_and_nested_test()->bool{
    GUI=crate::winwindow::Desktop::new();GUI_ACTIVE=true;
    let ok=(||{
        let v=ENV+0x800;let p=phys(v,64,true).unwrap();
        for(i,c)in "FileGroupDescriptorW".encode_utf16().chain(core::iter::once(0)).enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),c);}
        let Ok(a)=clipboard_format(v,true)else{return false;};
        let q=phys(v+64,64,true).unwrap();let name=b"filegroupdescriptorw\0";ptr::copy_nonoverlapping(name.as_ptr(),q,name.len());
        if a!=0xc000||clipboard_format(v+64,false)!=Ok(a)||clipboard_format(1,true).is_ok()||clipboard_format_name(a,v+128,5)!=Ok(4){return false;}
        let out=phys(v+128,10,false).unwrap();if ptr::read_unaligned(out.cast::<u16>())!=b'F' as u16||ptr::read_unaligned(out.add(8).cast::<u16>())!=0{return false;}
        IMAGE_SIZE=4096;ptr::addr_of_mut!(PT0.0).cast::<u64>().write(ptr::addr_of!(IMAGE)as u64|5);
        let Some(atom)=GUI.register(b"Nested",BASE+32)else{return false;};
        let Some(h)=GUI.create_sync(atom,0,[0,0,100,100])else{return false;};
        let Some(ch)=GUI.create_sync(atom,h,[1,2,10,10])else{return false;};
        let mut f=Frame{rax:0,rcx:0,rdx:0,r8:0,r9:0,rip:BASE+100,cs:0,flags:0,rsp:TOP-40,ss:0};
        if !begin_creation(&mut f,h,BASE+32,[0,0,100,100],0,0,BASE,0,0,0,0,0){return false;}
        f.rip=AUX+0xe4a;creation_return(&mut f,1);
        let outer=CREATIONS[0];if outer.phase!=2{return false;}
        f.rsp-=128;f.rip=BASE+200;
        let saved=f.rsp;
        if !begin_creation(&mut f,ch,BASE+32,[1,2,10,10],0x40000000,0,BASE,77,h,0,0,0){return false;}
        f.rip=AUX+0xe4a;creation_return(&mut f,1);f.rip=AUX+0xe4a;creation_return(&mut f,0);
        if CREATION_DEPTH!=1||f.rax!=ch||f.rsp!=saved||f.rip!=BASE+200{return false;}
        f.rip=AUX+0xe4a;creation_return(&mut f,u32::MAX as u64);
        CREATION_DEPTH==0&&f.rax==0&&f.rip==BASE+100&&GUI.rect(h).is_none()&&GUI.rect(ch).is_none()
    })();GUI_ACTIVE=false;GUI=crate::winwindow::Desktop::new();ok
}
unsafe fn session_test()->bool{
    reset();GUI=crate::winwindow::Desktop::new();GUI_ACTIVE=true;LIVE_REQUEST=true;
    let data=ptr::addr_of_mut!(DATA.0).cast::<u8>();let mut a=EMPTY;a.kind=204;a.dn=10;a.dll[..10].copy_from_slice(b"user32.dll");a.nn=11;a.name[..11].copy_from_slice(b"GetMessageW");APIS[0]=a;N=1;
    let code=[0xbbu8,0x39,0,0,0,0x48,0xb9,0x00,0x18,0x20,0,1,0,0,0,0x31,0xd2,0x45,0x31,0xc0,0x45,0x31,0xc9,0x31,0xc0,0xcd,0x81,0x89,0xd8,0xc3];
    ptr::copy_nonoverlapping(code.as_ptr(),data,code.len());
    if enter(AUX,50)!=5{return false;}
    if !GUI.post(crate::winwindow::Message{hwnd:0,id:0x12,wparam:0,lparam:0}){return false;}
    H5_RESUME=1;let status=enter(0,50);let ok=status==1&&EXIT==57&&phys(ENV+0x808,4,false).is_some_and(|p|ptr::read_unaligned(p.cast::<u32>())==0x12);
    GUI_ACTIVE=false;LIVE_REQUEST=false;ok
}
unsafe fn map_fixture_pages(file:&[u8],offset:usize,size:usize)->bool{map_image_pages(file,offset,size,ptr::addr_of!(IMAGE)as u64,ptr::addr_of_mut!(PT0.0).cast::<u64>())}
unsafe fn map_image_pages(file:&[u8],offset:usize,size:usize,raw:u64,table:*mut u64)->bool{
    let Some(pe)=u32at(file,60).map(|v|v as usize)else{return false;};let opt=pe+24;
    let n=u16at(file,pe+6).unwrap_or(0)as usize;let opts=u16at(file,pe+20).unwrap_or(0)as usize;
    let headers=u32at(file,opt+60).unwrap_or(0)as usize;
    if offset+size>LIMIT||offset%4096!=0||headers>size{return false;}
    for i in 0..headers.div_ceil(4096){let j=offset/4096+i;table.add(j).write(raw+j as u64*4096|5|NX);}
    for i in 0..n{let s=opt+opts+i*40;let va=u32at(file,s+12).unwrap_or(u32::MAX)as usize;let span=u32at(file,s+8).unwrap_or(0).max(u32at(file,s+16).unwrap_or(0))as usize;let attr=u32at(file,s+36).unwrap_or(0);
        if va.checked_add(span).is_none_or(|v|v>size){return false;}
        let flags=5|if attr&0x80000000!=0{2}else{0}|if attr&0x20000000==0{NX}else{0};
        for i in va/4096..(va+span).div_ceil(4096){let j=offset/4096+i;table.add(j).write(raw+j as u64*4096|flags);}
    }true
}
pub fn run_gui_fixture()->bool{unsafe{
    const EXE:&[u8]=include_bytes!("../target/hi5-fixture/h5gui.exe");
    const DLL:&[u8]=include_bytes!("../target/hi5-fixture/h5dll.dll");
    const OFF:usize=1024*1024;
    GUI_RECT=None;GUI_DC=0;GUI_ACTIVE=false;GUI=crate::winwindow::Desktop::new();
    if !supported()||!crate::pegraph::no_special_dirs(EXE)||!crate::pegraph::no_special_dirs(DLL){serial::line("H5K: GUI_FIXTURE_LAYOUT");return false;}
    reset();
    let image=&mut *ptr::addr_of_mut!(IMAGE.0);let seen=&mut *ptr::addr_of_mut!(SEEN);
    let (exe,dll)=image.split_at_mut(OFF);
    let er=match relocate::map_at(EXE,exe,seen,Some(BASE)){Ok(r)=>r,Err(e)=>{serial::line(e);return false;}};
    let dr=match relocate::map_dll(DLL,dll,seen,BASE+OFF as u64){Ok(r)=>r,Err(e)=>{serial::line(e);return false;}};
    if imports::inspect(DLL,|_,_|{}).map_or(true,|r|r.functions!=0||r.delayed){return false;}
    let mut ok=true;let mut linked=0;let data=ptr::addr_of_mut!(DATA.0).cast::<u8>();
    let report=imports::inspect_slots(EXE,|module,symbol,slot|{
        let at=slot as usize;if at+8>er.size||at%8!=0{ok=false;return;}
        let target=if module.eq_ignore_ascii_case("h5dll.dll"){
            if let Some(v)=crate::pegraph::export(&dll[..dr.size],symbol){linked+=1;BASE+OFF as u64+v as u64}else{ok=false;return;}
        }else{
            let imports::Symbol::Name(name)=symbol else{ok=false;return;};let k=kind(module,name);let i=N;
            if !(125..=132).contains(&k)||i>=208||module.len()>48||name.len()>128{ok=false;return;}
            let mut a=EMPTY;a.dn=module.len();a.dll[..a.dn].copy_from_slice(module.as_bytes());a.nn=name.len();a.name[..a.nn].copy_from_slice(name.as_bytes());a.kind=k;
            ptr::addr_of_mut!(APIS).cast::<Api>().add(i).write(a);N+=1;let stub=[0xb8,i as u8,0,0,0,0xcd,0x81,0xc3];ptr::copy_nonoverlapping(stub.as_ptr(),data.add(i*16),stub.len());AUX+i as u64*16
        };
        exe[at..at+8].copy_from_slice(&target.to_le_bytes());
    });
    if !ok||linked!=1||report.map_or(true,|r|r.delayed)||!map_fixture_pages(EXE,0,er.size)||!map_fixture_pages(DLL,OFF,dr.size){serial::line("H5K: GUI_FIXTURE_BIND_FAILED");return false;}
    IMAGE_SIZE=OFF+dr.size;let Some(entry)=crate::pegraph::entry(EXE)else{return false;};
    GUI_ACTIVE=true;let status=enter(BASE+entry as u64,300);GUI_ACTIVE=false;
    serial::metric("H5K: GUI_PE_EXIT=",EXIT as u64);serial::metric("H5K: GUI_PE_STATUS=",status);serial::metric("H5K: GUI_PE_FAULT=",H5_TRIAL_FAULT);
    let passed=status==1&&EXIT==42&&GUI_RECT==Some((8,8,172,70))&&GUI_DC==0;
    if passed{serial::line("H5K: GUI_PE_DLL_EXECUTED");}else{GUI_RECT=None;}
    passed
}}
fn api_offset(i:usize)->usize{if i<208{i*16}else{0x3000+(i-208)*16}}
unsafe fn api_stub(i:usize)->u64{
    let id=if i<208{i as u32}else{1024+i as u32};let b=id.to_le_bytes();
    let stub=[0xb8,b[0],b[1],b[2],b[3],0xcd,0x81,0xc3];let at=api_offset(i);
    ptr::copy_nonoverlapping(stub.as_ptr(),ptr::addr_of_mut!(DATA.0).cast::<u8>().add(at),stub.len());AUX+at as u64
}
unsafe fn original_import(module:&str,symbol:imports::Symbol<'_>)->Option<u64>{
    let i=N;if i>=API_LIMIT||module.len()>48{return None;}
    let mut a=EMPTY;a.dn=module.len();a.dll[..a.dn].copy_from_slice(module.as_bytes());
    match symbol{
        imports::Symbol::Name(n)=>{if n.len()>128{return None;}a.nn=n.len();a.name[..a.nn].copy_from_slice(n.as_bytes());a.kind=kind(module,n);},
        imports::Symbol::Ordinal(n)=>{a.name[0]=b'#';for j in 0..4{a.name[1+j]=b"0123456789abcdef"[((n>>(12-j*4))&15)as usize];}a.nn=5;a.kind=kind(module,core::str::from_utf8(&a.name[..5]).ok()?);}
    }
    let name=&a.name[..a.nn];
    let target=if module.eq_ignore_ascii_case("msvcrt.dll"){match name{
        b"_acmdln"=>ENV+0x650,b"__initenv"=>ENV+0x640,b"_commode"=>ENV+0x648,b"_fmode"=>ENV+0x64c,b"_iob"=>ENV+0x700,b"_initterm"=>AUX+0xf00,_=>api_stub(i)
    }}else{api_stub(i)};
    ptr::addr_of_mut!(APIS).cast::<Api>().add(i).write(a);N+=1;Some(target)
}
unsafe fn original_crt_env()->bool{
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();
    ptr::write_bytes(d.add(0x1400),0,256);ptr::write_bytes(d.add(0x1500),0,128);ptr::write_bytes(d.add(0x1600),0,64);
    for(i,c)in "C:\\7zFM.exe".encode_utf16().enumerate(){ptr::write_unaligned(d.add(0x1400+i*2).cast::<u16>(),c);}
    let b=b"C:\\7zFM.exe\0";ptr::copy_nonoverlapping(b.as_ptr(),d.add(0x1500),b.len());
    if !write64(ENV+0x600,ENV+0x500)||!write64(ENV+0x650,ENV+0x500)||!write64(ENV+0x640,ENV+0x620){return false;}
    ptr::write_unaligned(d.add(0x1370).cast::<u16>(),22);ptr::write_unaligned(d.add(0x1372).cast::<u16>(),24);
    ARGC=1;
    for(start,end,off)in [(h5_initterm_start as usize,h5_initterm_end as usize,0xf00usize),(h5_cleanup_start as usize,h5_cleanup_end as usize,0xd00usize)]{
        let Some(n)=end.checked_sub(start)else{return false;};if n>256{return false;}ptr::copy_nonoverlapping(start as *const u8,d.add(off),n);
    }true
}
unsafe fn startup_api(f:&mut Frame,k:u8,x:u64,y:u64,z:u64,w:u64){
    match k{
        133=>{if let Some(n)=c_len(x,true){f.rax=n as u64;}else{LAST=87;}},
        134=>{if let Some(p)=phys(x,104,true){ptr::write_bytes(p,0,104);ptr::write_unaligned(p.cast::<u32>(),104);ptr::write_unaligned(p.add(80).cast::<u64>(),u64::MAX);ptr::write_unaligned(p.add(88).cast::<u64>(),1);ptr::write_unaligned(p.add(96).cast::<u64>(),2);}else{LAST=87;}},
        135=>f.rax=0x409, // compatibility locale, not the host locale
        136=>f.rax=0x70000000,
        137=>{if x!=0x70000000||y as u32&!8!=0{LAST=87;return;}let a=allocate(z as usize);if a!=0&&y as u32&8!=0{if let Some(p)=phys(a,(z as usize).max(1),true){ptr::write_bytes(p,0,(z as usize).max(1));}}f.rax=a;},
        138=>{if x==0x70000000&&y as u32==0&&release(z){f.rax=1;}else{LAST=87;}},
        139=>load_library_api(f,x,false),
        140=>{if y!=0||z as u32&!0x1800!=0{LAST=50;}else{load_library_api(f,x,true);}},
        141=>PRNG=x as u32,
        142=>{PRNG=PRNG.wrapping_mul(214013).wrapping_add(2531011);f.rax=((PRNG>>16)&0x7fff)as u64;},
        143=>{let b=b"C:\\Windows";let n=b.len();if y as usize<=n{f.rax=(n+1)as u64;}else if let Some(p)=phys(x,(n+1)*2,true){for(i,v)in b.iter().enumerate(){ptr::write_unaligned(p.add(2*i).cast::<u16>(),*v as u16);}ptr::write_unaligned(p.add(n*2).cast::<u16>(),0);f.rax=n as u64;}else{LAST=87;}},
        144=>{if x==0||path(x).is_some_and(|p|p.bytes().is_empty()){f.rax=3;}else{f.rax=1;}},
        145=>{if x as u32&!0x40!=0{LAST=50;return;}let a=allocate(y as usize);if a!=0&&x as u32&0x40!=0{if let Some(p)=phys(a,(y as usize).max(1),true){ptr::write_bytes(p,0,(y as usize).max(1));}}f.rax=a;},
        146=>{f.rax=if release(x){0}else{LAST=6;x};},
        147=>{if !GUI_ACTIVE{LAST=50;return;}let Some(p)=phys(x,48,false)else{LAST=87;return;};let h=ptr::read_unaligned(p.cast::<u64>());let msg=ptr::read_unaligned(p.add(8).cast::<u32>());let wp=ptr::read_unaligned(p.add(16).cast::<u64>());let lp=ptr::read_unaligned(p.add(24).cast::<u64>());let Some(proc)=GUI.procedure(h)else{LAST=6;return;};if !executable(proc){LAST=87;return;}f.rip=proc;f.rcx=h;f.rdx=msg as u64;f.r8=wp;f.r9=lp;},
        148=>{if !GUI_ACTIVE||!executable(x){LAST=87;return;}let Some(lp)=arg(f,40)else{LAST=87;return;};f.rip=x;f.rcx=y;f.rdx=z as u32 as u64;f.r8=w;f.r9=lp;},
        _=>LAST=50
    }
}
/// The original executable is pinned, and receives only an isolated RAM namespace.
/// This is an entry-point diagnostic, not a persistent interactive GUI session.
pub fn original_gui_probe()->bool{original_gui_start(false)}
pub fn original_gui_live()->bool{original_gui_start(true)&&live_active()}
fn original_gui_start(live:bool)->bool{unsafe{
    const EXE:&[u8]=include_bytes!("../target/hi5-fixture/7zfm.exe");
    const DLL:&[u8]=include_bytes!("../target/hi5-fixture/7z.dll");
    const EXE_PIN:&[u8;32]=include_bytes!("../target/hi5-fixture/7zfm.exe.sha256");
    const DLL_PIN:&[u8;32]=include_bytes!("../target/hi5-fixture/7z.dll.sha256");
    GUI_STARTUP_RESULT=0;
    if sha256(EXE)!=*EXE_PIN||sha256(DLL)!=*DLL_PIN||!self_test(){serial::line("H5K: ORIGINAL_GUI_PIN_OR_CONTAINMENT_FAILED");return false;}
    if relocate::inspect_codec(DLL).is_err(){serial::line("H5K: ORIGINAL_GUI_CODEC_STAGE_FAILED");return false;}
    reset();GUI_RECT=None;GUI_DC=0;GUI=crate::winwindow::Desktop::new();ORIGINAL_GUI=true;LIVE_REQUEST=live;
    let result=(||->Result<u64,&'static str>{
        if !original_crt_env(){return Err("GUI CRT setup");}
        let image=&mut *ptr::addr_of_mut!(IMAGE.0);let r=relocate::map_at(EXE,image,&mut *ptr::addr_of_mut!(SEEN),Some(BASE))?;
        // TLS callbacks are not silently skipped. Delay thunks are bound eagerly below.
        let pe=u32at(EXE,60).ok_or("GUI PE")?as usize;let opt=pe+24;
        if u32at(EXE,opt+108).unwrap_or(0)>9&&(u32at(EXE,opt+112+9*8)!=Some(0)||u32at(EXE,opt+116+9*8)!=Some(0)){return Err("GUI TLS unsupported");}
        IMAGE_SIZE=r.size;if !map_fixture_pages(EXE,0,r.size){return Err("GUI page permissions");}
        // Validate the real pinned binary's main menu before attempting its entry point.
        let (off,len)=image_resource(&image[..r.size],4,71).ok_or("Original main menu missing")?;
        let h=MENUS.load(&image[off..off+len]).ok_or("Original main menu parse failed")?;
        let file=MENUS.get(h,0,true).ok_or("Original File menu missing")?;
        if file.id!=500||file.sub==0||MENUS.count(file.sub).unwrap_or(0)<10{return Err("Original File menu structure");}
        MENUS.destroy(h);serial::line("H5K: ORIGINAL_GUI_PINNED_MENU_OK");
        let mut ok=true;
        let mut bind=|module:&str,symbol:imports::Symbol<'_>,slot:u32|{
            let at=slot as usize;if at%8!=0||at.checked_add(8).is_none_or(|n|n>r.size){ok=false;return;}
            let Some(v)=original_import(module,symbol)else{ok=false;return;};image[at..at+8].copy_from_slice(&v.to_le_bytes());
        };
        let report=imports::inspect_slots(EXE,&mut bind)?;
        let delayed=imports::delay_slots(EXE,&mut bind)?;
        if !ok||report.functions!=292||N!=292+delayed as usize{return Err("GUI import layout");}
        serial::metric("H5K: ORIGINAL_GUI_STATIC_IMPORTS=",report.functions as u64);serial::metric("H5K: ORIGINAL_GUI_DELAY_IMPORTS=",delayed as u64);
        serial::line("H5K: ORIGINAL_GUI_ENTER");GUI_ACTIVE=true;
        let status=enter(BASE+crate::pegraph::entry(EXE).ok_or("GUI entry")?as u64,500);
        LIVE=live&&status==5&&LIVE_ROOT!=0&&GUI.visible(LIVE_ROOT)&&CREATION_DEPTH==0;GUI_ACTIVE=LIVE;
        if LIVE{serial::line("H5K: ORIGINAL_GUI_LIVE_WAIT");}
        serial::metric("H5K: ORIGINAL_GUI_STATUS=",status);serial::metric("H5K: ORIGINAL_GUI_CALLS=",CALLS);serial::metric("H5K: ORIGINAL_GUI_EXIT=",EXIT as u64);
        serial::metric("H5K: ORIGINAL_GUI_FAULT=",H5_TRIAL_FAULT);serial::metric("H5K: ORIGINAL_GUI_RIP=",H5_TRIAL_RIP);
        if LAST_API<N{let a=ptr::addr_of!(APIS).cast::<Api>().add(LAST_API).read();serial::write("H5K: ORIGINAL_GUI_LAST_API|");serial::write(core::str::from_utf8(&a.dll[..a.dn]).unwrap_or("?"));serial::write("|");serial::line(core::str::from_utf8(&a.name[..a.nn]).unwrap_or("?"));}
        Ok(CALLS)
    })();
    if !LIVE{ORIGINAL_GUI=false;GUI_ACTIVE=false;GUI_RECT=None;GUI_DC=0;LIVE_REQUEST=false;}
    match result{Ok(n) if n>0=>{GUI_STARTUP_RESULT=n;serial::line("H5K: ORIGINAL_GUI_RUNTIME_REACHED");true},Ok(_)=>{serial::line("H5K: ORIGINAL_GUI_NO_API_CALLS");false},Err(e)=>{serial::write("H5K: ORIGINAL_GUI_STAGE_ERROR|");serial::line(e);false}}
}}
pub fn original_gui_calls()->u64{unsafe{GUI_STARTUP_RESULT}}
pub fn live_active()->bool{unsafe{LIVE}}
pub fn live_close(){unsafe{if LIVE{LIVE=false;LIVE_REQUEST=false;ORIGINAL_GUI=false;GUI_ACTIVE=false;H5_RESUME=0;serial::line("H5K: ORIGINAL_GUI_SESSION_DISCARDED");}}}
fn live_pump(){unsafe{if !LIVE{return;}H5_RESUME=1;let status=enter(0,100);if status!=5{serial::metric("H5K: ORIGINAL_GUI_LIVE_END=",status);live_close();}}}
pub fn live_key(vk:u32,ch:Option<u8>){unsafe{if !LIVE{return;}let h=if GUI.focus()!=0{GUI.focus()}else{LIVE_ROOT};GUI.post(crate::winwindow::Message{hwnd:h,id:0x100,wparam:vk as u64,lparam:1});if let Some(ch)=ch{GUI.post(crate::winwindow::Message{hwnd:h,id:0x102,wparam:ch as u64,lparam:1});}GUI.post(crate::winwindow::Message{hwnd:h,id:0x101,wparam:vk as u64,lparam:0xc0000001});live_pump();}}
fn label_ascii(src:&[u16],out:&mut[u8])->usize{let mut n=0;for &v in src{if n==out.len(){break;}if v==38{continue;}out[n]=if (32..127).contains(&v){v as u8}else{b'?'};n+=1;}n}
pub fn live_draw(c:&crate::ui::Canvas,x:usize,y:usize,width:usize,height:usize){unsafe{
    if !LIVE{return;}let right=x.saturating_add(width);let bottom=y.saturating_add(height);c.rect(x,y,width,height,[245,245,245]);
    let menu=GUI.menu(LIVE_ROOT).unwrap_or(0);
    for i in 0..MENUS.count(menu).unwrap_or(0).min(width/75){if let Some(v)=MENUS.get(menu,i as u32,true){let mut b=[0;128];let n=label_ascii(&v.text[..v.len],&mut b);if let Ok(t)=core::str::from_utf8(&b[..n]){c.text_limit(x+i*75+4,y,t,[0,0,0],false,right);}}}
    if let Some(h)=CONTROLS.bar_at(0){for i in 0..CONTROLS.button_count(h).unwrap_or(0).min(width/64){if let Some((v,img))=CONTROLS.button(h,i){
        let bx=x+i*64;let by=y+30;c.rect(bx,by,62,66,[224,224,224]);
        if let Some(img)=img{if img.offset.checked_add(img.len).is_some_and(|e|e<=IMAGE_SIZE){let data=core::slice::from_raw_parts(ptr::addr_of!(IMAGE.0).cast::<u8>().add(img.offset),img.len);if let Some((w,h))=crate::wincontrols::dib_size(data){for yy in 0..h.min(36)as usize{for xx in 0..w.min(48)as usize{if let Some(rgb)=crate::wincontrols::dib_pixel(data,xx,yy){let mask=(rgb[0]as u32)|((rgb[1]as u32)<<8)|((rgb[2]as u32)<<16);if mask!=img.mask&&by+yy+2<bottom{c.pixel(bx+6+xx,by+yy+2,rgb);}}}}}}}
        let mut b=[0;96];let n=label_ascii(&v.text[..v.len],&mut b);if let Ok(t)=core::str::from_utf8(&b[..n]){c.text_limit(bx+2,by+36,t,[0,0,0],false,(bx+62).min(right));}
    }}}
    if LIVE_MENU!=0{let n=MENUS.count(LIVE_MENU).unwrap_or(0).min(height.saturating_sub(30)/25);let mw=width.min(320);c.rect(x,y+30,mw,n*25,[255,255,255]);for i in 0..n{if let Some(v)=MENUS.get(LIVE_MENU,i as u32,true){let mut b=[0;128];let len=label_ascii(&v.text[..v.len],&mut b);if let Ok(t)=core::str::from_utf8(&b[..len]){c.text_limit(x+5,y+30+i*25,t,if v.state&3!=0{[130,130,130]}else{[0,0,0]},false,x+mw);}}}}
}}
pub fn live_click(x:usize,y:usize){unsafe{
    if !LIVE{return;}let root=LIVE_ROOT;
    if y<30{LIVE_MENU=MENUS.get(GUI.menu(root).unwrap_or(0),(x/75)as u32,true).map_or(0,|v|v.sub);return;}
    if LIVE_MENU!=0{let menu=LIVE_MENU;LIVE_MENU=0;if x<320{if let Some(v)=MENUS.get(menu,((y-30)/25)as u32,true){if v.sub!=0{LIVE_MENU=v.sub;return;}if v.typ&0x800==0&&v.state&3==0{GUI.post(crate::winwindow::Message{hwnd:root,id:0x111,wparam:v.id as u64,lparam:0});live_pump();}}}return;}
    if (30..96).contains(&y){if let Some(h)=CONTROLS.bar_at(0){if let Some((v,_))=CONTROLS.button(h,x/64){if v.state&4!=0{GUI.post(crate::winwindow::Message{hwnd:root,id:0x111,wparam:v.command as u16 as u64,lparam:0});live_pump();}return;}}}
    let lp=((x.min(32767)as u64)&0xffff)|((y.min(32767)as u64)<<16);GUI.post(crate::winwindow::Message{hwnd:root,id:0x201,wparam:1,lparam:lp});GUI.post(crate::winwindow::Message{hwnd:root,id:0x202,wparam:0,lparam:lp});live_pump();
}}
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
unsafe fn codec_name(v:u64,wide:bool)->bool{
    let mut b=[0u8;128];let Some(n)=read_ascii(v,wide,&mut b)else{return false;};
    b[..n].eq_ignore_ascii_case(b"7z.dll")||b[..n].eq_ignore_ascii_case(b"C:\\7z.dll")
}
unsafe fn load_codec(f:&mut Frame)->Result<(),u32>{
    if CODEC_READY{f.rax=CODEC_BASE;return Ok(());}
    if CODEC_PENDING.is_some()||sha256(CODEC_FILE)!=*CODEC_PIN{return Err(193);}
    let pe=u32at(CODEC_FILE,60).ok_or(193u32)?as usize;let opt=pe+24;
    if u32at(CODEC_FILE,opt+108).unwrap_or(0)>9&&(u32at(CODEC_FILE,opt+184)!=Some(0)||u32at(CODEC_FILE,opt+188)!=Some(0)){return Err(50);}
    let image=&mut *ptr::addr_of_mut!(CODEC_IMAGE.0);
    let result=relocate::map_dll(CODEC_FILE,image,&mut *ptr::addr_of_mut!(SEEN),CODEC_BASE).map_err(|_|193u32)?;
    let mut ok=true;
    let mut bind=|module:&str,symbol:imports::Symbol<'_>,slot:u32|{let at=slot as usize;if at%8!=0||at.checked_add(8).is_none_or(|n|n>result.size){ok=false;return;}
        let Some(v)=original_import(module,symbol)else{ok=false;return;};image[at..at+8].copy_from_slice(&v.to_le_bytes());};
    let report=imports::inspect_slots(CODEC_FILE,&mut bind).map_err(|_|193u32)?;
    imports::delay_slots(CODEC_FILE,&mut bind).map_err(|_|193u32)?;
    if !ok||report.functions!=89||!map_image_pages(CODEC_FILE,0,result.size,ptr::addr_of!(CODEC_IMAGE)as u64,ptr::addr_of_mut!(CODEC_PT.0).cast::<u64>()){return Err(193);}
    CODEC_SIZE=result.size;flush();
    let entry=u32at(CODEC_FILE,opt+16).ok_or(193u32)?as u64;
    if entry==0{CODEC_READY=true;f.rax=CODEC_BASE;return Ok(());}
    let proc=CODEC_BASE+entry;
    if !executable(proc)||f.rsp<STACK+256||f.rsp>TOP{return Err(193);}
    let sp=(f.rsp.checked_sub(96).ok_or(193u32)?&!15)-8;
    if !write64(sp,AUX+0xe60){return Err(193);}
    CODEC_PENDING=Some((f.rsp,f.rip,sp));f.rsp=sp;f.rip=proc;f.rcx=CODEC_BASE;f.rdx=1;f.r8=0;f.r9=0;
    serial::line("H5K: ORIGINAL_CODEC_DLL_ENTRY");Ok(())
}
unsafe fn codec_return(f:&mut Frame,value:u64){
    let Some((sp,rip,cb))=CODEC_PENDING.take()else{H5_TRIAL_STATUS=4;return;};
    if f.rip!=AUX+0xe6a||f.rsp!=cb+8{H5_TRIAL_STATUS=4;return;}
    f.rsp=sp;f.rip=rip;CODEC_READY=value as u32!=0;f.rax=if CODEC_READY{CODEC_BASE}else{LAST=1114;0};
    if CODEC_READY{serial::line("H5K: ORIGINAL_CODEC_DLL_INITIALIZED");}
}
unsafe fn load_library_api(f:&mut Frame,v:u64,wide:bool){
    if v==0{LAST=87;return;}if ORIGINAL_GUI&&codec_name(v,wide){if let Err(e)=load_codec(f){LAST=e;}return;}f.rax=module_handle(v,wide);
}
unsafe fn module_handle(v:u64,wide:bool)->u64{
    if v==0{return BASE;}if ORIGINAL_GUI&&codec_name(v,wide){if CODEC_READY{return CODEC_BASE;}LAST=126;return 0;}let mut b=[0u8;128];let Some(n)=read_ascii(v,wide,&mut b)else{LAST=87;return 0;};
    let s=&b[..n];if (!ORIGINAL_GUI&&s.eq_ignore_ascii_case(b"7za.exe"))||(ORIGINAL_GUI&&s.eq_ignore_ascii_case(b"7zfm.exe")){return BASE;}
    if let Some(i)=module_id(s){return MOD_BASE+0x10000*i as u64;}
    LAST=126;serial::write("H5K: TRIAL_MODULE_MISSING|");serial::line(core::str::from_utf8(s).unwrap_or("?"));0
}
unsafe fn proc_address(h:u64,v:u64)->u64{
    if h==CODEC_BASE&&CODEC_READY{let mut b=[0u8;128];let symbol=if v>0&&v<=0xffff{imports::Symbol::Ordinal(v as u16)}else{let Some(n)=read_ascii(v,false,&mut b)else{LAST=127;return 0;};let Ok(name)=core::str::from_utf8(&b[..n])else{LAST=127;return 0;};imports::Symbol::Name(name)};let image=core::slice::from_raw_parts(ptr::addr_of!(CODEC_IMAGE.0).cast::<u8>(),CODEC_SIZE);if let Some(off)=crate::pegraph::export(image,symbol){let at=CODEC_BASE+off as u64;if executable(at){serial::line("H5K: ORIGINAL_CODEC_EXPORT_RESOLVED");return at;}}LAST=127;return 0;}
    let Some(i)=h.checked_sub(MOD_BASE).filter(|n|n%0x10000==0).map(|n|(n/0x10000)as usize).filter(|i|*i<MODULES.len())else{LAST=6;return 0;};
    let dll=MODULES[i];let mut b=[0u8;128];let Some(n)=read_ascii(v,false,&mut b)else{LAST=127;return 0;};
    let name=core::str::from_utf8(&b[..n]).unwrap_or("");
    serial::write("H5K: TRIAL_DYNAMIC|");serial::write(dll);serial::write("|");serial::line(name);
    // Return only implemented exports, never a successful result for an unknown API.
    for j in 0..N{let a=ptr::addr_of!(APIS).cast::<Api>().add(j).read();
        if a.dll[..a.dn].eq_ignore_ascii_case(dll.as_bytes())&&a.name[..a.nn]==b[..n]&&a.kind!=0{return AUX+api_offset(j)as u64;}
    }
    let k=kind(dll,name);if k==0||N>=if ORIGINAL_GUI{API_LIMIT}else{208}{LAST=127;return 0;}
    let mut a=EMPTY;a.dn=dll.len();a.dll[..a.dn].copy_from_slice(dll.as_bytes());a.nn=n;a.name[..n].copy_from_slice(&b[..n]);a.kind=k;
    let j=N;N+=1;ptr::addr_of_mut!(APIS).cast::<Api>().add(j).write(a);
    api_stub(j)
}
unsafe fn capture(v:u64,n:usize)->bool{
    if n==0{return true;}if n>OUTPUT_CAP-OUT_LEN{return false;}let Some(p)=phys(v,n,false)else{return false;};
    ptr::copy_nonoverlapping(p,ptr::addr_of_mut!(OUT).cast::<u8>().add(OUT_LEN),n);OUT_LEN+=n;true
}
#[no_mangle]unsafe extern "efiapi" fn h5_trial_dispatch(f:*mut Frame){
    let f=&mut *f;let raw=f.rax as usize;let id=if raw>=1232&&raw<1024+API_LIMIT{raw-1024}else{raw};if raw==256{EXIT=f.rcx as u32;H5_TRIAL_STATUS=1;return;}
    if raw==258{let value=f.rcx;creation_return(f,value);return;}
    if raw==259{let value=f.rcx;codec_return(f,value);return;}
    if raw==257{if !CLEANUP_ACTIVE||ONEXIT_LEN!=0{H5_TRIAL_STATUS=4;return;}CLEANUP_ACTIVE=false;if f.rdx==1{EXIT=f.rcx as u32;H5_TRIAL_STATUS=1;}else{f.rax=0;}return;}if id>=N||raw>=208&&raw<256||raw>=258&&raw<1232{H5_TRIAL_STATUS=4;return;}
    CALLS+=1;LAST_API=id;
    let a=ptr::addr_of!(APIS).cast::<Api>().add(id).read();
    if CALLS<=4096{serial::write("H5K: TRIAL_CALL|");serial::write(core::str::from_utf8(&a.dll[..a.dn]).unwrap_or("?"));serial::write("|");serial::line(core::str::from_utf8(&a.name[..a.nn]).unwrap_or("?"));}
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
        10=>{if z<=HEAP_BYTES as u64{if z==0{f.rax=x;}else if let(Some(d),Some(s))=(phys(x,z as usize,true),phys(y,z as usize,false)){ptr::copy(s,d,z as usize);f.rax=x;}else{abort_import(id);}}else{abort_import(id);}},
        11=>{if z<=HEAP_BYTES as u64{if z==0{f.rax=x;}else if let Some(d)=phys(x,z as usize,true){ptr::write_bytes(d,y as u8,z as usize);f.rax=x;}else{abort_import(id);}}else{abort_import(id);}},
        12=>{if let Some(n)=c_len(x,false){f.rax=n as u64;}else{abort_import(id);}},
        13=>{if matches!(x,1|2){APP_TYPE=x as u32;}else{abort_import(id);}},
        14=>{if phys(x,4,true).is_some()&&phys(y,8,true).is_some()&&phys(z,8,true).is_some(){write32(x,ARGC);write64(y,ENV+0x600);write64(z,ENV+0x620);f.rax=0;}else{abort_import(id);}},
        15=>f.rax=platform::ticks().wrapping_mul(10)as u32 as u64,
        16=>f.rax=u64::MAX,
        17=>f.rax=module_handle(x,&a.name[..a.nn]==b"GetModuleHandleW"),
        18=>{if z<=HEAP_BYTES as u64{if z==0{return;}if let (Some(a),Some(b))=(phys(x,z as usize,false),phys(y,z as usize,false)){for i in 0..z as usize{let d=*a.add(i)as i32-*b.add(i)as i32;if d!=0{f.rax=d as u32 as u64;break;}}}else{abort_import(id);}}else{abort_import(id);}},
        19=>f.rax=realloc(x,y as usize),
        20|21=>{if let Some(d)=text_cmp(x,y,a.kind==21){f.rax=d as u32 as u64;}else{abort_import(id);}},
        22=>{if x==0||phys(x,1,false).is_some(){MATH_HANDLER=x;}else{abort_import(id);}},
        23=>f.rax=if x as u32<=2{1}else{0},
        24=>f.rax=match x as u32{1=>1,2=>2,_=>u64::MAX},
        25=>create_file(f,x,y,z,w),
        26=>read_write(f,false,x,y,z,w),
        27=>{if event_close(x)||token_close(x){f.rax=1;}else{let r=fs().close(x);bool_result(f,r);}},
        28=>{if y!=0&&!write32(y,0){LAST=87;f.rax=u32::MAX as u64;return;}match fs().size(x){Ok(v)=>f.rax=v as u64,Err(e)=>{LAST=e;f.rax=u32::MAX as u64;}}},
        29=>{let offset=if z==0{y as u32 as i32 as i64}else{let Some(p)=phys(z,4,true)else{LAST=87;f.rax=u32::MAX as u64;return;};((ptr::read_unaligned(p.cast::<i32>())as i64)<<32)|(y as u32 as i64)};
            match fs().seek(x,offset,w as u32){Ok(v)=>{f.rax=v as u64;if z!=0{write32(z,0);}},Err(e)=>{LAST=e;f.rax=u32::MAX as u64;}}},
        30=>{let r=fs().truncate(x);bool_result(f,r);},
        31|32|33=>{let Some(p)=path(x)else{LAST=123;return;};let r=match a.kind{31 if y==0=>fs().mkdir(p),31=>Err(87),32=>fs().rmdir(p),_=>fs().delete(p)};bool_result(f,r);},
        34=>{let r=path(x).ok_or(123u32).and_then(|p|fs().attrs(p));match r{Ok(v)=>f.rax=v as u64,Err(e)=>{LAST=e;f.rax=u32::MAX as u64;}}},
        35=>{if x<4{f.rax=4;}else if let Some(p)=phys(y,8,true){for(i,v)in [67u16,58,92,0].iter().enumerate(){ptr::write_unaligned(p.add(i*2).cast::<u16>(),*v);}f.rax=3;}else{LAST=87;}},
        36=>{let stdout=ENV+0x700+48;let stderr=ENV+0x700+96;if y!=stdout&&y!=stderr{abort_import(id);return;}let Some(n)=c_len(x,false)else{abort_import(id);return;};if capture(x,n){f.rax=0;}else{f.rax=u32::MAX as u64;}},
        37=>{let stdout=ENV+0x700+48;let stderr=ENV+0x700+96;if y!=stdout&&y!=stderr{abort_import(id);return;}if OUT_LEN<OUTPUT_CAP{ptr::addr_of_mut!(OUT).cast::<u8>().add(OUT_LEN).write(x as u8);OUT_LEN+=1;f.rax=x as u8 as u64;}else{f.rax=u32::MAX as u64;}},
        38=>{if x!=0&&x!=ENV+0x700+48&&x!=ENV+0x700+96{abort_import(id);}},
        39=>f.rax=0, // GetLargePageMinimum: large pages not supported
        40=>f.rax=437, // narrow OEM code page
        41=>{if !matches!(x,1|2)||!write32(y,CONSOLE_MODE){LAST=87;}else{f.rax=1;}},
        42=>{if matches!(x,1|2)&&y<=3{CONSOLE_MODE=y as u32;f.rax=1;}else{LAST=87;}},
        43=>load_library_api(f,x,true),
        44=>{if !write64(x,100){LAST=87;}else{f.rax=1;}},
        45=>{if !write64(x,platform::ticks()){LAST=87;}else{f.rax=1;}},
        46=>{if x!=u64::MAX||!write64(y,1)||!write64(z,1){LAST=87;}else{f.rax=1;}},
        47=>{if x!=u64::MAX||y!=1{LAST=87;}else{f.rax=1;}},
        48=>{if x!=0{let Some(p)=phys(x,48,true)else{LAST=87;return;};ptr::write_bytes(p,0,48);ptr::write_unaligned(p.cast::<u16>(),9);ptr::write_unaligned(p.add(4).cast::<u32>(),4096);ptr::write_unaligned(p.add(8).cast::<u64>(),BASE);ptr::write_unaligned(p.add(16).cast::<u64>(),HEAP_END-1);ptr::write_unaligned(p.add(24).cast::<u64>(),1);ptr::write_unaligned(p.add(32).cast::<u32>(),1);ptr::write_unaligned(p.add(36).cast::<u32>(),8664);ptr::write_unaligned(p.add(40).cast::<u32>(),65536);}else{abort_import(id);}},
        49=>f.rax=proc_address(x,y),
        50=>{if MODULES.iter().enumerate().any(|(i,_)|x==MOD_BASE+0x10000*i as u64){f.rax=1;}else{LAST=6;}},
        51=>f.rax=platform::ticks().wrapping_mul(10),
        52=>{let path:&[u8]=if ORIGINAL_GUI{b"C:\\7zFM.exe"}else{b"C:\\7za.exe"};let n=path.len();if x!=0&&x!=BASE{LAST=6;return;}let cap=z as u32 as usize;if cap==0{LAST=122;return;}let m=n.min(cap-1);
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
        96=>{match find_first(x,y){Ok(h)=>f.rax=h,Err(e)=>{LAST=e;f.rax=u64::MAX;}}},
        97=>{let r=find_next(x,y);bool_result(f,r);},
        98=>{let r=find_close(x);bool_result(f,r);},
        99=>{let r=file_info(x,y);bool_result(f,r);},
        100=>{let r=path(x).ok_or(123u32).and_then(|p|fs().set_attrs(p,y as u32));bool_result(f,r);},
        101=>{let r=path(x).zip(path(y)).ok_or(123u32).and_then(|(a,b)|fs().rename(a,b));bool_result(f,r);},
        102=>{let mut times=[None;3];for(i,v)in [y,z,w].iter().enumerate(){if *v!=0{let Some(p)=phys(*v,8,false)else{LAST=87;return;};times[i]=Some(ptr::read_unaligned(p.cast::<u64>()));}}let r=fs().set_times(x,times);bool_result(f,r);},
        103=>{if let (Some(p),Some(q))=(phys(x,8,false),phys(y,8,true)){ptr::copy(p,q,8);f.rax=1;}else{LAST=87;}},
        104=>{if let (Some(p),Some(q))=(phys(x,8,false),phys(y,8,false)){let a=ptr::read_unaligned(p.cast::<u64>());let b=ptr::read_unaligned(q.cast::<u64>());f.rax=if a<b{u32::MAX as u64}else if a>b{1}else{0};}else{abort_import(id);}},
        105=>{if x!=0&&path(x).is_none(){LAST=123;return;}let free=fs().free_bytes();for(v,n)in [(y,free),(z,crate::trialfs::QUOTA as u64),(w,free)]{if v!=0&&!write64(v,n){LAST=87;return;}}f.rax=1;},
        106=>{if x!=0&&path(x).is_none(){LAST=123;return;}let Some(total)=arg(f,40)else{LAST=87;return;};for(v,n)in [(y,1),(z,512),(w,(fs().free_bytes()/512)as u32),(total,(crate::trialfs::QUOTA/512)as u32)]{if v!=0&&!write32(v,n){LAST=87;return;}}f.rax=1;},
        107=>{let Some(p)=phys(x,64,true)else{LAST=87;return;};if ptr::read_unaligned(p.cast::<u32>())!=64{LAST=87;return;}ptr::write_bytes(p,0,64);ptr::write_unaligned(p.cast::<u32>(),64);let avail=HEAP_END.saturating_sub(NEXT);
            for(i,n)in [HEAP_BYTES as u64,avail,HEAP_BYTES as u64,avail,HEAP_BYTES as u64,avail,0].iter().enumerate(){ptr::write_unaligned(p.add(8+i*8).cast::<u64>(),*n);}f.rax=1;},
        108=>{if let Some(n)=c_len(x,true){f.rax=bstr_alloc(x,n);}else if x==0{f.rax=0;}},
        109=>f.rax=bstr_alloc(x,y as u32 as usize),
        110=>{if !bstr_free(x){abort_import(id);}},
        111=>{if let Some(n)=bstr_len(x){f.rax=n as u64;}else{abort_import(id);}},
        112=>f.rax=variant_clear(x)as u64,
        113=>f.rax=variant_copy(x,y)as u64,
        114=>{if fs().handle_info(x).is_ok(){f.rax=1;}else{LAST=6;}}, // private RAM flush is immediate
        115=>{if z==0{LAST=87;return;}match fs().seek(x,y as i64,w as u32){Ok(v)=>{if write64(z,v as u64){f.rax=1;}else{LAST=87;}},Err(e)=>LAST=e}},
        116=>{if write32(y,fs().size(x).unwrap_or(0)){if fs().size(x).is_ok()&&write32(y+4,0){f.rax=1;}else{LAST=6;}}else{LAST=87;}},
        117=>{let r=path(x).ok_or(123u32).and_then(|p|fs().info(p).ok_or(2u32));match r{Ok(e)=>{if y!=0{LAST=87;return;}let Some(p)=phys(z,36,true)else{LAST=87;return;};ptr::write_bytes(p,0,36);ptr::write_unaligned(p.cast::<u32>(),e.attrs);for j in 0..3{ptr::write_unaligned(p.add(4+j*8).cast::<u64>(),e.times[j]);}ptr::write_unaligned(p.add(32).cast::<u32>(),e.size);f.rax=1;},Err(e)=>LAST=e}},
        118=>format_message(f,x,y,z,w),
        119..=122=>calendar_api(f,a.kind,x,y,z),
        123=>{match char_prev(x as u16 as u32,y,z,w as u32){Some(v)=>f.rax=v,None=>{LAST=87;f.rax=0;}}},
        124=>{match char_upper(x){Some(v)=>f.rax=v,None=>{LAST=87;f.rax=0;}}},
        125..=132=>gui_api(f,a.kind,x,y,z,w),
        133..=148=>startup_api(f,a.kind,x,y,z,w),
        149=>{match event_create(x,y as u32!=0,z as u32!=0,w,&a.name[..a.nn]==b"CreateEventW"){Ok(h)=>{LAST=0;f.rax=h;},Err(e)=>LAST=e}},
        150|151=>{let r=event_set(x,a.kind==150);bool_result(f,r);},
        152=>{match event_wait(x,y as u32){Ok(v)=>f.rax=v as u64,Err(e)=>{LAST=e;f.rax=u32::MAX as u64;}}},
        153..=158=>gui_startup_api(f,a.kind,x,y,z,w),
        161..=208=>gui_startup_api(f,a.kind,x,y,z,w),
        159=>f.rax=lsa_open_restricted(y,w) as u64,
        160=>f.rax=lsa_win_error(x as u32) as u64,
        _=>{if &a.name[..a.nn]==b"_CxxThrowException"{serial::metric("H5K: TRIAL_CXX_OBJECT=",x);serial::metric("H5K: TRIAL_CXX_THROWINFO=",y);serial::metric("H5K: TRIAL_CXX_LAST_ERROR=",LAST as u64);if let Some(p)=phys(f.rsp,8,false){serial::metric("H5K: TRIAL_CXX_RETURN=",ptr::read_unaligned(p.cast::<u64>()));}}abort_import(id)}
    }
}
// Isolated trial processes have no authority to open local or remote security policy.
unsafe fn lsa_open_restricted(attributes:u64,out:u64)->u32{
    if !write64(out,0){return 0xc000000d;}
    if phys(attributes,48,false).is_none(){return 0xc000000d;}
    0xc0000022
}
fn lsa_win_error(status:u32)->u32{match status{0=>0,0xc0000022=>5,0xc000000d=>87,_=>317}}
unsafe fn lsa_test()->bool{
    let attr=ENV+0x800;let out=ENV+0x840;
    write32(attr,48)&&write64(out,u64::MAX)&&lsa_open_restricted(attr,out)==0xc0000022
      &&phys(out,8,false).is_some_and(|p|ptr::read_unaligned(p.cast::<u64>())==0)
      &&lsa_open_restricted(0,out)==0xc000000d&&lsa_open_restricted(attr,1)==0xc000000d
      &&lsa_win_error(0xc0000022)==5&&lsa_win_error(0xc000000d)==87&&lsa_win_error(0)==0
}
unsafe fn event_index(h:u64)->Option<usize>{
    if h==0{return None;}(&*ptr::addr_of!(EVENTS)).iter().position(|e|e.handle==h)
}
unsafe fn event_create(security:u64,manual:bool,signaled:bool,name:u64,wide:bool)->Result<u64,u32>{
    if security!=0{return Err(50);}
    if name!=0{match c_len(name,wide){Some(0)=>{},Some(_)=>return Err(50),None=>return Err(87)}}
    let i=(&*ptr::addr_of!(EVENTS)).iter().position(|e|e.handle==0).ok_or(4u32)?;
    let h=EVENT_NEXT;if h>=0x50000000{return Err(4);}EVENT_NEXT+=1;
    EVENTS[i]=Event{handle:h,manual,signaled};Ok(h)
}
unsafe fn event_close(h:u64)->bool{
    if let Some(i)=event_index(h){EVENTS[i]=EMPTY_EVENT;true}else{false}
}
unsafe fn event_set(h:u64,signaled:bool)->Result<(),u32>{
    let i=event_index(h).ok_or(6u32)?;EVENTS[i].signaled=signaled;Ok(())
}
unsafe fn event_wait(h:u64,timeout:u32)->Result<u32,u32>{
    let i=event_index(h).ok_or(6u32)?;
    if EVENTS[i].signaled{if !EVENTS[i].manual{EVENTS[i].signaled=false;}return Ok(0);}
    if timeout==0{return Ok(258);}
    // Do not pretend a blocking wait succeeded or manufacture an early timeout.
    Err(120)
}
unsafe fn event_test()->bool{
    let Ok(a)=event_create(0,false,true,0,true)else{return false;};
    let Ok(b)=event_create(0,true,false,0,true)else{return false;};
    if a==b||event_wait(a,0)!=Ok(0)||event_wait(a,0)!=Ok(258)||event_wait(a,u32::MAX)!=Err(120){return false;}
    if event_set(b,true)!=Ok(())||event_wait(b,0)!=Ok(0)||event_wait(b,0)!=Ok(0)||event_set(b,false)!=Ok(())||event_wait(b,0)!=Ok(258){return false;}
    if !event_close(a)||event_close(a)||event_wait(a,0)!=Err(6)||event_set(a,true)!=Err(6)||!event_close(b){return false;}
    let Ok(c)=event_create(0,false,false,0,true)else{return false;};
    c!=a&&event_create(1,false,false,0,true)==Err(50)&&event_close(c)
}
unsafe fn fs()->&'static mut crate::trialfs::Fs{if WORK_ACTIVE{&mut *ptr::addr_of_mut!(WORK)}else{&mut *ptr::addr_of_mut!(FS)}}
unsafe fn path(v:u64)->Option<crate::trialfs::Path>{
    let n=c_len(v,true)?;if n>96{return None;}let mut b=[0u8;96];for i in 0..n{
        let w=ptr::read_unaligned(phys(v+2*i as u64,2,false)?.cast::<u16>());if w>127{return None;}b[i]=w as u8;
    }crate::trialfs::Path::parse(&b[..n])
}
unsafe fn arg(f:&Frame,off:u64)->Option<u64>{Some(ptr::read_unaligned(phys(f.rsp.checked_add(off)?,8,false)?.cast::<u64>()))}
unsafe fn bool_result(f:&mut Frame,r:Result<(),u32>){match r{Ok(())=>f.rax=1,Err(e)=>{LAST=e;f.rax=0;}}}
unsafe fn read_write(f:&mut Frame,write:bool,x:u64,y:u64,z:u64,w:u64){
    let n=z as u32 as usize;
    let result=(||{
        if arg(f,40)!=Some(0)||phys(w,4,true).is_none(){return Err(87u32);}
        // A synchronous read may request more than the entire file. The filesystem
        // returns only bytes available up to EOF, rather than rejecting the request.
        if n>HEAP_BYTES{return Err(87);}
        if n==0{return if write{fs().write(x,&[])}else{fs().read(x,&mut [])};}
        let p=phys(y,n,!write).ok_or(87u32)?;
        if write{fs().write(x,core::slice::from_raw_parts(p,n))}
        else{fs().read(x,core::slice::from_raw_parts_mut(p,n))}
    })();
    match result{Ok(k)=>{write32(w,k as u32);f.rax=1;},Err(e)=>{LAST=e;write32(w,0);serial::metric("H5K: TRIAL_IO_REQUEST=",n as u64);serial::metric("H5K: TRIAL_IO_WRITE=",write as u64);serial::metric("H5K: TRIAL_IO_ERROR=",e as u64);}}
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
    if d.eq_ignore_ascii_case("kernel32.dll"){match n{"ExitProcess"=>1,"GetCurrentProcessId"|"GetCurrentThreadId"=>2,"SetLastError"=>3,"GetLastError"=>4,"GetCommandLineW"=>5,"GetStdHandle"=>6,"WriteFile"=>7,"GetTickCount"=>15,"GetCurrentProcess"=>16,"GetModuleHandleA"|"GetModuleHandleW"=>17,"CreateFileW"=>25,"ReadFile"=>26,"CloseHandle"=>27,"GetFileSize"=>28,"SetFilePointer"=>29,"SetEndOfFile"=>30,"CreateDirectoryW"=>31,"RemoveDirectoryW"=>32,"DeleteFileW"=>33,"GetFileAttributesW"=>34,"GetCurrentDirectoryW"=>35,"GetLargePageMinimum"=>39,"GetOEMCP"=>40,"GetConsoleMode"=>41,"SetConsoleMode"=>42,"LoadLibraryW"=>43,"QueryPerformanceFrequency"=>44,"QueryPerformanceCounter"=>45,"GetProcessAffinityMask"=>46,"SetProcessAffinityMask"=>47,"GetSystemInfo"|"GetNativeSystemInfo"=>48,"GetProcAddress"=>49,"FreeLibrary"=>50,"GetTickCount64"=>51,"GetModuleFileNameW"=>52,"GetSystemTimeAsFileTime"|"GetSystemTimePreciseAsFileTime"=>53,"GetActiveProcessorCount"=>54,"IsWow64Process"=>55,"VirtualAlloc"=>58,"VirtualFree"=>59,"IsProcessorFeaturePresent"=>60,"InitializeCriticalSection"=>61,"EnterCriticalSection"=>62,"LeaveCriticalSection"=>63,"DeleteCriticalSection"=>64,"InitializeCriticalSectionAndSpinCount"=>65,"TryEnterCriticalSection"=>66,"InitializeCriticalSectionEx"=>67,"GetConsoleScreenBufferInfo"=>68,"SetConsoleCtrlHandler"=>69,"GetFileType"=>71,"GetVersion"=>73,"GetVersionExA"|"GetVersionExW"=>74,"SetFileApisToOEM"=>76,"SetFileApisToANSI"=>77,"AreFileApisANSI"=>78,"GetACP"=>79,"MultiByteToWideChar"=>80,"WideCharToMultiByte"=>81,"GetLogicalDriveStringsW"=>83,"GetTempPathW"=>84,"LocalFree"=>85,"GetConsoleOutputCP"=>86,"SetDefaultDllDirectories"=>90,"FindFirstFileW"=>96,"FindNextFileW"=>97,"FindClose"=>98,"GetFileInformationByHandle"=>99,"SetFileAttributesW"=>100,"MoveFileW"=>101,"SetFileTime"=>102,"FileTimeToLocalFileTime"|"LocalFileTimeToFileTime"=>103,"CompareFileTime"=>104,"GetDiskFreeSpaceExW"=>105,"GetDiskFreeSpaceW"=>106,"GlobalMemoryStatusEx"=>107,"FlushFileBuffers"=>114,"SetFilePointerEx"=>115,"GetFileSizeEx"=>116,"GetFileAttributesExW"=>117,"FormatMessageW"=>118,"FileTimeToDosDateTime"=>119,"DosDateTimeToFileTime"=>120,"FileTimeToSystemTime"=>121,"SystemTimeToFileTime"=>122,"lstrlenW"=>133,"GetStartupInfoA"=>134,"GetSystemDefaultLangID"|"GetUserDefaultLangID"=>135,"GetProcessHeap"=>136,"HeapAlloc"=>137,"HeapFree"=>138,"LoadLibraryA"=>139,"LoadLibraryExW"=>140,"GetWindowsDirectoryW"=>143,"GetDriveTypeW"=>144,"LocalAlloc"|"GlobalAlloc"=>145,"GlobalFree"=>146,"CreateEventW"|"CreateEventA"=>149,"SetEvent"=>150,"ResetEvent"=>151,"WaitForSingleObject"=>152,_=>0}}
    else if d.eq_ignore_ascii_case("msvcrt.dll"){match n{"exit"|"_exit"=>1,"malloc"=>8,"free"=>9,"memcpy"|"memmove"=>10,"memset"=>11,"strlen"=>12,"__set_app_type"=>13,"__getmainargs"=>14,"memcmp"=>18,"realloc"=>19,"strcmp"=>20,"wcscmp"=>21,"__setusermatherr"=>22,"_isatty"=>23,"_get_osfhandle"=>24,"fputs"=>36,"fputc"=>37,"fflush"=>38,"_onexit"=>56,"_cexit"|"_c_exit"=>57,"wcsstr"=>70,"wcslen"=>133,"srand"=>141,"rand"=>142,_=>0}}else if d.eq_ignore_ascii_case("oleaut32.dll"){match n{"#0002"|"SysAllocString"=>108,"#0004"|"SysAllocStringLen"=>109,"#0006"|"SysFreeString"=>110,"#0007"|"SysStringLen"=>111,"#0009"|"VariantClear"=>112,"#000a"|"VariantCopy"=>113,_=>0}}else if d.eq_ignore_ascii_case("advapi32.dll"){match n{"OpenProcessToken"=>87,"LookupPrivilegeValueW"=>88,"AdjustTokenPrivileges"=>89,"SystemFunction036"=>91,"RegOpenKeyExW"=>92,"RegQueryValueExW"|"RegCloseKey"=>93,"GetFileSecurityW"|"SetFileSecurityW"=>94,"GetTokenInformation"=>95,"LsaOpenPolicy"=>159,"LsaNtStatusToWinError"=>160,_=>0}}else if d.eq_ignore_ascii_case("user32.dll"){match n{"CharPrevExA"=>123,"CharUpperW"=>124,"RegisterClassW"=>125,"CreateWindowExW"=>126,"ShowWindow"=>127,"GetDC"=>128,"ReleaseDC"=>129,"PostMessageW"=>131,"PeekMessageW"=>132,"DispatchMessageW"=>147,"CallWindowProcW"=>148,"GetDialogBaseUnits"=>154,"SystemParametersInfoW"|"SystemParametersInfoA"=>155,"LoadStringW"=>158,"LoadIconW"=>161,"LoadCursorW"=>162,"LoadMenuW"=>163,"LoadAcceleratorsW"=>164,"DefWindowProcW"=>165,"GetClientRect"=>166,"IsWindow"=>167,"IsWindowVisible"=>168,"RegisterClipboardFormatW"=>169,"RegisterClipboardFormatA"=>170,"GetClipboardFormatNameW"=>171,"GetMenu"=>172,"SetMenu"=>173,"GetParent"=>174,"GetDlgCtrlID"=>175,"GetWindowLongPtrW"=>176,"SetWindowLongPtrW"=>177,"GetWindowRect"=>178,"CreateMenu"|"CreatePopupMenu"=>179,"GetSubMenu"=>180,"GetMenuItemCount"=>181,"GetMenuItemInfoW"=>182,"InsertMenuItemW"=>183,"SetMenuItemInfoW"=>184,"DestroyMenu"=>185,"RemoveMenu"=>186,"DeleteMenu"=>187,"GetMenuItemID"=>188,"GetMenuState"=>189,"CheckMenuItem"=>190,"EnableMenuItem"=>191,"MoveWindow"=>192,"SetFocus"=>193,"GetFocus"=>194,"ScreenToClient"=>195,"ClientToScreen"=>196,"SendMessageW"=>197,"LoadBitmapW"=>198,"GetMessageW"=>204,"TranslateMessage"=>205,"PostQuitMessage"=>206,"UpdateWindow"=>207,"InvalidateRect"=>208,_=>0}}else if d.eq_ignore_ascii_case("comctl32.dll"){match n{"#0011"|"InitCommonControls"=>153,"ImageList_Create"=>200,"ImageList_GetImageCount"=>201,"ImageList_AddMasked"=>202,"ImageList_Destroy"=>203,_=>0}}else if d.eq_ignore_ascii_case("ole32.dll"){match n{"OleInitialize"=>156,"OleUninitialize"=>157,_=>0}}else if d.eq_ignore_ascii_case("gdi32.dll"){match n{"Rectangle"=>130,"DeleteObject"=>199,_=>0}}else if d.eq_ignore_ascii_case("ntdll.dll"){match n{"RtlInitializeCriticalSection"=>61,"RtlEnterCriticalSection"=>62,"RtlLeaveCriticalSection"=>63,"RtlDeleteCriticalSection"=>64,"RtlTryEnterCriticalSection"=>66,"RtlGetVersion"=>75,"RtlNtStatusToDosError"=>82,_=>0}}else{0}
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
    WORK_ACTIVE=false;ORIGINAL_GUI=false;COMMON_INIT_SEEN=false;GUI_RESOURCES=[EMPTY_GUI_RESOURCE;64];GUI_RESOURCE_NEXT=0x51000000;GUI_CLASS_MENU=[0;16];MENUS.reset();CONTROLS.reset();CREATIONS=[NO_CREATION;8];CREATION_DEPTH=0;CLIP_FORMATS=[EMPTY_FORMAT;32];PRNG=1;fs().reset();FIND=[EMPTY_SEARCH;16];ARGC=2;MATH_HANDLER=0;CALLS=0;LAST_API=usize::MAX;HELP_OK=false;DETAIL_LEN=0;ONEXIT_LEN=0;LOCKS=[EMPTY_LOCK;128];CONSOLE_MODE=3;CTRL_HANDLER=0;FILE_OEM=false;TOKENS=[EMPTY_TOKEN;16];TOKEN_NEXT=0x10000;EVENTS=[EMPTY_EVENT;64];EVENT_NEXT=0x40000000;CLEANUP_ACTIVE=false;DLL_POLICY=0;ptr::write_bytes(ptr::addr_of_mut!(BLOCKS).cast::<Block>(),0,2048);NEXT=HEAP;N=0;IMAGE_SIZE=0;OUT_LEN=0;LAST=0;EXIT=0;APP_TYPE=0;
    H5_TRIAL_STATUS=0;H5_TRIAL_FAULT=0;H5_TRIAL_RIP=0;H5_RESUME=0;LIVE_REQUEST=false;LIVE=false;LIVE_ROOT=0;LIVE_MENU=0;CODEC_READY=false;CODEC_SIZE=0;CODEC_PENDING=None;ptr::write_bytes(ptr::addr_of_mut!(CODEC_PT.0).cast::<u64>(),0,512);
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>()as u64;
    let t=ptr::addr_of_mut!(PT1.0).cast::<u64>();
    t.write(d|5);t.add(1).write(d+4096|7|NX);t.add(3).write(d+0x3000|5);
    for i in 16..48usize{t.add(i).write(d+i as u64*4096|7|NX);}
    for i in 64..512usize{t.add(i).write(d+i as u64*4096|7|NX);}
    let raw=ptr::addr_of!(HEAP_DATA)as u64;
    let tab=ptr::addr_of_mut!(HEAP_PT).cast::<u64>();for i in 0..HEAP_BYTES/4096{tab.add(i).write(raw+i as u64*4096|7|NX);}
    init_env();
}
unsafe fn supported()->bool{
    if !nx_available()||__cpuid(1).edx&(1<<24)==0{return false;}
    for base in [ptr::addr_of!(IMAGE)as u64,ptr::addr_of!(DATA)as u64,ptr::addr_of!(HEAP_DATA)as u64,ptr::addr_of!(CODEC_IMAGE)as u64]{if base+LIMIT as u64>1<<32||!(0..LIMIT).step_by(4096).all(|i|cpu::writable_identity(base+i as u64)){return false;}}
    let h=ptr::addr_of!(HEAP_DATA)as u64;if h+HEAP_BYTES as u64>1<<32||!(0..HEAP_BYTES).step_by(4096).all(|i|cpu::writable_identity(h+i as u64)){return false;}
    [ptr::addr_of!(PT0)as u64,ptr::addr_of!(PT1)as u64,ptr::addr_of!(CODEC_PT)as u64].iter().all(|v|cpu::writable_identity(*v))&&(0..16).all(|i|cpu::writable_identity(ptr::addr_of!(HEAP_PT)as u64+i*4096))
}
unsafe fn enter(ip:u64,budget:u64)->u64{
    let pd=ptr::addr_of_mut!(platform::USER_PD.0).cast::<u64>();let a=pd.read();let b=pd.add(1).read();
    let codec_old=pd.add(18).read();
    let oldgs=msr(0xc0000101);let oldfs=msr(0xc0000100);let efer=msr(0xc0000080);
    let cr4:u64;asm!("mov {},cr4",out(reg)cr4,options(nostack));
    // Disable SYSCALL, FSGSBASE and AVX during the single-thread trial.
    set_msr(0xc0000080,(efer|1<<11)&!1);
    asm!("mov cr4,{}",in(reg)(cr4&!((1<<16)|(1<<18))),options(nostack));
    set_msr(0xc0000101,ENV);set_msr(0xc0000100,0);
    let mut old=[0u64;16];for i in 0..16{old[i]=pd.add(i+2).read();pd.add(i+2).write((ptr::addr_of!(HEAP_PT)as u64+i as u64*4096)|7);}
    pd.write(ptr::addr_of!(PT0)as u64|7);pd.add(1).write(ptr::addr_of!(PT1)as u64|7);pd.add(18).write(ptr::addr_of!(CODEC_PT)as u64|7);flush();
    // Windows x64 entry requires a return address and 32-byte caller home area.
    let sp=TOP-40;
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();
    if H5_RESUME==0{ptr::write_unaligned(d.add((sp-AUX)as usize).cast::<u64>(),AUX+0xe00);}
    // Return trampoline: propagate the entry point's EAX as exit status.
    let ret=[0x89u8,0xc1,0xb8,0x00,0x01,0x00,0x00,0xcd,0x81,0x0f,0x0b];
    ptr::copy_nonoverlapping(ret.as_ptr(),d.add(0xe00),ret.len());
    // mov rcx,rax; mov eax,258; int 81h; ud2 (return RIP is +10)
    let callback=[0x48u8,0x89,0xc1,0xb8,0x02,0x01,0x00,0x00,0xcd,0x81,0x0f,0x0b];
    ptr::copy_nonoverlapping(callback.as_ptr(),d.add(0xe40),callback.len());
    let codec_return=[0x48u8,0x89,0xc1,0xb8,0x03,0x01,0,0,0xcd,0x81,0x0f,0x0b];ptr::copy_nonoverlapping(codec_return.as_ptr(),d.add(0xe60),codec_return.len());
    H5_TRIAL_STATUS=0;H5_TRIAL_BUDGET=budget;H5_TRIAL_ACTIVE=1;h5_trial_enter(ip,sp);H5_TRIAL_ACTIVE=0;
    pd.write(a);pd.add(1).write(b);pd.add(18).write(codec_old);for i in 0..16{pd.add(i+2).write(old[i]);}flush();set_msr(0xc0000101,oldgs);set_msr(0xc0000100,oldfs);set_msr(0xc0000080,efer);
    asm!("mov cr4,{}",in(reg)cr4,options(nostack));
    H5_TRIAL_STATUS
}
pub fn self_test()->bool{unsafe{
    WORK_ACTIVE=false;
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
    let a=allocate(1024);let b=allocate(1024);if a!=HEAP||b!=HEAP+1024||!release(a){return false;}
    let small_a=allocate(128);let small_b=allocate(256);if small_a!=a||small_b!=a+128||!release(small_a)||!release(small_b)||!release(b)||NEXT!=HEAP{return false;}
    let a=allocate_aligned(4096,4096);if a!=HEAP||!release(a)||NEXT!=HEAP{return false;}
    serial::line("H5K: TRIAL_HEAP_REUSE_OK");
    reset();if !nls_test(){return false;}serial::line("H5K: TRIAL_NLS_CONVERSION_OK");
    reset();if !file_abi_test(){return false;}serial::line("H5K: TRIAL_FILE_ABI_OK");
    reset();if !archive_io_time_test(){return false;}serial::line("H5K: TRIAL_ARCHIVE_IO_TIME_OK");
    reset();if !user_text_test(){return false;}serial::line("H5K: TRIAL_USER_TEXT_OK");
    reset();if !event_test(){return false;}serial::line("H5K: TRIAL_EVENT_OBJECTS_OK");
    reset();if !gui_startup_test(){return false;}serial::line("H5K: TRIAL_GUI_STARTUP_ABI_OK");
    reset();if !lsa_test(){return false;}serial::line("H5K: TRIAL_RESTRICTED_LSA_OK");
    reset();if !gui_creation_test(){return false;}serial::line("H5K: TRIAL_GUI_CREATION_CALLBACK_OK");
    reset();if !clipboard_and_nested_test(){return false;}serial::line("H5K: TRIAL_CLIPBOARD_NESTED_CREATION_OK");
    if !crate::winmenu::self_test(&mut *ptr::addr_of_mut!(MENUS)){return false;}serial::line("H5K: TRIAL_MENU_TREE_OK");
    if !crate::wincontrols::self_test(&mut *ptr::addr_of_mut!(CONTROLS)){return false;}serial::line("H5K: TRIAL_TOOLBAR_BITMAP_MODEL_OK");
    if !session_test(){return false;}serial::line("H5K: TRIAL_GUI_SESSION_RESUME_OK");
    // The second RX stub page must not collide with the reserved exit/cleanup IDs.
    for i in [256usize,257,API_LIMIT-1]{reset();N=i+1;let mut a=EMPTY;a.kind=2;ptr::addr_of_mut!(APIS).cast::<Api>().add(i).write(a);let entry=api_stub(i);if enter(entry,50)!=1||EXIT!=1{return false;}}
    serial::line("H5K: TRIAL_EXTENDED_STUBS_OK");
    reset();
    // Store into all four x64 home slots, then return 42 through the entry trampoline.
    let code=[0x48u8,0xc7,0x44,0x24,0x08,1,0,0,0,0x48,0xc7,0x44,0x24,0x10,2,0,0,0,0x48,0xc7,0x44,0x24,0x18,3,0,0,0,0x48,0xc7,0x44,0x24,0x20,4,0,0,0,0xb8,42,0,0,0,0xc3];
    ptr::copy_nonoverlapping(code.as_ptr(),ptr::addr_of_mut!(DATA.0).cast::<u8>(),code.len());
    if enter(AUX,50)!=1||EXIT!=42{return false;}serial::line("H5K: TRIAL_WIN64_ENTRY_OK");true
}}
pub const PACKAGE_SIZE:usize=1328640;
pub fn verified(b:&[u8])->bool{b.len()==PACKAGE_SIZE&&sha256(b)==*PIN}
pub fn help_ok()->bool{unsafe{HELP_OK}}
pub const SAMPLE:&[u8]=b"Hi5OS genuine Windows archive qualification\r\nSecond line: ZIP and 7z round trip.\r\n";
pub fn workspace_ready(){unsafe{if !WORK_READY{WORK.reset();WORK_READY=true;}}}
pub fn workspace_load(b:&[u8])->bool{unsafe{workspace_ready();WORK.unpack(b)}}
pub fn workspace_pack(out:&mut[u8])->Option<usize>{unsafe{workspace_ready();WORK.pack(out)}}
pub fn workspace_seed(data:&[u8])->bool{unsafe{workspace_ready();WORK.put(crate::trialfs::Path::parse(b"notes.txt").unwrap(),data)}}
pub fn workspace_large_seed()->bool{unsafe{
    workspace_ready();let p=|v:&[u8]|crate::trialfs::Path::parse(v).unwrap();let dir=p(b"batch");
    if WORK.info(dir).is_none()&&WORK.mkdir(dir).is_err(){return false;}
    let Ok((h,_))=WORK.open(p(b"batch\\large.bin"),0xc0000000,0,2)else{return false;};
    let mut b=[0u8;4096];for i in 0..48usize{for j in 0..b.len(){b[j]=((i*17+j*31+(j>>8))&255)as u8;}if WORK.write(h,&b)!=Ok(b.len()){let _=WORK.close(h);return false;}}
    WORK.close(h).is_ok()&&WORK.put(p(b"batch\\small.txt"),b"Hi5OS multi-file test\r\n")
}}
pub fn workspace_update_seed()->bool{unsafe{workspace_ready();WORK.put(crate::trialfs::Path::parse(b"batch\\small.txt").unwrap(),b"Hi5OS updated content\r\n")}}
pub fn workspace_verify()->bool{unsafe{workspace_ready();let p=|s:&[u8]|crate::trialfs::Path::parse(s).unwrap();let Some(src)=WORK.contents(p(b"notes.txt"))else{return false;};WORK.contents(p(b"outzip\\notes.txt"))==Some(src)&&WORK.contents(p(b"out7z\\notes.txt"))==Some(src)}}
pub fn command_ok()->bool{unsafe{H5_TRIAL_STATUS==1&&EXIT==0}}
pub fn exit_code()->u32{unsafe{EXIT}}
pub fn output_bytes()->&'static[u8]{unsafe{core::slice::from_raw_parts(ptr::addr_of!(OUT).cast::<u8>(),OUT_LEN)}}
unsafe fn set_args(cmd:&[u8])->bool{
    if cmd.len()>108||cmd.iter().any(|c|!(32..127).contains(c)||*c==b'"'){return false;}
    let mut args=cmd.split(|c|*c==b' ').filter(|s|!s.is_empty());if args.next()!=Some(b"7za".as_slice()){return false;}
    let d=ptr::addr_of_mut!(DATA.0).cast::<u8>();ptr::write_bytes(d.add(0x1400),0,256);ptr::write_bytes(d.add(0x1500),0,128);ptr::write_bytes(d.add(0x1600),0,64);
    let mut line=[0u8;120];line[..7].copy_from_slice(b"7za.exe");let mut n=7;let mut pos=0;let mut count=0;
    for arg in core::iter::once(b"7za.exe".as_slice()).chain(args){
        if count>=7||pos+arg.len()+1>128{return false;}
        ptr::copy_nonoverlapping(arg.as_ptr(),d.add(0x1500+pos),arg.len());ptr::write_unaligned(d.add(0x1600+count*8).cast::<u64>(),ENV+0x500+pos as u64);pos+=arg.len()+1;
        if count!=0{if n+1+arg.len()>line.len(){return false;}line[n]=b' ';n+=1;line[n..n+arg.len()].copy_from_slice(arg);n+=arg.len();}count+=1;
    }
    for(i,c)in line[..n].iter().enumerate(){ptr::write_unaligned(d.add(0x1400+2*i).cast::<u16>(),*c as u16);}
    ptr::write_unaligned(d.add(0x1370).cast::<u16>(),(n*2)as u16);ptr::write_unaligned(d.add(0x1372).cast::<u16>(),(n*2+2)as u16);ARGC=count as u32;true
}
pub fn run(b:&[u8])->Result<u64,&'static str>{run_command(b,b"7za -h")}
pub fn run_command(b:&[u8],cmd:&[u8])->Result<u64,&'static str>{unsafe{
    if !verified(b){return Err("Only the pinned official 7-Zip executable is allowed");}
    if !self_test(){return Err("Trial containment checks failed");}
    reset();if !set_args(cmd){return Err("Invalid command arguments");}
    workspace_ready();let backup=&mut *ptr::addr_of_mut!(WORK_BACKUP);let before=WORK.pack(backup).ok_or("Workspace snapshot failed")?;
    WORK.close_all();ptr::write_bytes(ptr::addr_of_mut!(HEAP_DATA.0).cast::<u8>(),0,HEAP_BYTES);
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
        match s{imports::Symbol::Name(n)=>{if n.len()>128{ok=false;return;}a.nn=n.len();a.name[..a.nn].copy_from_slice(n.as_bytes());a.kind=kind(dll,n);},imports::Symbol::Ordinal(n)=>{a.name[0]=b'#';for j in 0..4{a.name[1+j]=b"0123456789abcdef"[((n>>(12-j*4))&15)as usize];}a.nn=5;a.kind=kind(dll,core::str::from_utf8(&a.name[..5]).unwrap_or(""));}}
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
    WORK_ACTIVE=true;let status=enter(BASE+entry,3000);
    WORK.close_all();WORK_ACTIVE=false;if status!=1||EXIT!=0{let _=WORK.unpack(&backup[..before]);}
    serial::metric("H5K: TRIAL_RESULT=",status);serial::metric("H5K: TRIAL_EXIT=",EXIT as u64);
    serial::metric("H5K: TRIAL_FAULT_ADDRESS=",H5_TRIAL_FAULT);serial::metric("H5K: TRIAL_FAULT_RIP=",H5_TRIAL_RIP);
    serial::metric("H5K: TRIAL_API_CALLS=",CALLS);
    if DETAIL_LEN==0&&status==2{detail(b"CPU",b"application fault");}else if DETAIL_LEN==0&&status==3{detail(b"CPU",b"execution timeout");}
    if LAST_API<N{let a=ptr::addr_of!(APIS).cast::<Api>().add(LAST_API).read();serial::write("H5K: TRIAL_LAST_API|");serial::write(core::str::from_utf8(&a.dll[..a.dn]).unwrap_or("?"));serial::write("|");serial::line(core::str::from_utf8(&a.name[..a.nn]).unwrap_or("?"));}
    let bytes=core::slice::from_raw_parts(ptr::addr_of!(OUT).cast::<u8>(),OUT_LEN);
    serial::line("H5K: TRIAL_OUTPUT_BEGIN");for line in bytes.split(|c|*c==10){if let Ok(s)=core::str::from_utf8(line){if s.bytes().all(|c|c==13||c==9||(32..127).contains(&c)){serial::line(s);}}}
    serial::line("H5K: TRIAL_OUTPUT_END");
    let help=status==1&&EXIT==0&&bytes.windows(5).any(|w|w==b"7-Zip")&&bytes.windows(6).any(|w|w==b"Usage:");
    HELP_OK=help;if cmd==b"7za i"&&status==1&&EXIT==0&&bytes.windows(7).any(|w|w==b"Formats"){serial::line("H5K: SEVENZIP_FORMATS_OK");}
    if help{serial::line("H5K: SEVENZIP_HELP_OK");}else{serial::line("H5K: SEVENZIP_HELP_NOT_QUALIFIED");}
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
