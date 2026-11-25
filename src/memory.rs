//! Conservative physical frame allocator: conventional memory below 4 GiB.
//! No firmware/loader/runtime/MMIO reclamation. Single CPU, interrupts off.
use crate::{boot::BootInfo, cpu};
use core::ptr;
const PAGE: u64 = 4096;
const LIMIT: u64 = 1<<32;
const WORDS: usize = (LIMIT/PAGE/64) as usize;
pub struct Allocator {
    managed: [u64; WORDS],
    free: [u64; WORDS],
    pub total: u64,
    pub available: u64,
    cursor: usize,
}
static mut ALLOCATOR: Allocator = Allocator {
    managed: [0; WORDS], free: [0; WORDS], total: 0, available: 0, cursor: 0,
};
impl Allocator {
    fn add_range(&mut self, start: u64, end: u64) -> Result<(), &'static str> {
        let mut first = (start/PAGE) as usize;
        let last = (end/PAGE) as usize;
        while first < last {
            let w = first/64;
            let bit = first%64;
            let count = (64-bit).min(last-first);
            let mask = if count == 64 { u64::MAX } else { ((1u64<<count)-1)<<bit };
            if self.managed[w] & mask != 0 { return Err("OVERLAPPING_CONVENTIONAL_MEMORY"); }
            self.managed[w] |= mask;
            self.free[w] |= mask;
            self.total += count as u64;
            self.available += count as u64;
            first += count;
        }
        Ok(())
    }
    pub fn allocate(&mut self) -> Option<u64> {
        for i in self.cursor..WORDS {
            let bits = self.free[i];
            if bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                self.free[i] &= !(1u64<<bit);
                self.available -= 1;
                self.cursor = i;
                return Some(((i*64+bit) as u64)*PAGE);
            }
        }
        None
    }
    pub fn release(&mut self, address: u64) -> bool {
        if address >= LIMIT || address%PAGE != 0 { return false; }
        let n = (address/PAGE) as usize;
        let w = n/64;
        let mask = 1u64<<(n%64);
        if self.managed[w]&mask == 0 || self.free[w]&mask != 0 { return false; }
        self.free[w] |= mask;
        self.available += 1;
        self.cursor = self.cursor.min(w);
        true
    }
}
pub fn init(info: &BootInfo) -> Result<&'static mut Allocator, &'static str> {
    // One call during bootstrap, no concurrency; exclusive ownership thereafter.
    let a = unsafe { &mut *ptr::addr_of_mut!(ALLOCATOR) };
    if a.total != 0 { return Err("ALLOCATOR_ALREADY_INITIALISED"); }
    let reserved_fb = info.framebuffer.map(|fb| {
        let start = (fb.base as u64) & !(PAGE-1);
        let end = (fb.base as u64).saturating_add(fb.size as u64).saturating_add(PAGE-1) & !(PAGE-1);
        (start,end)
    });
    for offset in (0..info.map_size).step_by(info.descriptor_size) {
        let (kind,start,pages) = unsafe {
            let p = info.map.add(offset);
            (ptr::read_unaligned(p.cast::<u32>()), ptr::read_unaligned(p.add(8).cast::<u64>()), ptr::read_unaligned(p.add(24).cast::<u64>()))
        };
        if kind != 7 { continue; }
        if start%PAGE != 0 { return Err("UNALIGNED_MEMORY_RANGE"); }
        let end = pages.checked_mul(PAGE).and_then(|n|start.checked_add(n)).ok_or("INVALID_MEMORY_RANGE")?;
        let lo = start.max(0x100000);
        let hi = end.min(LIMIT);
        if lo >= hi { continue; }
        if let Some((fb_lo,fb_hi)) = reserved_fb {
            if lo < fb_hi && fb_lo < hi {
                if lo < fb_lo { a.add_range(lo,fb_lo.min(hi))?; }
                if fb_hi < hi { a.add_range(fb_hi.max(lo),hi)?; }
                continue;
            }
        }
        a.add_range(lo,hi)?;
    }
    if a.total < 16 { return Err("INSUFFICIENT_CONVENTIONAL_MEMORY"); }
    Ok(a)
}
pub fn self_test(a: &mut Allocator) -> Result<(), &'static str> {
    let before = a.available;
    let mut frames = [0u64; 8];
    for i in 0..frames.len() {
        let p = a.allocate().ok_or("ALLOC_FAILED")?;
        if p%PAGE != 0 || frames[..i].contains(&p) || !cpu::writable_identity(p) { return Err("BAD_FRAME"); }
        frames[i] = p;
        // Distinct patterns across frames reveal aliases; entire pages tested.
        for j in 0..512usize {
            unsafe { ptr::write_volatile((p as *mut u64).add(j), 0x4835_4b00_0000_0000 | ((i as u64)<<16) | j as u64); }
        }
    }
    for (i,p) in frames.iter().enumerate() {
        for j in 0..512usize {
            let value = unsafe { ptr::read_volatile((*p as *const u64).add(j)) };
            if value != (0x4835_4b00_0000_0000 | ((i as u64)<<16) | j as u64) { return Err("FRAME_DATA_MISMATCH"); }
            unsafe { ptr::write_volatile((*p as *mut u64).add(j),0); }
        }
    }
    if a.release(0) || a.release(frames[0]+1) || a.available != before-8 { return Err("INVALID_FREE_ACCEPTED"); }
    for p in frames { if !a.release(p) { return Err("FREE_FAILED"); } }
    if a.release(frames[0]) || a.available != before { return Err("DOUBLE_FREE_OR_ACCOUNTING"); }
    let reused = a.allocate().ok_or("REUSE_FAILED")?;
    if reused != frames[0] || !a.release(reused) || a.available != before { return Err("REUSE_ACCOUNTING"); }
    Ok(())
}
