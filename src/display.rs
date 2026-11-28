//! Direct framebuffer diagnostics: original 5x7 glyphs, no font dependency.
use crate::boot::Framebuffer;
use core::ptr;
const BG: [u8;3] = [8,20,35];
const BLUE: [u8;3] = [8,199,206];
const WHITE: [u8;3] = [216,232,245];
const GREEN: [u8;3] = [107,219,156];
const RED: [u8;3] = [255,118,118];
pub fn glyph(c: u8) -> [u8;7] {
    match c {
        b'0' => [14,17,19,21,25,17,14],
        b'1' => [4,12,4,4,4,4,14],
        b'2' => [14,17,1,2,4,8,31],
        b'3' => [30,1,1,14,1,1,30],
        b'4' => [2,6,10,18,31,2,2],
        b'5' => [31,16,16,30,1,1,30],
        b'6' => [14,16,16,30,17,17,14],
        b'7' => [31,1,2,4,8,8,8],
        b'8' => [14,17,17,14,17,17,14],
        b'9' => [14,17,17,15,1,1,14],
        b'A' => [14,17,17,31,17,17,17],
        b'B' => [30,17,17,30,17,17,30],
        b'C' => [14,17,16,16,16,17,14],
        b'D' => [30,17,17,17,17,17,30],
        b'E' => [31,16,16,30,16,16,31],
        b'F' => [31,16,16,30,16,16,16],
        b'G' => [14,17,16,23,17,17,14],
        b'H' => [17,17,17,31,17,17,17],
        b'I' => [31,4,4,4,4,4,31],
        b'J' => [7,2,2,2,18,18,12],
        b'K' => [17,18,20,24,20,18,17],
        b'L' => [16,16,16,16,16,16,31],
        b'M' => [17,27,21,21,17,17,17],
        b'N' => [17,25,21,19,17,17,17],
        b'O' => [14,17,17,17,17,17,14],
        b'P' => [30,17,17,30,16,16,16],
        b'Q' => [14,17,17,17,21,18,13],
        b'R' => [30,17,17,30,20,18,17],
        b'S' => [15,16,16,14,1,1,30],
        b'T' => [31,4,4,4,4,4,4],
        b'U' => [17,17,17,17,17,17,14],
        b'V' => [17,17,17,17,17,10,4],
        b'W' => [17,17,17,21,21,27,17],
        b'X' => [17,17,10,4,10,17,17],
        b'Y' => [17,17,10,4,4,4,4],
        b'Z' => [31,1,2,4,8,16,31],
        b'.' => [0,0,0,0,0,12,12],
        b'-' => [0,0,0,31,0,0,0],
        b':' => [0,4,4,0,4,4,0],
        _ => [0;7],
    }
}
pub struct Screen { fb: Framebuffer, scale: usize, y: usize, x: usize }
impl Screen {
    pub fn new(fb: Framebuffer) -> Self {
        let scale = (fb.width/240).min(fb.height/150).clamp(1,4);
        let mut s = Self { fb, scale, x: 8*scale, y: 8*scale };
        s.rect(0,0,fb.width,fb.height,BG);
        s.line(concat!("HI5OS - HI5KERNEL V",env!("CARGO_PKG_VERSION")),BLUE);
        s.line("STARTING YOUR WORKSPACE",WHITE);
        s.y += 4*scale;
        s
    }
    fn pixel(&self,x: usize,y: usize,rgb:[u8;3]) {
        if x>=self.fb.width || y>=self.fb.height {return;}
        let v = if self.fb.format == 0 {u32::from_le_bytes([rgb[0],rgb[1],rgb[2],0])} else {u32::from_le_bytes([rgb[2],rgb[1],rgb[0],0])};
        unsafe {ptr::write_volatile((self.fb.base as *mut u32).add(y*self.fb.stride+x),v);}
    }
    fn rect(&self,x:usize,y:usize,w:usize,h:usize,c:[u8;3]) {
        for yy in y..(y+h).min(self.fb.height) { for xx in x..(x+w).min(self.fb.width) {self.pixel(xx,yy,c);} }
    }
    fn at(&self,mut x:usize,y:usize,text:&str,c:[u8;3])->usize {
        for b in text.bytes() {
            for (dy,bits) in glyph(b).iter().enumerate() {
                for dx in 0..5 { if bits & (1<<(4-dx)) != 0 {
                    self.rect(x+dx*self.scale,y+dy*self.scale,self.scale,self.scale,c);
                }}
            }
            x += 6*self.scale;
        }
        x
    }
    fn line(&mut self,text:&str,c:[u8;3]) {self.at(self.x,self.y,text,c);self.y+=10*self.scale;}
    pub fn ok(&mut self,text:&str) { self.line(text,GREEN); }
    pub fn fail(&mut self,text:&str) { self.line("FAIL - SEE SERIAL LOG",RED); self.line(text,RED); }
    pub fn note(&mut self,text:&str) {self.line(text,WHITE);}
    pub fn number(&mut self,label:&str,mut n:u64) {
        let x=self.at(self.x,self.y,label,WHITE);
        let mut buf=[b'0';20];let mut i=buf.len();
        loop {i-=1;buf[i]=b'0'+(n%10) as u8;n/=10;if n==0 {break;}}
        if let Ok(s)=core::str::from_utf8(&buf[i..]) {self.at(x,self.y,s,WHITE);}
        self.y+=10*self.scale;
    }
}

