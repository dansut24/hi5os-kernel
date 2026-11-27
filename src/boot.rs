//! Minimal UEFI 2.x x64 ABI declarations. No firmware calls after handoff.
use crate::{kernel, serial};
use core::{ffi::c_void, ptr};

const ERROR: usize = 1 << (usize::BITS - 1);
const INVALID_PARAMETER: usize = ERROR | 2;
const SYSTEM_SIGNATURE: u64 = 0x5453_5953_2049_4249;
const BOOT_SIGNATURE: u64 = 0x5652_4553_544f_4f42;
const MAP_CAPACITY: usize = 128 * 1024;

#[repr(C)]
struct Header { signature: u64, revision: u32, size: u32, crc32: u32, reserved: u32 }
#[repr(C)]
pub struct SystemTable {
    header: Header,
    firmware_vendor: *const u16,
    firmware_revision: u32,
    console_in_handle: usize,
    console_in: usize,
    console_out_handle: usize,
    console_out: usize,
    stderr_handle: usize,
    stderr: usize,
    runtime_services: usize,
    boot_services: *const BootServices,
}

type GetMap = unsafe extern "efiapi" fn(*mut usize, *mut c_void, *mut usize, *mut usize, *mut u32) -> usize;
type ExitBoot = unsafe extern "efiapi" fn(usize, usize) -> usize;
type LocateProtocol = unsafe extern "efiapi" fn(*const Guid, *const c_void, *mut *mut c_void) -> usize;
#[repr(C)]
struct BootServices {
    header: Header,
    before_get_map: [usize; 4],
    get_memory_map: GetMap,
    allocate_pool: usize,
    free_pool: unsafe extern "efiapi" fn(*mut c_void)->usize,
    before_exit: [usize; 19],
    exit_boot_services: ExitBoot,
    before_locate: [usize; 10],
    locate_protocol: LocateProtocol,
}
#[repr(C)]
struct Guid { a: u32, b: u16, c: u16, d: [u8; 8] }
const GOP_GUID: Guid = Guid { a: 0x9042a9de, b: 0x23dc, c: 0x4a38, d: [0x96,0xfb,0x7a,0xde,0xd0,0x80,0x51,0x6a] };
#[repr(C)]
struct Gop { query_mode: unsafe extern "efiapi" fn(*mut Gop,u32,*mut usize,*mut *mut GopInfo)->usize, set_mode: unsafe extern "efiapi" fn(*mut Gop,u32)->usize, blt: usize, mode: *const GopMode }
#[repr(C)]
struct GopMode {
    max_mode: u32, mode: u32, info: *const GopInfo, info_size: usize,
    framebuffer_base: u64, framebuffer_size: usize,
}
#[repr(C)]
struct GopInfo {
    version: u32, width: u32, height: u32, pixel_format: u32,
    pixel_masks: [u32; 4], stride: u32,
}
#[derive(Clone, Copy)]
pub struct Framebuffer { pub base: usize, pub size: usize, pub width: usize, pub height: usize, pub stride: usize, pub format: u32 }
#[repr(align(16))]
struct MapBuffer([u8; MAP_CAPACITY]);
static mut MAP: MapBuffer = MapBuffer([0; MAP_CAPACITY]);

pub struct BootInfo {
    pub map: *const u8,
    pub map_size: usize,
    pub descriptor_size: usize,
    pub descriptor_version: u32,
    pub framebuffer: Option<Framebuffer>,
    pub clock: Option<crate::security::Clock>,
}

// Compile-time ABI guards for the only supported architecture.
const _: () = {
    assert!(core::mem::offset_of!(SystemTable, boot_services) == 96);
    assert!(core::mem::offset_of!(BootServices, get_memory_map) == 56);
    assert!(core::mem::offset_of!(BootServices, free_pool) == 72);
    assert!(core::mem::offset_of!(BootServices, exit_boot_services) == 232);
    assert!(core::mem::offset_of!(BootServices, locate_protocol) == 320);
    assert!(core::mem::offset_of!(GopMode, framebuffer_base) == 24);
    assert!(core::mem::size_of::<GopInfo>() == 36);
};

