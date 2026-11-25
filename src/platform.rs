//! VM-only x86_64 platform: own GDT, IDT, four-level identity map, ring-3 probe.
use crate::{boot::BootInfo,cpu,serial,kernel};
use core::{arch::{asm,global_asm},ptr};
#[repr(C,align(4096))]
struct Table([u64;512]);
static mut ROOT:Table=Table([0;512]);
static mut PDPT:Table=Table([0;512]);
static mut PD:[Table;4]=[Table([0;512]),Table([0;512]),Table([0;512]),Table([0;512])];
static mut GUARD:Table=Table([0;512]);
static mut USER_PD:Table=Table([0;512]);
static mut USER_PT:Table=Table([0;512]);
static mut USER_CODE:Table=Table([0;512]);
static mut USER_STACK:Table=Table([0;512]);
const USER_VA:u64=0x1_0000_0000;
#[repr(C,packed)]
struct Dtr {limit:u16,base:u64}
#[repr(C,align(16))]
struct Gdt([u64;7]);
static mut GDT:Gdt=Gdt([0,0x00af9a000000ffff,0x00cf92000000ffff,0x00cff2000000ffff,0x00affa000000ffff,0,0]);
#[repr(C,align(16))]
struct Tss([u8;104]);
static mut TSS:Tss=Tss([0;104]);
#[repr(C,align(16))]
struct EntryStack([u8;16384]);
static mut ENTRY:EntryStack=EntryStack([0;16384]);
#[repr(C)]
#[derive(Clone,Copy)]
struct Gate{lo:u16,sel:u16,ist:u8,attr:u8,mid:u16,hi:u32,zero:u32}
const EMPTY:Gate=Gate{lo:0,sel:0,ist:0,attr:0,mid:0,hi:0,zero:0};
static mut IDT:[Gate;256]=[EMPTY;256];
#[no_mangle] static mut H5_BREAK:u64=0;
#[no_mangle] static mut H5_FAULT:u64=0;
#[no_mangle] static mut H5_EXPECT:u64=0;
#[no_mangle] static mut H5_TICKS:u64=0;
#[no_mangle] static mut H5_SAVED_RSP:u64=0;
#[no_mangle] static mut H5_USER_OK:u64=0;

