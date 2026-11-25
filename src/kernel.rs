//! First independent kernel entry: diagnostic-only, no memory reclamation.
use crate::{boot::{BootInfo, Framebuffer}, serial};
use core::{arch::asm, ptr};

pub fn halt() -> ! {
    loop { unsafe { asm!("cli", "hlt", options(nomem, nostack)); } }
}

fn fail(reason: &str) -> ! { serial::line(reason); halt() }

pub fn enter(info: BootInfo) -> ! {
    serial::line("H5K: EXIT_BOOT_SERVICES_OK");
    serial::line("H5K: KERNEL_ENTERED");
    serial::metric("H5K: DESCRIPTOR_VERSION=", info.descriptor_version as u64);
    serial::metric("H5K: MAP_ENTRIES=", (info.map_size / info.descriptor_size) as u64);
    let mut conventional_pages = 0u64;
    for offset in (0..info.map_size).step_by(info.descriptor_size) {
        // EFI_MEMORY_DESCRIPTOR v1, x64: Type at 0, PhysicalStart at 8,
        // NumberOfPages at 24. Respect the firmware's descriptor stride.
        let (kind, start, pages) = unsafe {
            let p = info.map.add(offset);
            (ptr::read_unaligned(p.cast::<u32>()), ptr::read_unaligned(p.add(8).cast::<u64>()), ptr::read_unaligned(p.add(24).cast::<u64>()))
        };
        if kind == 7 { // EfiConventionalMemory; diagnostic count only
            if start % 4096 != 0 || pages.checked_mul(4096).and_then(|n| start.checked_add(n)).is_none() { fail("H5K: BAD_MEMORY_RANGE"); }
            conventional_pages = match conventional_pages.checked_add(pages) { Some(n) => n, None => fail("H5K: MEMORY_OVERFLOW") };
        }
    }
    if conventional_pages == 0 { fail("H5K: NO_CONVENTIONAL_MEMORY"); }
    serial::metric("H5K: CONVENTIONAL_PAGES=", conventional_pages);
    serial::line("H5K: MEMORY_MAP_OK");
    // CPUID is guaranteed on x86-64. No scheduler or custom page tables yet.
    let leaf = unsafe { core::arch::x86_64::__cpuid(0) };
    serial::metric("H5K: CPUID_MAX=", leaf.eax as u64);
    if let Some(fb) = info.framebuffer {
        draw(fb);
        serial::line("H5K: FRAMEBUFFER_OK");
    } else { serial::line("H5K: FRAMEBUFFER_SKIPPED"); }
    serial::line("H5K: KERNEL_READY");
    halt()
}

fn pixel(fb: Framebuffer, x: usize, y: usize, rgb: [u8; 3]) {
    if x >= fb.width || y >= fb.height { return; }
    let value = if fb.format == 0 { u32::from_le_bytes([rgb[0],rgb[1],rgb[2],0]) } else { u32::from_le_bytes([rgb[2],rgb[1],rgb[0],0]) };
    // GOP bounds were validated before handoff. No mapping changes yet.
    unsafe { ptr::write_volatile((fb.base as *mut u32).add(y * fb.stride + x), value); }
}
fn draw(fb: Framebuffer) {
    for y in 0..fb.height { for x in 0..fb.width { pixel(fb,x,y,[9,21,36]); } }
    // Original 5x7 bitmap glyphs: H5K, drawn directly into GOP memory.
    let glyphs: [[u8;7];3] = [
        [17,17,17,31,17,17,17],
        [31,16,16,30,1,1,30],
        [17,18,20,24,20,18,17],
    ];
    let scale = (fb.width / 25).min(fb.height / 12).clamp(1, 12);
    let left = fb.width.saturating_sub(17 * scale) / 2;
    let top = fb.height.saturating_sub(7 * scale) / 2;
    for (g, glyph) in glyphs.iter().enumerate() {
        for (y, row) in glyph.iter().enumerate() {
            for x in 0..5 { if row & (1 << (4-x)) != 0 {
                for dy in 0..scale { for dx in 0..scale {
                    pixel(fb,left+(g*6+x)*scale+dx,top+y*scale+dy,[80,174,241]);
                }}
            }}
        }
    }
}