// Runtime modes for the identified QEMU Standard VGA and qualified active SVGA II adapter.
pub const MODES:[(usize,usize,&str);6]=[(640,480,"640x480"),(800,600,"800x600"),(1024,768,"1024x768"),(1280,720,"1280x720"),(1600,900,"1600x900"),(1920,1080,"1920x1080")];
fn pci(slot:u32,reg:u32)->u32{crate::platform::out32(0xcf8,0x80000000|(slot<<11)|reg);crate::platform::input32(0xcfc)}
fn out16(port:u16,value:u16){unsafe{core::arch::asm!("out dx, ax",in("dx")port,in("ax")value,options(nomem,nostack,preserves_flags));}}
fn in16(port:u16)->u16{let v:u16;unsafe{core::arch::asm!("in ax, dx",in("dx")port,out("ax")v,options(nomem,nostack,preserves_flags));}v}
fn reg(i:u16)->u16{out16(0x1ce,i);in16(0x1cf)}
fn put(i:u16,v:u16){out16(0x1ce,i);out16(0x1cf,v)}
fn aperture(fb:Framebuffer)->Option<usize>{
    if fb.base<0x80000000||fb.base>=0x1_0000_0000||fb.format!=1{return None;}
    let mut found=false;
    for slot in 0..32{if pci(slot,0)!=0x11111234{continue;}let bar=pci(slot,0x10);if bar&7==0&&(bar&!15)as usize==fb.base&&pci(slot,4)&2!=0{found=true;break;}}
    if !found||!(0xb0c4..=0xb0c5).contains(&reg(0))||reg(4)&1==0||reg(3)!=32{return None;}
    let n=reg(10)as usize*65536;
    if n==0||n>256*1024*1024||fb.base.checked_add(n)?>0x1_0000_0000||fb.stride.checked_mul(fb.height)?.checked_mul(4)?>n{return None;}
    Some(n)
}
pub fn mode_available(fb:Framebuffer,i:usize)->bool{let Some(&(w,h,_))=MODES.get(i)else{return false;};aperture(fb).is_some_and(|n|w*h*4<=n)||svga_active(fb).is_some_and(|a|a.fits(w,h))}
pub fn runtime_available(fb:Framebuffer)->bool{aperture(fb).is_some()||svga_active(fb).is_some()}
pub fn backend(fb:Framebuffer)->&'static str{if aperture(fb).is_some(){"Standard VGA / Bochs - runtime modes"}else if svga_active(fb).is_some(){"VMware SVGA II - native modes (preview)"}else if crate::boot::vmware(){"VMware firmware framebuffer - fixed mode"}else{"UEFI framebuffer - fixed mode"}}
fn program(w:u16,h:u16,bpp:u16,enable:u16,virtual_width:u16,xoff:u16,yoff:u16){
    put(4,0);put(1,w);put(2,h);put(3,bpp);put(4,enable);put(6,virtual_width);put(8,xoff);put(9,yoff);
}
pub fn set_mode(fb:Framebuffer,i:usize)->Option<Framebuffer>{
    let &(w,h,_)=MODES.get(i)?;if let Some(a)=svga_active(fb){return a.change(fb,w,h);}let size=aperture(fb)?;if w*h*4>size{return None;}
    let old=[reg(1),reg(2),reg(3),reg(4),reg(6),reg(8),reg(9)];
    program(w as u16,h as u16,32,0x41,w as u16,0,0);let stride=reg(6)as usize;
    if reg(1)as usize!=w||reg(2)as usize!=h||reg(3)!=32||reg(4)&1==0||stride<w||stride.checked_mul(h).and_then(|n|n.checked_mul(4)).is_none_or(|n|n>size){
        program(old[0],old[1],old[2],old[3],old[4],old[5],old[6]);
        if [reg(1),reg(2),reg(3),reg(4),reg(6),reg(8),reg(9)]!=old{crate::serial::line("H5K: FAIL GRAPHICS_ROLLBACK");crate::kernel::halt();}return None;
    }
    Some(Framebuffer{base:fb.base,size,width:w,height:h,stride,format:1})
}

