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
#[repr(C)]struct Frame{rax:u64,rcx:u64,rdx:u64,r8:u64,r9:u64,rip:u64,cs:u64,flags:u64,rsp:u64,ss:u64}
global_asm!(r#"
.text
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
extern "efiapi"{fn h5_initterm_start();fn h5_initterm_end();fn h5_trial_enter(ip:u64,sp:u64);fn h5_trial_sys();fn h5_trial_fault_error();fn h5_trial_fault_noerror();}
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
unsafe fn write32(v:u64,x:u32)->bool{if let Some(p)=phys(v,4,true){ptr::write_unaligned(p.cast(),x);true}else{false}}
unsafe fn write64(v:u64,x:u64)->bool{if let Some(p)=phys(v,8,true){ptr::write_unaligned(p.cast(),x);true}else{false}}
unsafe fn c_len(v:u64,wide:bool)->Option<usize>{let step=if wide{2}else{1};for n in 0..65536usize{let p=phys(v.checked_add((n*step)as u64)?,step,false)?;if if wide{ptr::read_unaligned(p.cast::<u16>())==0}else{*p==0}{return Some(n);}}None}
unsafe fn allocate(n:usize)->u64{
    let Some(n)=n.max(1).checked_add(15).map(|x|x&!15)else{return 0;};
    let blocks=&mut *ptr::addr_of_mut!(BLOCKS);
    if let Some(b)=blocks.iter_mut().find(|b|!b.used&&b.at!=0&&b.len>=n){b.used=true;return b.at;}
    let Some(b)=blocks.iter_mut().find(|b|b.at==0)else{return 0;};let at=NEXT;
    if at.checked_add(n as u64).map_or(true,|x|x>HEAP_END){return 0;}
    NEXT=at+n as u64;*b=Block{at,len:n,used:true};at
}
unsafe fn release(v:u64)->bool{if v==0{return true;}if let Some(b)=(&mut *ptr::addr_of_mut!(BLOCKS)).iter_mut().find(|b|b.at==v&&b.used){b.used=false;true}else{false}}
unsafe fn abort_import(i:usize){let a=ptr::addr_of!(APIS).cast::<Api>().add(i).read();serial::write("H5K: TRIAL_UNSUPPORTED|");serial::write(core::str::from_utf8(&a.dll[..a.dn]).unwrap_or("?"));serial::write("|");serial::line(core::str::from_utf8(&a.name[..a.nn]).unwrap_or("?"));H5_TRIAL_STATUS=4;}
unsafe fn capture(v:u64,n:usize)->bool{
    if n==0{return true;}if n>8192-OUT_LEN{return false;}let Some(p)=phys(v,n,false)else{return false;};
    ptr::copy_nonoverlapping(p,ptr::addr_of_mut!(OUT).cast::<u8>().add(OUT_LEN),n);OUT_LEN+=n;true
}
#[no_mangle]unsafe extern "efiapi" fn h5_trial_dispatch(f:*mut Frame){
    let f=&mut *f;let id=f.rax as usize;if id==256{EXIT=f.rcx as u32;H5_TRIAL_STATUS=1;return;}if id>=N{H5_TRIAL_STATUS=4;return;}
    CALLS+=1;LAST_API=id;
    let a=ptr::addr_of!(APIS).cast::<Api>().add(id).read();
    let (x,y,z,w)=(f.rcx,f.rdx,f.r8,f.r9);f.rax=0;
    match a.kind{
        1=>{EXIT=x as u32;H5_TRIAL_STATUS=1;},
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
        17=>{if x==0{f.rax=BASE;}else{abort_import(id);}},
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
        27=>{let r=fs().close(x);bool_result(f,r);},
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
        41=>{if !matches!(x,1|2)||!write32(y,0){LAST=87;}else{f.rax=1;}},
        42=>{if matches!(x,1|2)&&y==0{f.rax=1;}else{LAST=87;}},
        43=>{if x==0{LAST=87;}else{f.rax=0;LAST=126;}}, // no external DLL loader
        44=>{if !write64(x,100){LAST=87;}else{f.rax=1;}},
        45=>{if !write64(x,platform::ticks()){LAST=87;}else{f.rax=1;}},
        46=>{if x!=u64::MAX||!write64(y,1)||!write64(z,1){LAST=87;}else{f.rax=1;}},
        47=>{if x!=u64::MAX||y!=1{LAST=87;}else{f.rax=1;}},
        48=>{if x!=0{let Some(p)=phys(x,48,true)else{LAST=87;return;};ptr::write_bytes(p,0,48);ptr::write_unaligned(p.cast::<u16>(),9);ptr::write_unaligned(p.add(4).cast::<u32>(),4096);ptr::write_unaligned(p.add(8).cast::<u64>(),BASE);ptr::write_unaligned(p.add(16).cast::<u64>(),AUX+LIMIT as u64-1);ptr::write_unaligned(p.add(24).cast::<u64>(),1);ptr::write_unaligned(p.add(32).cast::<u32>(),1);ptr::write_unaligned(p.add(36).cast::<u32>(),8664);ptr::write_unaligned(p.add(40).cast::<u32>(),65536);}else{abort_import(id);}},
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
    if d.eq_ignore_ascii_case("kernel32.dll"){match n{"ExitProcess"=>1,"GetCurrentProcessId"|"GetCurrentThreadId"=>2,"SetLastError"=>3,"GetLastError"=>4,"GetCommandLineW"=>5,"GetStdHandle"=>6,"WriteFile"=>7,"GetTickCount"=>15,"GetCurrentProcess"=>16,"GetModuleHandleA"|"GetModuleHandleW"=>17,"CreateFileW"=>25,"ReadFile"=>26,"CloseHandle"=>27,"GetFileSize"=>28,"SetFilePointer"=>29,"SetEndOfFile"=>30,"CreateDirectoryW"=>31,"RemoveDirectoryW"=>32,"DeleteFileW"=>33,"GetFileAttributesW"=>34,"GetCurrentDirectoryW"=>35,"GetLargePageMinimum"=>39,"GetOEMCP"=>40,"GetConsoleMode"=>41,"SetConsoleMode"=>42,"LoadLibraryW"=>43,"QueryPerformanceFrequency"=>44,"QueryPerformanceCounter"=>45,"GetProcessAffinityMask"=>46,"SetProcessAffinityMask"=>47,"GetSystemInfo"=>48,_=>0}}
    else if d.eq_ignore_ascii_case("msvcrt.dll"){match n{"exit"|"_exit"=>1,"malloc"=>8,"free"=>9,"memcpy"|"memmove"=>10,"memset"=>11,"strlen"=>12,"__set_app_type"=>13,"__getmainargs"=>14,"memcmp"=>18,"realloc"=>19,"strcmp"=>20,"wcscmp"=>21,"__setusermatherr"=>22,"_isatty"=>23,"_get_osfhandle"=>24,"fputs"=>36,"fputc"=>37,"fflush"=>38,_=>0}}else{0}
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
    fs().reset();MATH_HANDLER=0;CALLS=0;LAST_API=usize::MAX;HELP_OK=false;BLOCKS=[FREE;128];NEXT=HEAP;N=0;IMAGE_SIZE=0;OUT_LEN=0;LAST=0;EXIT=0;APP_TYPE=0;
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
    let report=imports::inspect_slots(b,|dll,s,slot|{
        let i=N;if i>=224||dll.len()>48||slot as usize+8>r.size||slot%8!=0{ok=false;return;}
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
    let status=enter(BASE+entry,300);
    serial::metric("H5K: TRIAL_RESULT=",status);serial::metric("H5K: TRIAL_EXIT=",EXIT as u64);
    serial::metric("H5K: TRIAL_FAULT_ADDRESS=",H5_TRIAL_FAULT);serial::metric("H5K: TRIAL_FAULT_RIP=",H5_TRIAL_RIP);
    serial::metric("H5K: TRIAL_API_CALLS=",CALLS);
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