unsafe fn graphics(bs: *const BootServices) -> Option<Framebuffer> {
    let mut interface = ptr::null_mut();
    if ((*bs).locate_protocol)(&GOP_GUID, ptr::null(), &mut interface) != 0 || interface.is_null() { return None; }
    let gop=interface.cast::<Gop>();
    if prefer_large_gop(){select_gop(bs,gop);}
    framebuffer(gop)
}
unsafe fn framebuffer(gop:*mut Gop)->Option<Framebuffer>{
    let mode = (*gop).mode;
    if mode.is_null() || (*mode).info.is_null() || (*mode).info_size < core::mem::size_of::<GopInfo>() { return None; }
    let info = &*(*mode).info;
    let fb = Framebuffer { base: (*mode).framebuffer_base as usize, size: (*mode).framebuffer_size, width: info.width as usize, height: info.height as usize, stride: info.stride as usize, format: info.pixel_format };
    let needed = fb.stride.checked_mul(fb.height)?.checked_mul(4)?;
    if fb.base == 0 || fb.base % 4 != 0 || fb.width == 0 || fb.height == 0 || fb.stride < fb.width || fb.format > 1 || needed > fb.size || fb.base.checked_add(needed).is_none() { return None; }
    Some(fb)
}
// VMware and explicit CI-only QEMU vendor opt into firmware mode negotiation.
pub fn vmware()->bool{let b=unsafe{core::arch::x86_64::__cpuid(1)};if b.ecx&(1<<31)==0{return false;}let h=unsafe{core::arch::x86_64::__cpuid(0x40000000)};h.ebx.to_le_bytes()==*b"VMwa"&&h.ecx.to_le_bytes()==*b"reVM"&&h.edx.to_le_bytes()==*b"ware"}
fn prefer_large_gop()->bool{let b=unsafe{core::arch::x86_64::__cpuid(0)};vmware()||(b.ebx.to_le_bytes()==*b"Hi5G"&&b.edx.to_le_bytes()==*b"opTe"&&b.ecx.to_le_bytes()==*b"stVM")}
fn mode_rank(w:u32,h:u32,format:u32,stride:u32)->u64{if w<640||h<480||w>1920||h>1080||stride<w||format>1{return 0;}if w==1920&&h==1080{return u64::MAX;}w as u64*h as u64}
fn viewport_rank(w:u32,h:u32,format:u32,stride:u32)->u64{let r=mode_rank(w,h,format,stride);if r==0{return 0;}if w==1280&&h==720{return u64::MAX;}if w==1024&&h==768{return u64::MAX-1;}if w<=1280&&h<=800{return 10000000+r;}1}
pub fn display_model_test()->bool{viewport_rank(1280,720,1,1280)>viewport_rank(1920,1080,1,1920)&&viewport_rank(1024,768,1,1024)>viewport_rank(1600,900,1,1600)&&mode_rank(1920,1080,1,1920)>mode_rank(1600,900,1,1600)&&mode_rank(2560,1440,1,2560)==0&&mode_rank(1024,768,2,1024)==0&&mode_rank(1024,768,1,1000)==0}
unsafe fn select_gop(bs:*const BootServices,g:*mut Gop){
    let m=(*g).mode;if m.is_null(){return;}
    let old=(*m).mode;let max=(*m).max_mode.min(256);
    let mut best=None;let mut rank=0;
    for i in 0..max{let mut size=0;let mut info=ptr::null_mut();if ((*g).query_mode)(g,i,&mut size,&mut info)!=0||info.is_null(){continue;}
        if size>=core::mem::size_of::<GopInfo>(){let v=&*info;let r=if vmware(){viewport_rank(v.width,v.height,v.pixel_format,v.stride)}else{mode_rank(v.width,v.height,v.pixel_format,v.stride)};if r>rank{rank=r;best=Some(i);}}
        ((*bs).free_pool)(info.cast());
    }
    if let Some(i)=best{if i!=old&&((*g).set_mode)(g,i)!=0{serial::line("H5K: GOP_PREFERRED_MODE_REJECTED");let _=((*g).set_mode)(g,old);}
        if framebuffer(g).is_none(){let _=((*g).set_mode)(g,old);}
    }
    if let Some(f)=framebuffer(g){serial::metric("H5K: GOP_WIDTH=",f.width as u64);serial::metric("H5K: GOP_HEIGHT=",f.height as u64);serial::line("H5K: GOP_PREFERRED_MODE_READY");}
}
// Only before ExitBootServices; no runtime pointer retained.
type GetTime=unsafe extern "efiapi" fn(*mut crate::security::EfiTime,*mut c_void)->usize;
#[repr(C)]struct RuntimeTime{header:Header,get_time:GetTime}
unsafe fn read_clock(table:*const SystemTable)->Option<crate::security::Clock>{
    let rt=(*table).runtime_services as *const RuntimeTime;
    if rt.is_null()||(*rt).header.signature!=0x56524553544e5552||((*rt).header.size as usize)<core::mem::size_of::<RuntimeTime>(){return None;}
    let mut time=crate::security::EfiTime::empty();
    if ((*rt).get_time)(&mut time,ptr::null_mut())!=0{return None;}
    crate::security::convert(time)
}
fn fail(reason: &str, status: usize) -> ! {
    serial::line(reason);
    serial::metric("H5K: EFI_STATUS=", status as u64);
    kernel::halt()
}

