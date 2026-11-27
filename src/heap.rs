//! Fixed 40 MiB image-owned heap, single CPU. No interrupt-context allocation.
//! GlobalAlloc interface; sized for the bounded WASM interpreter and its linear memory.
use core::{alloc::{GlobalAlloc,Layout},cell::UnsafeCell,ptr,sync::atomic::{AtomicBool,Ordering}};
pub const CAP:usize=40*1024*1024;
const UNIT:usize=64;
const COUNT:usize=CAP/UNIT;
#[repr(align(4096))]struct Arena([u8;CAP]);
static mut ARENA:Arena=Arena([0;CAP]);
pub struct Heap{locked:AtomicBool,bits:UnsafeCell<[u64;COUNT/64]>}
unsafe impl Sync for Heap{}
struct Guard<'a>(&'a AtomicBool);
impl Drop for Guard<'_>{fn drop(&mut self){self.0.store(false,Ordering::Release);}}
#[global_allocator]
static HEAP:Heap=Heap{locked:AtomicBool::new(false),bits:UnsafeCell::new([0;COUNT/64])};
impl Heap{
    fn lock(&self)->Guard<'_>{while self.locked.compare_exchange_weak(false,true,Ordering::Acquire,Ordering::Relaxed).is_err(){core::hint::spin_loop();}Guard(&self.locked)}
    fn base()->*mut u8{unsafe{ptr::addr_of_mut!(ARENA.0).cast::<u8>()}}
    fn units(layout:Layout)->Option<usize>{let n=layout.size().max(1).checked_add(UNIT-1)?/UNIT;if n>COUNT{None}else{Some(n)}}
    fn used(b:&[u64],i:usize)->bool{b[i/64]&(1u64<<(i%64))!=0}
    fn set(b:&mut[u64],i:usize,on:bool){let mask=1u64<<(i%64);if on{b[i/64]|=mask;}else{b[i/64]&=!mask;}}
}
unsafe impl GlobalAlloc for Heap{
    unsafe fn alloc(&self,l:Layout)->*mut u8{
        let Some(n)=Self::units(l)else{return ptr::null_mut();};
        if l.align()>4096{return ptr::null_mut();}
        let _g=self.lock();let b=&mut *self.bits.get();let base=Self::base();let step=(l.align()/UNIT).max(1);let mut i=0;
        while i<=COUNT-n{
            if let Some(off)=(0..n).find(|j|Self::used(b,i+*j)){i=(i+off+1+step-1)&!(step-1);continue;}
            for j in i..i+n{Self::set(b,j,true);}return base.add(i*UNIT);
        }ptr::null_mut()
    }
    unsafe fn dealloc(&self,p:*mut u8,l:Layout){
        let Some(n)=Self::units(l)else{return;};
        let base=Self::base()as usize;let Some(off)=(p as usize).checked_sub(base)else{return;};
        if off%UNIT!=0||off>=CAP||n>COUNT-off/UNIT{return;}
        let i=off/UNIT;let _g=self.lock();let b=&mut *self.bits.get();
        // As required by GlobalAlloc, callers must provide the original pointer/layout.
        if (i..i+n).any(|j|!Self::used(b,j)){return;}
        for j in i..i+n{Self::set(b,j,false);}
    }
}
pub fn self_test()->bool{
    unsafe{
        let base=Heap::base()as u64;
        if base.checked_add(CAP as u64).map_or(true,|e|e>1u64<<32)||!(0..CAP).step_by(4096).all(|i|crate::cpu::writable_identity(base+i as u64)){return false;}
        let small=Layout::from_size_align(73,64).unwrap();let aligned=Layout::from_size_align(4096,4096).unwrap();
        let a=HEAP.alloc(small);let b=HEAP.alloc_zeroed(aligned);
        if a.is_null()||b.is_null()||a as usize%64!=0||b as usize%4096!=0||a==b{return false;}
        ptr::write_bytes(a,0xa5,73);
        if !(0..73).all(|i|ptr::read_volatile(a.add(i))==0xa5)||!(0..4096).all(|i|ptr::read_volatile(b.add(i))==0){return false;}
        HEAP.dealloc(a,small);let c=HEAP.alloc(small);if c!=a{return false;}HEAP.dealloc(c,small);HEAP.dealloc(b,aligned);
        let whole=Layout::from_size_align(CAP,4096).unwrap();let p=HEAP.alloc(whole);
        if p.is_null()||!HEAP.alloc(small).is_null(){return false;}
        ptr::write_volatile(p,0x12);ptr::write_volatile(p.add(CAP-1),0x34);
        if ptr::read_volatile(p)!=0x12||ptr::read_volatile(p.add(CAP-1))!=0x34{return false;}
        HEAP.dealloc(p,whole);let q=HEAP.alloc_zeroed(small);if q!=p||!(0..73).all(|i|ptr::read_volatile(q.add(i))==0){return false;}HEAP.dealloc(q,small);
        true
    }
}
