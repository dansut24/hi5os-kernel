//! Hi5UI software rasteriser. Native RGB framebuffer; bounded back buffer.
use crate::boot::Framebuffer;
use core::ptr;
pub type Color=[u8;3];
const MAX:usize=2048*1536;
#[repr(align(4096))]
struct Buffer([u32;MAX]);
static mut BACK:Buffer=Buffer([0;MAX]);
static REG:&[u8;97280]=include_bytes!("generated/regular.bin");
static BIG:&[u8;97280]=include_bytes!("generated/large.bin");
static REG_ADV:&[u8;95]=include_bytes!("generated/regular-adv.bin");
static BIG_ADV:&[u8;95]=include_bytes!("generated/large-adv.bin");
pub struct Canvas{pub fb:Framebuffer,pub buffered:bool}
impl Canvas{
    pub fn new(fb:Framebuffer)->Self{Self{fb,buffered:fb.width.checked_mul(fb.height).map_or(false,|n|n<=MAX)}}
    fn pack(&self,c:Color)->u32{if self.fb.format==0{u32::from_le_bytes([c[0],c[1],c[2],0])}else{u32::from_le_bytes([c[2],c[1],c[0],0])}}
    fn unpack(&self,v:u32)->Color{let b=v.to_le_bytes();if self.fb.format==0{[b[0],b[1],b[2]]}else{[b[2],b[1],b[0]]}}
    fn target(&self,x:usize,y:usize)->*mut u32{unsafe{if self.buffered{ptr::addr_of_mut!(BACK.0).cast::<u32>().add(y*self.fb.width+x)}else{(self.fb.base as *mut u32).add(y*self.fb.stride+x)}}}
    fn set(&self,x:usize,y:usize,v:u32){if x>=self.fb.width||y>=self.fb.height{return;}unsafe{if self.buffered{ptr::write(self.target(x,y),v);}else{ptr::write_volatile(self.target(x,y),v);}}}
    pub fn pixel(&self,x:usize,y:usize,c:Color){self.set(x,y,self.pack(c));}
    pub fn rect(&self,x:usize,y:usize,w:usize,h:usize,c:Color){
        let x2=x.saturating_add(w).min(self.fb.width);let y2=y.saturating_add(h).min(self.fb.height);
        if x>=x2||y>=y2{return;}let v=self.pack(c);
        for yy in y..y2{unsafe{let dst=self.target(x,yy);if self.buffered{for i in 0..x2-x{ptr::write(dst.add(i),v);}}else{for i in 0..x2-x{ptr::write_volatile(dst.add(i),v);}}}}
    }
    pub fn round(&self,x:usize,y:usize,w:usize,h:usize,r:usize,c:Color){
        let r=r.min(w/2).min(h/2);
        if r==0{self.rect(x,y,w,h,c);return;}
        self.rect(x,y+r,w,h-2*r,c);
        for i in 0..r{
            let d=r-i;let mut off=r;
            for j in 0..r{let dx=r-j;if dx*dx+d*d<=r*r{off=j;break;}}
            self.rect(x+off,y+i,w-2*off,1,c);
            self.rect(x+off,y+h-1-i,w-2*off,1,c);
        }
    }
    pub fn text(&self,x:usize,y:usize,t:&str,c:Color){self.text_limit(x,y,t,c,false,self.fb.width);}
    pub fn title(&self,x:usize,y:usize,t:&str,c:Color){self.text_limit(x,y,t,c,true,self.fb.width);}
    pub fn text_limit(&self,mut x:usize,y:usize,t:&str,c:Color,big:bool,right:usize){
        let (atlas,adv)=if big{(BIG,BIG_ADV)}else{(REG,REG_ADV)};
        for byte in t.bytes(){
            let idx=if (32..127).contains(&byte){(byte-32)as usize}else{31};
            let width=adv[idx]as usize;
            if x.saturating_add(width+1)>right{break;}
            for yy in 0..32{if y+yy>=self.fb.height{break;}for xx in 0..32{
                if x+xx>=right||x+xx>=self.fb.width{break;}
                let a=atlas[idx*1024+yy*32+xx]as u32;if a==0{continue;}
                if a==255{self.pixel(x+xx,y+yy,c);}else{
                    let old=unsafe{ptr::read_volatile(self.target(x+xx,y+yy))};let bg=self.unpack(old);
                    let mix=|i:usize|((c[i]as u32*a+bg[i]as u32*(255-a)+127)/255)as u8;
                    self.pixel(x+xx,y+yy,[mix(0),mix(1),mix(2)]);
                }
            }}
            x+=width;
        }
    }
    pub fn num(&self,x:usize,y:usize,mut n:u64,c:Color){let mut b=[b'0';20];let mut i=20;loop{i-=1;b[i]=b'0'+(n%10)as u8;n/=10;if n==0{break;}}if let Ok(t)=core::str::from_utf8(&b[i..]){self.text(x,y,t,c);}}
    pub fn present(&self){if !self.buffered{return;}unsafe{
        let src=ptr::addr_of!(BACK.0).cast::<u32>();let dst=self.fb.base as *mut u32;
        for y in 0..self.fb.height{for x in 0..self.fb.width{ptr::write_volatile(dst.add(y*self.fb.stride+x),ptr::read(src.add(y*self.fb.width+x)));}}
    }}
    pub fn front_pixel(&self,x:usize,y:usize,c:Color){if x<self.fb.width&&y<self.fb.height{unsafe{ptr::write_volatile((self.fb.base as *mut u32).add(y*self.fb.stride+x),self.pack(c));}}}
}