pub unsafe fn start(image: usize, table: *mut SystemTable) -> ! {
    if table.is_null() || (*table).header.signature != SYSTEM_SIGNATURE || ((*table).header.size as usize) < core::mem::size_of::<SystemTable>() { fail("H5K: BAD_SYSTEM_TABLE", 0); }
    let bs = (*table).boot_services;
    if bs.is_null() || (*bs).header.signature != BOOT_SIGNATURE || ((*bs).header.size as usize) < core::mem::size_of::<BootServices>() { fail("H5K: BAD_BOOT_SERVICES", 0); }
    let clock=read_clock(table);
    let framebuffer = graphics(bs);
    if framebuffer.is_some() { serial::line("H5K: GOP_FOUND"); } else { serial::line("H5K: GOP_UNAVAILABLE"); }
    let map = ptr::addr_of_mut!(MAP.0).cast::<u8>();
    // No firmware calls other than GetMemoryMap and ExitBootServices from here.
    // The fixed-size preallocated map avoids allocations changing the key.
    for _ in 0..4 {
        let mut size = MAP_CAPACITY;
        let mut key = 0usize;
        let mut stride = 0usize;
        let mut version = 0u32;
        let status = ((*bs).get_memory_map)(&mut size, map.cast(), &mut key, &mut stride, &mut version);
        if status != 0 { fail("H5K: MEMORY_MAP_FAILED", status); }
        if stride < 40 || size == 0 || size > MAP_CAPACITY || size % stride != 0 || version != 1 { fail("H5K: MEMORY_MAP_INVALID", 0); }
        let status = ((*bs).exit_boot_services)(image, key);
        if status == 0 {
            // No firmware boot-service pointers are passed to the kernel.
            core::arch::asm!("cli", options(nomem, nostack));
            kernel::enter(BootInfo { map, map_size: size, descriptor_size: stride, descriptor_version: version, framebuffer, clock });
        }
        if status != INVALID_PARAMETER { fail("H5K: EXIT_FAILED", status); }
    }
    fail("H5K: EXIT_RETRY_EXHAUSTED", INVALID_PARAMETER)
}