#[derive(Clone,Copy)]pub struct SvgaInfo{pub io:u16,pub fb:usize,pub fifo:usize}
pub fn svga_info()->Option<SvgaInfo>{
    for slot in 0..32{if pci(slot,0)!=0x040515ad{continue;}if pci(slot,8)>>24!=3||pci(slot,4)&3!=3{continue;}let io=pci(slot,0x10);let fb=pci(slot,0x14);let fifo=pci(slot,0x18);
        if io&1==0||io&!3==0||io&!3>0xfff8||fb&7!=0||fifo&7!=0||fb&!15==0||fifo&!15==0{continue;}
        return Some(SvgaInfo{io:(io&!3)as u16,fb:(fb&!15)as usize,fifo:(fifo&!15)as usize});
    }None
}
pub fn log_adapter(){if let Some(v)=svga_info(){crate::serial::line("H5K: SVGA2_PCI_DETECTED");crate::serial::metric("H5K: SVGA2_IO=",v.io as u64);crate::serial::metric("H5K: SVGA2_FB=",v.fb as u64);crate::serial::metric("H5K: SVGA2_FIFO=",v.fifo as u64);}else{crate::serial::line("H5K: SVGA2_PCI_NOT_DETECTED");}}

// VMware SVGA II register interface. Only take over an already enabled,
// trace-updated 32bpp linear framebuffer; no FIFO reset, PCI BAR sizing,
// topology, screen objects, 3D or unvalidated framebuffer writes.
// Reference: VMware svga_interface.txt, sections SetMode and Mode changes.
const SVGA_ID2:u32=0x90000002;
const CAP_TRACES:u32=0x00200000;
#[derive(Clone,Copy)]struct SvgaState{width:usize,height:usize,max_w:usize,max_h:usize,vram:usize,offset:usize,pitch:usize,format:u32,fb_size:usize}
impl SvgaState{
    fn fits(&self,w:usize,h:usize)->bool{w>=640&&h>=480&&w<=1920&&h<=1080&&w<=self.max_w&&h<=self.max_h&&w.checked_mul(h).and_then(|v|v.checked_mul(4)).is_some_and(|v|v<=self.vram.saturating_sub(self.offset))}
    fn framebuffer(&self,bar:usize)->Option<Framebuffer>{
        let stride=self.pitch/4;let visible=self.pitch.checked_mul(self.height)?;
        if !self.fits(self.width,self.height)||self.pitch%4!=0||stride<self.width||self.pitch>65536||visible>self.fb_size||visible>self.vram.checked_sub(self.offset)?{return None;}
        if self.vram==0||self.vram>256*1024*1024||bar<0x80000000||bar.checked_add(self.vram)?>0x1_0000_0000||self.offset%4!=0{return None;}
        Some(Framebuffer{base:bar.checked_add(self.offset)?,size:self.vram-self.offset,width:self.width,height:self.height,stride,format:self.format})
    }
}
trait SvgaRegs{fn get(&self,i:u32)->u32;fn put(&mut self,i:u32,v:u32);}
#[derive(Clone,Copy)]struct Svga{info:SvgaInfo}
impl SvgaRegs for Svga{fn get(&self,i:u32)->u32{crate::platform::out32(self.info.io,i);crate::platform::input32(self.info.io+1)}fn put(&mut self,i:u32,v:u32){crate::platform::out32(self.info.io,i);crate::platform::out32(self.info.io+1,v);}}
fn read_state(r:&impl SvgaRegs)->Option<SvgaState>{
    if r.get(0)!=SVGA_ID2||r.get(1)!=1||r.get(7)!=32||r.get(8)!=0{return None;}
    // If FIFO is enabled, trace updates must already be explicitly enabled.
    // Otherwise the existing software framebuffer cannot notify this device.
    let config=r.get(20);let caps=r.get(17);
    if config>1||(config!=0&&(caps&CAP_TRACES==0||r.get(45)!=1)){return None;}
    let format=match (r.get(9),r.get(10),r.get(11)){(0x00ff0000,0x0000ff00,0x000000ff)=>1,(0x000000ff,0x0000ff00,0x00ff0000)=>0,_=>return None};
    Some(SvgaState{width:r.get(2)as usize,height:r.get(3)as usize,max_w:r.get(4)as usize,max_h:r.get(5)as usize,vram:r.get(15)as usize,offset:r.get(14)as usize,pitch:r.get(12)as usize,format,fb_size:r.get(16)as usize})
}
fn same_visible(a:Framebuffer,b:Framebuffer)->bool{a.base==b.base&&a.width==b.width&&a.height==b.height&&a.stride==b.stride&&a.format==b.format}
fn switch_native(r:&mut impl SvgaRegs,bar:usize,old:Framebuffer,w:usize,h:usize)->Result<Framebuffer,bool>{
    let Some(state)=read_state(r)else{return Err(true);};let Some(current)=state.framebuffer(bar)else{return Err(true);};
    if !same_visible(current,old)||!state.fits(w,h){return Err(true);}
    if (w,h)==(old.width,old.height){return Ok(current);}
    // Keep the active device enabled; WIDTH/HEIGHT changes are supported.
    r.put(2,w as u32);r.put(3,h as u32);
    if let Some(f)=read_state(r).and_then(|v|v.framebuffer(bar)){if f.width==w&&f.height==h&&f.format==old.format{return Ok(f);}}
    r.put(2,old.width as u32);r.put(3,old.height as u32);
    Err(read_state(r).and_then(|v|v.framebuffer(bar)).is_some_and(|f|same_visible(f,old)))
}
impl Svga{
    fn fits(&self,w:usize,h:usize)->bool{read_state(self).is_some_and(|s|s.fits(w,h))}
    fn change(mut self,old:Framebuffer,w:usize,h:usize)->Option<Framebuffer>{let bar=self.info.fb;match switch_native(&mut self,bar,old,w,h){Ok(f)=>{crate::serial::line("H5K: SVGA2_NATIVE_MODE_OK");Some(f)},Err(true)=>{crate::serial::line("H5K: SVGA2_MODE_REJECTED");None},Err(false)=>{crate::serial::line("H5K: FAIL SVGA2_MODE_ROLLBACK");crate::kernel::halt();}}}
}
fn svga_active(fb:Framebuffer)->Option<Svga>{let info=svga_info()?;let a=Svga{info};let native=read_state(&a)?.framebuffer(info.fb)?;same_visible(native,fb).then_some(a)}
pub fn native_preview(fb:Framebuffer)->bool{svga_active(fb).is_some()}
pub fn log_runtime(fb:Framebuffer){if let Some(info)=svga_info(){let a=Svga{info};crate::serial::metric("H5K: SVGA2_ID=",a.get(0)as u64);crate::serial::metric("H5K: SVGA2_ENABLE=",a.get(1)as u64);if a.get(0)==SVGA_ID2{crate::serial::metric("H5K: SVGA2_CAPS=",a.get(17)as u64);crate::serial::metric("H5K: SVGA2_CONFIG=",a.get(20)as u64);crate::serial::metric("H5K: SVGA2_VRAM=",a.get(15)as u64);}crate::serial::line(if native_preview(fb){"H5K: SVGA2_RUNTIME_READY"}else{"H5K: SVGA2_FIRMWARE_FALLBACK"});}}
pub fn svga_test()->bool{
    struct Fake{regs:[u32;46],reject:bool}
    impl SvgaRegs for Fake{fn get(&self,i:u32)->u32{self.regs[i as usize]}fn put(&mut self,i:u32,v:u32){if self.reject&&i==3&&v==1080{return;}self.regs[i as usize]=v;if i==2{self.regs[12]=v*4;}}}
    let mut r=Fake{regs:[0;46],reject:false};for(i,v)in [(0,SVGA_ID2),(1,1),(2,1280),(3,720),(4,1920),(5,1080),(7,32),(9,0xff0000),(10,0xff00),(11,0xff),(12,5120),(14,4096),(15,32*1024*1024),(16,16*1024*1024)]{r.regs[i]=v;}
    let bar=0xe0000000;let Some(old)=read_state(&r).and_then(|s|s.framebuffer(bar))else{return false;};
    let Ok(new)=switch_native(&mut r,bar,old,1920,1080)else{return false;};
    if new.width!=1920||new.height!=1080||new.stride!=1920||new.base!=bar+4096{return false;}
    if switch_native(&mut r,bar,new,1280,720).is_err(){return false;}
    r.reject=true;
    if !matches!(switch_native(&mut r,bar,old,1920,1080),Err(true))||r.get(2)!=1280||r.get(3)!=720{return false;}
    r.regs[20]=1;if read_state(&r).is_some(){return false;}r.regs[17]=CAP_TRACES;r.regs[45]=1;if read_state(&r).is_none(){return false;}
    r.regs[12]=10;if read_state(&r).and_then(|s|s.framebuffer(bar)).is_some(){return false;}true
}
