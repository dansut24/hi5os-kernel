//! Single-core x86-64 bootstrap stack and mapping inspection.
use crate::{boot::BootInfo, kernel};
use core::{arch::asm, mem::MaybeUninit, ptr};

const STACK_SIZE: usize = 64 * 1024;
#[repr(C, align(4096))]
struct Stack([u8; STACK_SIZE]);
static mut STACK: Stack = Stack([0; STACK_SIZE]);
static mut INFO: MaybeUninit<BootInfo> = MaybeUninit::uninit();
const CANARY: u64 = 0x4835_4b53_5441_434b;
pub fn stack_base() -> usize { ptr::addr_of!(STACK) as usize }

pub fn enter(info: BootInfo) -> ! {
    unsafe {
        ptr::addr_of_mut!(INFO).write(MaybeUninit::new(info));
        ptr::write_volatile(stack_base() as *mut u64, CANARY);
        // Microsoft x64 / EFI ABI: stack aligned before call, 32-byte home area.
        // No references into the old firmware stack are kept.
        asm!(
            "mov rsp, r10",
            "and rsp, -16",
            "sub rsp, 32",
            "call {entry}",
            "ud2",
            in("r10") stack_base() + STACK_SIZE,
            in("rcx") ptr::addr_of!(INFO).cast::<BootInfo>(),
            entry = sym on_stack,
            options(noreturn)
        );
    }
}
extern "efiapi" fn on_stack(info: *const BootInfo) -> ! {
    // BootInfo contains only value fields and a pointer to static memory-map storage.
    let info = unsafe { ptr::read(info) };
    kernel::on_stack(info)
}
pub fn stack_ok() -> bool {
    let rsp: usize;
    unsafe { asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags)); }
    rsp > stack_base()+4096 && rsp < stack_base()+STACK_SIZE &&
        unsafe { ptr::read_volatile(stack_base() as *const u64) == CANARY }
}
pub fn cr3() -> u64 {
    let value: u64;
    unsafe { asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags)); }
    value
}
pub fn four_level() -> bool {
    let cr4: u64;
    unsafe { asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags)); }
    cr4 & (1<<12) == 0
}
// UEFI x64 bootstrap uses identity-mapped page-table storage. Check the
// selected conventional-memory frame is still identity mapped and writable
// before touching it; handle 1 GiB, 2 MiB and 4 KiB leaves.
pub fn writable_identity(addr: u64) -> bool {
    if !four_level() || addr >= (1<<32) { return false; }
    const MASK: u64 = 0x000f_ffff_ffff_f000;
    let mut table = cr3() & MASK;
    for shift in [39u32, 30, 21, 12] {
        if table == 0 { return false; }
        let index = ((addr >> shift) & 511) as usize;
        let pte = unsafe { ptr::read_volatile((table as *const u64).add(index)) };
        if pte & 3 != 3 { return false; }
        if shift == 12 || ((shift == 30 || shift == 21) && pte & (1<<7) != 0) {
            let offset = (1u64<<shift)-1;
            return ((pte & MASK & !offset) | (addr & offset)) == addr;
        }
        table = pte & MASK;
    }
    false
}
