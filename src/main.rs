#![no_std]
#![no_main]

mod boot;
mod kernel;
mod serial;

use core::panic::PanicInfo;

// Firmware enters here; boot::start never returns. The kernel entry is separate
// logically, but linked into the same PE image for this first milestone.
#[no_mangle]
pub extern "efiapi" fn efi_main(image: usize, table: *mut boot::SystemTable) -> usize {
    serial::init();
    serial::line("H5K: BOOT v0.0.1");
    // SAFETY: the firmware supplies the image handle and system table.
    unsafe { boot::start(image, table) }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    serial::line("H5K: PANIC");
    kernel::halt()
}
