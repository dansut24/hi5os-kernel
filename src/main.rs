#![no_std]
#![no_main]

mod boot;
mod kernel;
mod serial;
mod memory;
mod display;
mod cpu;
mod platform;
mod desktop;
mod ui;
mod services;
mod disk;
mod compat;
mod imports;
mod net;
mod http;
mod heap;
mod security;
mod sheet;

use core::panic::PanicInfo;

// Firmware enters here; boot::start never returns. The kernel entry is separate
// logically, but linked into the same PE image for this first milestone.
#[no_mangle]
pub extern "efiapi" fn efi_main(image: usize, table: *mut boot::SystemTable) -> usize {
    serial::init();
    serial::line(concat!("H5K: BOOT v", env!("CARGO_PKG_VERSION")));
    // SAFETY: the firmware supplies the image handle and system table.
    unsafe { boot::start(image, table) }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    serial::line("H5K: PANIC");
    kernel::halt()
}