global_asm!(r#"
.text
.global h5_break
h5_break:
    inc qword ptr [rip + H5_BREAK]
    iretq
.global h5_fault
h5_fault:
    push rax
    mov rax, cr2
    cmp rax, qword ptr [rip + H5_EXPECT]
    jne h5_bad
    cmp qword ptr [rip + H5_EXPECT], 0
    je h5_bad
    lea rax, [rip + h5_probe_fault_resume]
    mov [rsp + 16], rax
    inc qword ptr [rip + H5_FAULT]
    mov qword ptr [rip + H5_EXPECT], 0
    pop rax
    add rsp, 8
    iretq
.global h5_probe_fault
h5_probe_fault:
    mov qword ptr [rip + H5_EXPECT], rcx
    mov al, byte ptr [rcx]
h5_probe_fault_resume:
    ret
.global h5_timer
h5_timer:
    push rax
    inc qword ptr [rip + H5_TICKS]
    mov al, 0x20
    out 0x20, al
    pop rax
    iretq
.global h5_bad
h5_bad:
    cli
    and rsp, -16
    sub rsp, 32
    call h5_fatal
    ud2
.global h5_syscall
h5_syscall:
    cmp eax, 0x48354b
    jne h5_bad
    cmp qword ptr [rsp + 8], 0x23
    jne h5_bad
    mov qword ptr [rip + H5_USER_OK], 1
    mov rsp, qword ptr [rip + H5_SAVED_RSP]
    jmp h5_user_return
.global h5_run_user
h5_run_user:
    mov qword ptr [rip + H5_SAVED_RSP], rsp
    push 0x1b
    push rdx
    push 2
    push 0x23
    push rcx
    iretq
h5_user_return:
    ret
"#);
extern "efiapi" {
    fn h5_break(); fn h5_fault(); fn h5_timer(); fn h5_bad(); fn h5_syscall();
    fn h5_probe_fault(address:u64); fn h5_run_user(ip:u64,sp:u64);
}
#[no_mangle]
extern "efiapi" fn h5_fatal()->! {serial::line("H5K: FAIL UNEXPECTED_CPU_EXCEPTION");kernel::halt()}
fn addr<T>(p:*const T)->u64{p as u64}
pub fn out(port:u16,value:u8){unsafe{asm!("out dx, al",in("dx")port,in("al")value,options(nomem,nostack,preserves_flags));}}
pub fn input(port:u16)->u8{let v:u8;unsafe{asm!("in al, dx",in("dx")port,out("al")v,options(nomem,nostack,preserves_flags));}v}
pub fn out32(port:u16,value:u32){unsafe{asm!("out dx, eax",in("dx")port,in("eax")value,options(nomem,nostack,preserves_flags));}}
pub fn input32(port:u16)->u32{let v:u32;unsafe{asm!("in eax, dx",in("dx")port,out("eax")v,options(nomem,nostack,preserves_flags));}v}
unsafe fn gate(i:usize,handler:usize,attr:u8){
    let a=handler as u64;
    ptr::addr_of_mut!(IDT).cast::<Gate>().add(i).write(Gate{lo:a as u16,sel:8,ist:0,attr,mid:(a>>16)as u16,hi:(a>>32)as u32,zero:0});
}
pub fn init(info:&BootInfo)->Result<(), &'static str>{
    if !cpu::four_level(){return Err("FIVE_LEVEL_UNSUPPORTED");}
    let guard=cpu::stack_base() as u64;
    // Candidate explicitly limited to identity-mapped low 4 GiB VM firmware.
    if guard%4096!=0 || guard>=1<<32 || addr(ptr::addr_of!(ROOT))>=1<<32 ||
       addr(ptr::addr_of!(USER_STACK))+4096>1<<32 ||
       addr(h5_fatal as *const ())>=1<<32 ||
       info.framebuffer.map(|f|(f.base as u64).checked_add(f.size as u64).map_or(true,|e|e>1<<32)).unwrap_or(false) {return Err("ADDRESS_OUTSIDE_VM_MAP");}
    unsafe{
        for p in [addr(ptr::addr_of!(ROOT)),addr(ptr::addr_of!(PDPT)),addr(ptr::addr_of!(PD)),addr(ptr::addr_of!(GUARD)),addr(ptr::addr_of!(USER_PD)),addr(ptr::addr_of!(USER_PT)),addr(ptr::addr_of!(USER_CODE)),addr(ptr::addr_of!(USER_STACK)),guard]{
            if !cpu::writable_identity(p){return Err("BOOTSTRAP_MAPPING_UNSUPPORTED");}
        }
        let r=ptr::addr_of_mut!(ROOT.0).cast::<u64>();
        let p=ptr::addr_of_mut!(PDPT.0).cast::<u64>();
        r.write(addr(ptr::addr_of!(PDPT))|7);
        for i in 0..4usize{
            let d=ptr::addr_of_mut!(PD).cast::<Table>().add(i);
            p.add(i).write(addr(d)|3);
            for j in 0..512usize{
                let phys=((i*512+j)as u64)<<21;
                let flags=if phys>=0x8000_0000{0x9b}else{0x83};
                (*d).0[j]=phys|flags;
            }
        }
        let gi=(guard>>30)as usize;let gj=((guard>>21)&511)as usize;
        let gp=ptr::addr_of_mut!(GUARD.0).cast::<u64>();
        let base=guard & !0x1f_ffff;
        for j in 0..512usize{let a=base+(j as u64)*4096;gp.add(j).write(if a==guard{0}else{a|3});}
        (*ptr::addr_of_mut!(PD).cast::<Table>().add(gi)).0[gj]=addr(ptr::addr_of!(GUARD))|3;
        p.add(4).write(addr(ptr::addr_of!(USER_PD))|7);
        ptr::addr_of_mut!(USER_PD.0).cast::<u64>().write(addr(ptr::addr_of!(USER_PT))|7);
        let u=ptr::addr_of_mut!(USER_PT.0).cast::<u64>();
        u.write(addr(ptr::addr_of!(USER_CODE))|5);
        u.add(1).write(addr(ptr::addr_of!(USER_STACK))|7);
        // MOV EAX,0x48354b; INT 80h; UD2. No privileged code runs in user mode.
        let code=[0xb8u8,0x4b,0x35,0x48,0,0xcd,0x80,0x0f,0x0b];
        ptr::copy_nonoverlapping(code.as_ptr(),ptr::addr_of_mut!(USER_CODE).cast::<u8>(),code.len());

        let t=ptr::addr_of_mut!(TSS).cast::<u8>();
        ptr::write_unaligned(t.add(4).cast::<u64>(),addr(ptr::addr_of!(ENTRY))+16384);
        ptr::write_unaligned(t.add(102).cast::<u16>(),104);
        let tb=addr(t);let g=ptr::addr_of_mut!(GDT.0).cast::<u64>();
        g.add(5).write(103|((tb&0xffffff)<<16)|(0x89<<40)|(((tb>>24)&255)<<56));
        g.add(6).write(tb>>32);
        let gd=Dtr{limit:55,base:addr(g)};
        asm!("lgdt [{}]","push 8","lea rax, [rip + 2f]","push rax","retfq","2:","mov ax, 0x10","mov ds, ax","mov es, ax","mov ss, ax","mov ax, 0x28","ltr ax",in(reg)&gd,out("rax") _,);
        for i in 0..256{gate(i,h5_bad as usize,0x8e);}
        gate(3,h5_break as usize,0x8e);gate(14,h5_fault as usize,0x8e);
        gate(32,h5_timer as usize,0x8e);gate(128,h5_syscall as usize,0xee);
        let id=Dtr{limit:4095,base:addr(ptr::addr_of!(IDT))};
        asm!("lidt [{}]",in(reg)&id,options(readonly,nostack,preserves_flags));
        asm!("int3");
        if ptr::read_volatile(ptr::addr_of!(H5_BREAK))!=1{return Err("BREAKPOINT_TEST");}
        serial::line("H5K: GDT_IDT_BREAKPOINT_OK");
        let root=addr(ptr::addr_of!(ROOT));
        asm!("mov cr3, {}",in(reg)root,options(nostack,preserves_flags));
        // Enable supervisor write protection. No NX or W^X policy in this candidate.
        let mut cr0:u64;asm!("mov {}, cr0",out(reg)cr0,options(nostack,preserves_flags));
        cr0|=1<<16;asm!("mov cr0, {}",in(reg)cr0,options(nostack,preserves_flags));
        if cpu::cr3()&!4095!=root{return Err("CR3_SWITCH");}
        serial::line("H5K: OWN_PAGING_OK");
        h5_probe_fault(guard);
        if ptr::read_volatile(ptr::addr_of!(H5_FAULT))!=1{return Err("GUARD_PAGE_TEST");}
        serial::line("H5K: GUARD_FAULT_OK");
        h5_run_user(USER_VA,USER_VA+8192-16);
        if ptr::read_volatile(ptr::addr_of!(H5_USER_OK))!=1{return Err("USER_MODE_TEST");}
        serial::line("H5K: USER_SYSCALL_OK");
    }
    Ok(())
}
pub fn timer_init(){
    // Remap 8259 PIC; only PIT IRQ0 unmasked. PS/2 is polled.
    out(0x20,0x11);out(0xa0,0x11);out(0x21,0x20);out(0xa1,0x28);
    out(0x21,4);out(0xa1,2);out(0x21,1);out(0xa1,1);
    out(0x21,0xfe);out(0xa1,0xff);
    out(0x43,0x36);let divisor=11932u16;out(0x40,divisor as u8);out(0x40,(divisor>>8)as u8);
    // Prevent a stale firmware local-APIC timer interrupt on supported VM CPUs.
    unsafe{asm!("sti",options(nomem,nostack));}
}
pub fn ticks()->u64{unsafe{ptr::read_volatile(ptr::addr_of!(H5_TICKS))}}
pub fn wait_tick(){unsafe{asm!("hlt",options(nomem,nostack));}}
