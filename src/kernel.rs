//! Bootstrap and qualified kernel self-tests before native desktop launch.
use crate::{boot::BootInfo,cpu,display::Screen,memory,serial,platform,services,desktop};
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
    let ram=allocator.total/256;
    if let Err(e)=platform::init(&info){fail(&mut screen,e);}
    if !crate::heap::init(allocator){fail(&mut screen,"HEAP_RESERVE");}
    if !crate::heap::self_test(){fail(&mut screen,"HEAP_TEST");}
    serial::line("H5K: HEAP_ALLOC_FREE_OK");
    if !crate::security::init(info.clock){fail(&mut screen,"SECURITY_FOUNDATIONS_TEST");}
    if !crate::sheet::self_test(){fail(&mut screen,"SHEET_TEST");}
    serial::line("H5K: SHEET_FORMULAS_OK");
    if !crate::office::self_test(){fail(&mut screen,"OFFICE_FORMAT_TEST");}
    serial::line("H5K: OFFICE_FORMATS_OK");
    if !crate::ui::self_test(){fail(&mut screen,"HI5UI_TEST");}
    serial::line("H5K: HI5UI_TOKENS_OK");
    if !crate::pointer::self_test(){fail(&mut screen,"POINTER_MODEL_TEST");}
    serial::line("H5K: POINTER_MODEL_OK");
    let mut tasks=services::Tasks::new();
    if !tasks.self_test(){fail(&mut screen,"SCHEDULER_TEST");}
    serial::line("H5K: COOPERATIVE_SCHEDULER_OK");
    if !crate::procman::self_test(){fail(&mut screen,"PROCESS_MODEL_TEST");}
    serial::line("H5K: PROCESS_MODEL_OK");
    let mut files=services::Files::new();
    if !files.self_test(){fail(&mut screen,"RAMFS_TEST");}
    serial::line("H5K: RAMFS_OK");
    if !crate::winwindow::self_test(){fail(&mut screen,"WINDOW_OBJECT_TEST");}
    serial::line("H5K: WINDOW_OBJECT_TEST_OK");
    if !crate::msi::self_test(){fail(&mut screen,"MSI_CONTAINER_TEST");}
    serial::line("H5K: MSI_CONTAINER_TEST_OK");
    if !crate::compat::self_test(){fail(&mut screen,"COMPAT_FORMAT_TEST");}
    serial::line("H5K: COMPAT_FORMAT_PROBE_OK");
    if !crate::compat::stage_test(){fail(&mut screen,"PE_STAGE_TEST");}
    serial::line("H5K: PE_STAGE_OK");
    if !crate::platform::run_pe_fixture(){fail(&mut screen,"PE_EXECUTION_TEST");}
    if !crate::imports::self_test(){fail(&mut screen,"PE_IMPORT_INVENTORY_TEST");}
    serial::line("H5K: PE_IMPORT_INVENTORY_OK");
    if !crate::relocate::self_test(){fail(&mut screen,"PE_RELOC_TEST");}
    serial::line("H5K: PE_RELOC_SELFTEST_OK");
    if !crate::platform::run_runtime_fixture(){fail(&mut screen,"PE_RUNTIME_TEST");}
    if !crate::platform::run_console_fixture(){fail(&mut screen,"PE_CONSOLE_TEST");}
    let nics=services::network_probe();
    let (store,loaded)=crate::disk::Store::open();
    if let Some((present,data,len))=loaded {if present{files.write(&data[..len]);}else{files.remove();}}
    if let Some(fb)=info.framebuffer{desktop::run(fb,ram,nics,files,tasks,store);}
    fail(&mut screen,"DESKTOP_REQUIRES_FRAMEBUFFER")
}
