//! v0.0.2 first memory milestone; firmware mappings retained deliberately.
use crate::{boot::BootInfo,cpu,display::Screen,memory,serial};
use core::arch::asm;

pub fn halt() -> ! {loop {unsafe {asm!("cli","hlt",options(nomem,nostack));}}}
pub fn enter(info:BootInfo)->! {
    serial::line("H5K: EXIT_BOOT_SERVICES_OK");
    serial::line("H5K: KERNEL_ENTERED");
    cpu::enter(info)
}
fn fail(screen:&mut Option<Screen>,reason:&str)->! {
    serial::write("H5K: FAIL ");serial::line(reason);
    if let Some(s)=screen {s.fail(reason);}
    halt()
}
pub fn on_stack(info:BootInfo)->! {
    let mut screen=info.framebuffer.map(Screen::new);
    if let Some(s)=&mut screen {s.ok("UEFI HANDOFF: PASS");}
    if !cpu::stack_ok() {fail(&mut screen,"STACK_CHECK");}
    serial::line("H5K: STACK_OK");
    if let Some(s)=&mut screen {s.ok("KERNEL STACK: PASS");}
    serial::metric("H5K: DESCRIPTOR_VERSION=",info.descriptor_version as u64);
    serial::metric("H5K: MAP_ENTRIES=",(info.map_size/info.descriptor_size) as u64);
    let allocator=match memory::init(&info) {Ok(a)=>a,Err(e)=>fail(&mut screen,e)};
    serial::metric("H5K: MANAGED_PAGES=",allocator.total);
    serial::line("H5K: MEMORY_MAP_OK");
    serial::line("H5K: ALLOCATOR_OK");
    if let Some(s)=&mut screen {s.ok("PHYSICAL ALLOCATOR: PASS");s.number("MANAGED RAM MIB: ",allocator.total/256);}
    if let Err(e)=memory::self_test(allocator) {fail(&mut screen,e);}
    serial::line("H5K: FRAME_RW_REUSE_OK");
    if let Some(s)=&mut screen {s.ok("FRAME READ WRITE REUSE: PASS");s.number("FREE FRAMES: ",allocator.available);}
    if !cpu::stack_ok() {fail(&mut screen,"STACK_CANARY");}
    serial::line("H5K: STACK_CANARY_OK");
    let leaf=unsafe {core::arch::x86_64::__cpuid(0)};
    serial::metric("H5K: CPUID_MAX=",leaf.eax as u64);
    serial::metric("H5K: FIRMWARE_CR3=",cpu::cr3());
    if let Some(s)=&mut screen {
        s.number("CPU MAX CPUID LEAF: ",leaf.eax as u64);
        s.note("PAGING: FIRMWARE MAP RETAINED");
        s.ok("KERNEL READY");
        serial::line("H5K: FRAMEBUFFER_OK");
    } else {serial::line("H5K: FRAMEBUFFER_SKIPPED");}
    serial::line("H5K: KERNEL_READY");
    halt()
}
