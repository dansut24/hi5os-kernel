//! Direct framebuffer diagnostics: original 5x7 glyphs, no font dependency.
use crate::boot::Framebuffer;
use core::ptr;
const BG: [u8;3] = [9,21,36];
const BLUE: [u8;3] = [80,174,241];
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
        s.line("HI5OS - HI5KERNEL V0.1.3",BLUE);
        s.line("MEMORY DEVELOPMENT PREVIEW",WHITE);
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
